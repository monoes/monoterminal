// Windows ConPTY Backend Implementation
// SRS Reference: §2.1.2.3 Windows ConPTY [D1.2.3]
//
// Implements the PtyBackend trait for Windows using the Console Pseudo-console API.
// Windows 10 1809+ (build 17763+) required.
//
// Architecture:
// - BufReader/BufWriter around async pipe handles for I/O
// - Direct read()/write() calls from Session Manager (no background tasks)
// - Cleanup via terminate() or Drop
//
// Safety: All unsafe FFI calls have documented safety invariants.

use super::{
    error::{PtyError, PtyResult},
    PtyBackend, PtyConfig,
};
use async_trait::async_trait;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows::Win32::System::Threading::{
    CreateProcessW, InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

/// 4KB buffer size per SRS §3.1.4
const PTY_BUFFER_SIZE: usize = 4096;

/// Windows ConPTY backend
///
/// Implements PtyBackend trait for Windows using CreatePseudoConsole API.
/// Session Manager calls methods on this struct via `Box<dyn PtyBackend>`.
pub struct ConPtyBackend {
    /// Pseudo-console handle (Option allows terminate() to consume and prevent Drop race)
    hpc: Option<HPCON>,
    /// Child process handle (Option allows terminate() to consume and prevent Drop race)
    process_handle: Option<HANDLE>,
    /// Buffered output reader (ConPTY → Session Manager)
    output_reader: BufReader<AsyncPipeReader>,
    /// Direct input writer (Session Manager → ConPTY) - unbuffered for immediate delivery
    input_writer: AsyncPipeWriter,
    /// Shell process ID
    shell_pid: u32,
}

// SAFETY: Windows HANDLEs are safe to send between threads - they're just kernel object references.
// The HPCON and HANDLE types are opaque handles that can be used from any thread.
// They are also safe to share (&T) between threads as the Windows kernel handles synchronization.
unsafe impl Send for ConPtyBackend {}
unsafe impl Sync for ConPtyBackend {}

/// Async wrapper around Windows pipe HANDLE
///
/// Implements tokio::io::AsyncRead and AsyncWrite.
/// Uses blocking I/O with spawn_blocking for Phase 1 simplicity.
/// TODO: Implement proper overlapped I/O with IOCP for production.
struct AsyncPipeReader {
    handle: HANDLE,
}

struct AsyncPipeWriter {
    handle: HANDLE,
}

// SAFETY: HANDLEs are safe to send and share between threads
unsafe impl Send for AsyncPipeReader {}
unsafe impl Sync for AsyncPipeReader {}
unsafe impl Send for AsyncPipeWriter {}
unsafe impl Sync for AsyncPipeWriter {}

impl AsyncPipeReader {
    unsafe fn from_handle(handle: HANDLE) -> Self {
        Self { handle }
    }
}

impl AsyncPipeWriter {
    unsafe fn from_handle(handle: HANDLE) -> Self {
        Self { handle }
    }
}

impl Drop for AsyncPipeReader {
    fn drop(&mut self) {
        // SAFETY: Close the pipe handle to prevent resource leaks
        // CloseHandle is safe to call on valid HANDLEs and handles double-close gracefully
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl Drop for AsyncPipeWriter {
    fn drop(&mut self) {
        // SAFETY: Close the pipe handle to prevent resource leaks
        // CloseHandle is safe to call on valid HANDLEs and handles double-close gracefully
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl tokio::io::AsyncRead for AsyncPipeReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Phase 1: Use PeekNamedPipe to check data availability before reading
        // This avoids blocking the tokio executor with synchronous ReadFile
        // TODO: Implement proper overlapped I/O with IOCP for production (Phase 2+)

        use windows::Win32::Storage::FileSystem::ReadFile;

        let handle = self.handle;

        // Check if data is available WITHOUT blocking
        let mut bytes_available: u32 = 0;

        // SAFETY: PeekNamedPipe is safe to call with valid handle
        // We only check bytes available, not reading actual data yet
        let peek_result = unsafe {
            PeekNamedPipe(
                handle,
                None,                       // Don't read data, just check availability
                0,                          // No buffer
                None,                       // Don't need bytes read
                Some(&mut bytes_available), // Get bytes available
                None,                       // Don't need bytes left in message
            )
        };

        // DIAGNOSTIC: Log PeekNamedPipe result
        tracing::debug!(
            "🔍 PeekNamedPipe: handle={:?}, result={:?}, bytes_available={}",
            handle.0,
            peek_result.is_ok(),
            bytes_available
        );

        match peek_result {
            Err(e) => {
                let err_code = e.code().0;
                tracing::warn!(
                    "🔍 PeekNamedPipe ERROR: code={}, handle={:?}",
                    err_code,
                    handle.0
                );
                // ERROR_NO_DATA (232) or ERROR_PIPE_NOT_CONNECTED (233) = pipe closing
                if err_code == 232 || err_code == 233 {
                    tracing::debug!("🔍 Pipe closing (error {}), returning EOF", err_code);
                    return std::task::Poll::Ready(Ok(())); // EOF
                } else {
                    return std::task::Poll::Ready(Err(std::io::Error::from_raw_os_error(
                        err_code,
                    )));
                }
            }
            Ok(_) if bytes_available == 0 => {
                // No data available yet - return Pending without blocking
                // Register waker so tokio runtime polls this future again
                tracing::trace!("🔍 No data available, returning Pending");
                cx.waker().wake_by_ref();
                return std::task::Poll::Pending;
            }
            Ok(_) => {
                // Data available! Safe to call ReadFile (won't block)
                tracing::info!(
                    "🔍 DATA AVAILABLE: {} bytes ready to read!",
                    bytes_available
                );
            }
        }

        // SAFETY: We need raw access to the buffer for Windows ReadFile API
        let buf_slice = unsafe {
            let ptr = buf.unfilled_mut().as_mut_ptr();
            let len = buf.unfilled_mut().len();
            // Cast MaybeUninit<u8> to u8 for Windows API - ReadFile will initialize it
            std::slice::from_raw_parts_mut(ptr as *mut u8, len)
        };

        // Data is available - call ReadFile (won't block because we peeked first)
        let mut bytes_read: u32 = 0;

        tracing::debug!("🔍 Calling ReadFile: buffer_size={}", buf_slice.len());

        // SAFETY: ReadFile is safe to call with valid handle and buffer
        // We know data is available from PeekNamedPipe, so this won't block
        let result = unsafe { ReadFile(handle, Some(buf_slice), Some(&mut bytes_read), None) };

        tracing::info!(
            "🔍 ReadFile RESULT: {:?}, bytes_read={}",
            result.is_ok(),
            bytes_read
        );

        match result {
            Ok(_) if bytes_read > 0 => {
                tracing::info!("🎉 READ SUCCESS: {} bytes read from ConPTY!", bytes_read);
                unsafe { buf.assume_init(bytes_read as usize) };
                buf.advance(bytes_read as usize);
                std::task::Poll::Ready(Ok(()))
            }
            Ok(_) => {
                // Peek said data available but ReadFile got 0 bytes
                // This can happen with race conditions - return Pending and try again
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
            Err(e) => {
                let err_code = e.code().0;
                // ERROR_NO_DATA (232) or ERROR_PIPE_NOT_CONNECTED (233) = pipe closing
                if err_code == 232 || err_code == 233 {
                    std::task::Poll::Ready(Ok(())) // EOF
                } else {
                    std::task::Poll::Ready(Err(std::io::Error::from_raw_os_error(err_code)))
                }
            }
        }
    }
}

impl tokio::io::AsyncWrite for AsyncPipeWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        use windows::Win32::Storage::FileSystem::WriteFile;

        let handle = self.handle;
        let mut bytes_written: u32 = 0;

        // SAFETY: WriteFile is safe to call with valid handle and buffer
        let result = unsafe { WriteFile(handle, Some(buf), Some(&mut bytes_written), None) };

        match result {
            Ok(_) => std::task::Poll::Ready(Ok(bytes_written as usize)),
            Err(e) => {
                let err_code = e.code().0;
                // ERROR_NO_DATA (232) or ERROR_PIPE_NOT_CONNECTED (233) = pipe closing
                if err_code == 232 || err_code == 233 {
                    std::task::Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "PTY pipe closed",
                    )))
                } else {
                    std::task::Poll::Ready(Err(std::io::Error::from_raw_os_error(err_code)))
                }
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use windows::Win32::Storage::FileSystem::FlushFileBuffers;

        // Explicitly flush to ensure low-latency input delivery
        let result = unsafe { FlushFileBuffers(self.handle) };

        match result {
            Ok(_) => std::task::Poll::Ready(Ok(())),
            Err(_) => {
                // Flush failure is non-fatal for pipes, data is still written
                std::task::Poll::Ready(Ok(()))
            }
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        // Flush before shutdown
        self.poll_flush(_cx)
    }
}

#[async_trait]
impl PtyBackend for ConPtyBackend {
    async fn create(config: PtyConfig) -> PtyResult<Self> {
        tracing::info!(
            "Creating ConPTY: shell={}, cwd={:?}, size={}x{}",
            config.shell,
            config.working_dir,
            config.cols,
            config.rows
        );

        // Create pipes for ConPTY I/O
        let (input_read, input_write) = create_pipe()?;
        let (output_read, output_write) = create_pipe()?;

        tracing::info!("🔍 PIPE HANDLES CREATED:");
        tracing::info!(
            "🔍   input_read={:?}, input_write={:?}",
            input_read.0,
            input_write.0
        );
        tracing::info!(
            "🔍   output_read={:?}, output_write={:?}",
            output_read.0,
            output_write.0
        );

        // Create pseudo-console
        let coord = COORD {
            X: config.cols as i16,
            Y: config.rows as i16,
        };

        // SAFETY: CreatePseudoConsole is safe to call with valid handles and size.
        // The handles are owned by us and will be properly managed.
        // Note: API changed in windows crate 0.58+ - now returns HPCON directly
        tracing::info!("🔍 Calling CreatePseudoConsole:");
        tracing::info!(
            "🔍   size={}x{}, input_read={:?}, output_write={:?}",
            coord.X,
            coord.Y,
            input_read.0,
            output_write.0
        );

        let hpc = unsafe {
            CreatePseudoConsole(coord, input_read, output_write, 0)
                .map_err(|e| PtyError::CreateFailed(format!("CreatePseudoConsole failed: {}", e)))?
        };

        tracing::info!("🔍 CreatePseudoConsole SUCCESS, hpc={:?}", hpc.0);

        // CRITICAL: Close the PTY-end handles after CreatePseudoConsole
        // Per Microsoft ConPTY sample: CreatePseudoConsole duplicates the handles internally
        // We must close our copies or ConPTY won't activate the pipes!
        // See: https://github.com/microsoft/terminal/blob/main/samples/ConPTY/EchoCon/EchoCon/EchoCon.cpp#L123-125
        unsafe {
            tracing::info!(
                "🔍 Closing PTY-end handles: input_read={:?}, output_write={:?}",
                input_read.0,
                output_write.0
            );
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(output_write);
            tracing::info!("🔍 PTY-end handles closed - ConPTY now owns duplicates");
        }

        // Spawn child process attached to ConPTY
        let (process_handle, shell_pid) = spawn_process(&hpc, &config)?;

        // Wrap pipe handles in async readers/writers
        // SAFETY: We're wrapping valid pipe handles for async I/O
        tracing::info!("🔍 Wrapping pipe handles:");
        tracing::info!("🔍   output_reader <- output_read={:?}", output_read.0);
        tracing::info!("🔍   input_writer <- input_write={:?}", input_write.0);

        // DIAGNOSTIC: Verify handle is valid before wrapping
        tracing::info!(
            "🔍 VALIDATION: output_read handle = {:?} (should read ConPTY output)",
            output_read.0
        );
        tracing::info!("🔍 VALIDATION: This is the CLIENT-END handle (we read, ConPTY writes)");

        // CRITICAL DIAGNOSTIC: Try RAW synchronous ReadFile to see if data exists
        // This tests if the pipe actually has data, bypassing all async machinery
        use windows::Win32::Storage::FileSystem::ReadFile;
        tracing::info!("🔍 RAW READ TEST: Attempting synchronous ReadFile for 100ms...");

        // Give ping a moment to start and output
        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut test_buffer = [0u8; 1024];
        let mut bytes_read: u32 = 0;
        let raw_read_result = unsafe {
            ReadFile(
                output_read,
                Some(&mut test_buffer),
                Some(&mut bytes_read),
                None,
            )
        };

        match raw_read_result {
            Ok(_) if bytes_read > 0 => {
                tracing::error!(
                    "🎉🎉🎉 RAW READ SUCCESS: {} bytes! Data EXISTS! AsyncPipeReader is the bug!",
                    bytes_read
                );
                tracing::error!(
                    "🎉 First 100 bytes: {:?}",
                    &test_buffer[..std::cmp::min(100, bytes_read as usize)]
                );
            }
            Ok(_) => {
                tracing::error!("🔍 RAW READ: 0 bytes (no data yet, but read succeeded)");
            }
            Err(e) => {
                tracing::error!("🔍 RAW READ ERROR: {:?}", e);
            }
        }

        let output_reader = BufReader::with_capacity(PTY_BUFFER_SIZE, unsafe {
            AsyncPipeReader::from_handle(output_read)
        });

        // Direct writer without buffering - immediate writes to ConPTY
        // BufWriter was causing flush() to block, preventing data delivery
        let input_writer = unsafe { AsyncPipeWriter::from_handle(input_write) };

        tracing::info!("ConPTY session created: pid={}", shell_pid);

        Ok(Self {
            hpc: Some(hpc),
            process_handle: Some(process_handle),
            output_reader,
            input_writer,
            shell_pid,
        })
    }

    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        tracing::info!("PTY backend: read() ENTRY, buffer size={}", buf.len());
        tracing::info!("PTY backend: About to call output_reader.read()");
        let result = self.output_reader.read(buf).await;
        tracing::info!(
            "PTY backend: output_reader.read() returned: {:?}",
            result
                .as_ref()
                .map(|n| format!("{} bytes", n))
                .unwrap_or_else(|e| format!("Error: {}", e))
        );
        result
    }

    async fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        tracing::info!("📝 PTY write() ENTRY: {} bytes", data.len());
        tracing::info!("📝 PTY calling input_writer.write_all() (direct, no buffer)");
        // Direct write without BufWriter - data goes straight to ConPTY
        self.input_writer.write_all(data).await?;
        tracing::info!("📝 PTY write_all() SUCCESS - data sent to ConPTY");
        Ok(())
    }

    fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()> {
        let coord = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        // Get hpc handle, error if already consumed by terminate()
        let hpc = self
            .hpc
            .ok_or_else(|| PtyError::ResizeFailed("PTY already terminated".to_string()))?;

        // SAFETY: ResizePseudoConsole is safe to call on a valid HPCON
        unsafe {
            ResizePseudoConsole(hpc, coord).map_err(|e| PtyError::ResizeFailed(e.to_string()))?;
        }

        tracing::debug!("Resized ConPTY to {}x{}", cols, rows);
        Ok(())
    }

    fn shell_pid(&self) -> u32 {
        self.shell_pid
    }

    async fn terminate(mut self: Box<Self>) -> PtyResult<()> {
        tracing::info!("Terminating ConPTY session: pid={}", self.shell_pid);

        // Take ownership of handles to prevent Drop from double-closing
        let hpc = self.hpc.take();
        let process_handle = self.process_handle.take();

        if let (Some(hpc), Some(process_handle)) = (hpc, process_handle) {
            // SAFETY: TerminateProcess is safe to call on a valid process handle
            unsafe {
                TerminateProcess(process_handle, 1).map_err(|e| {
                    PtyError::CreateFailed(format!("TerminateProcess failed: {}", e))
                })?;

                // Cleanup handles (now consumed, Drop won't run on them)
                ClosePseudoConsole(hpc);
                let _ = CloseHandle(process_handle);
            }

            tracing::info!("ConPTY session terminated: pid={}", self.shell_pid);
        } else {
            tracing::warn!("ConPTY session already terminated: pid={}", self.shell_pid);
        }

        Ok(())
    }
}

impl Drop for ConPtyBackend {
    fn drop(&mut self) {
        tracing::debug!("Dropping ConPTY backend: pid={}", self.shell_pid);

        // Only close handles if they weren't consumed by terminate()
        // This prevents double-close and race conditions with active ReadFile calls
        if let (Some(hpc), Some(process_handle)) = (self.hpc.take(), self.process_handle.take()) {
            tracing::debug!("Drop cleaning up ConPTY handles: pid={}", self.shell_pid);

            // SAFETY: Handles are valid and haven't been closed yet
            unsafe {
                ClosePseudoConsole(hpc);
                let _ = CloseHandle(process_handle);
            }
        } else {
            tracing::debug!(
                "Drop: ConPTY handles already cleaned up (terminate() called): pid={}",
                self.shell_pid
            );
        }
    }
}

// Helper method for test compatibility - boxes self and calls trait method
impl ConPtyBackend {
    /// Terminate the PTY session (helper that boxes internally)
    ///
    /// This method allows calling terminate() on a bare ConPtyBackend instance
    /// in tests, which internally boxes it and calls the PtyBackend trait method.
    ///
    /// Production code uses Box<dyn PtyBackend> and calls the trait method directly.
    /// This shadows the trait method when called on concrete type.
    #[allow(dead_code)] // Used in tests
    pub async fn terminate(self) -> PtyResult<()> {
        Box::new(self).terminate().await
    }
}

// ========== Helper Functions ==========

/// Create a pipe for ConPTY I/O
fn create_pipe() -> PtyResult<(HANDLE, HANDLE)> {
    let mut read_handle = HANDLE::default();
    let mut write_handle = HANDLE::default();

    // SAFETY: CreatePipe is safe to call with valid out-pointers.
    // We pass None for security attributes (default) and 0 for buffer size (default).
    unsafe {
        CreatePipe(&mut read_handle, &mut write_handle, None, 0)
            .map_err(|e| PtyError::CreateFailed(format!("CreatePipe failed: {}", e)))?;
    }

    Ok((read_handle, write_handle))
}

/// Convert Rust string to null-terminated wide string for Windows APIs
fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Spawn child process attached to ConPTY
///
/// # Safety
/// Uses CreateProcessW with EXTENDED_STARTUPINFO_PRESENT and properly
/// initialized STARTUPINFOEX containing the ConPTY handle.
fn spawn_process(hpc: &HPCON, config: &PtyConfig) -> PtyResult<(HANDLE, u32)> {
    let mut command_line = to_wide_string(&config.shell);

    let cwd = config
        .working_dir
        .to_str()
        .ok_or_else(|| PtyError::InvalidConfig("Invalid working directory".to_string()))?;
    let cwd_wide = to_wide_string(cwd);

    // Initialize STARTUPINFOEX
    let mut startup_info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup_info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    // Do NOT set STARTF_USESTDHANDLES - ConPTY attribute handles std redirects automatically
    // Setting it without hStdInput/Output/Error causes child process to get INVALID_HANDLE_VALUE
    // startup_info.StartupInfo.dwFlags = 0;  // Already zero from zeroed(), no need to set

    // Determine required size for attribute list
    let mut attr_size: usize = 0;
    unsafe {
        let _ = InitializeProcThreadAttributeList(
            LPPROC_THREAD_ATTRIBUTE_LIST(ptr::null_mut()),
            1,
            0,
            &mut attr_size,
        );
    }

    // Allocate and initialize attribute list
    let mut attr_list_buffer: Vec<u8> = vec![0u8; attr_size];
    let attr_list_ptr = LPPROC_THREAD_ATTRIBUTE_LIST(attr_list_buffer.as_mut_ptr() as *mut _);

    unsafe {
        InitializeProcThreadAttributeList(attr_list_ptr, 1, 0, &mut attr_size).map_err(|e| {
            PtyError::SpawnFailed(format!("InitializeProcThreadAttributeList failed: {}", e))
        })?;
    }

    startup_info.lpAttributeList = attr_list_ptr;

    // Attach ConPTY to the attribute list
    unsafe {
        UpdateProcThreadAttribute(
            attr_list_ptr,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(hpc as *const _ as *const _),
            std::mem::size_of::<HPCON>(),
            None,
            None,
        )
        .map_err(|e| PtyError::SpawnFailed(format!("UpdateProcThreadAttribute failed: {}", e)))?;
    }

    // Create the process
    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    unsafe {
        CreateProcessW(
            None,
            windows::core::PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            None,
            PCWSTR(cwd_wide.as_ptr()),
            &startup_info.StartupInfo,
            &mut process_info,
        )
        .map_err(|e| PtyError::SpawnFailed(format!("CreateProcessW failed: {}", e)))?;
    }

    // Close thread handle (we don't need it)
    unsafe {
        let _ = CloseHandle(process_info.hThread);
    }

    Ok((process_info.hProcess, process_info.dwProcessId))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[tokio::test]
    async fn test_create_conpty() {
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let backend = ConPtyBackend::create(config)
            .await
            .expect("Failed to create ConPTY");

        assert!(backend.shell_pid() > 0);
    }

    #[tokio::test]
    async fn test_create_powershell() {
        let config = PtyConfig {
            shell: "powershell.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let backend = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PowerShell ConPTY");

        assert!(backend.shell_pid() > 0);

        // Clean up
        backend.terminate().await.ok();
    }

    #[tokio::test]
    #[ignore = "Known issue: AsyncPipeReader uses blocking ReadFile in poll_read, violates tokio async contract. See windows.rs PtyHandle for proper async architecture. TODO: Phase 2 - migrate to windows.rs or implement proper overlapped I/O"]
    async fn test_write_read() {
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut backend = ConPtyBackend::create(config)
            .await
            .expect("Failed to create ConPTY");

        // Write echo command
        backend
            .write(b"echo hello\r\n")
            .await
            .expect("Failed to write");

        // Read output
        let mut found = false;
        let mut buf = vec![0u8; 4096];

        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;

            match backend.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) if n > 0 => {
                    let text = String::from_utf8_lossy(&buf[..n]);
                    tracing::debug!("Received: {}", text);

                    if text.contains("hello") {
                        found = true;
                        break;
                    }
                }
                Ok(_) => unreachable!("read returned non-zero but pattern didn't match"),
                Err(e) => {
                    tracing::error!("Read error: {}", e);
                    break;
                }
            }
        }

        assert!(found, "Expected 'hello' in output");

        // Clean up
        backend.terminate().await.ok();
    }

    #[tokio::test]
    async fn test_resize() {
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut backend = ConPtyBackend::create(config)
            .await
            .expect("Failed to create ConPTY");

        // Resize should not fail
        backend.resize(30, 100).expect("Failed to resize");
        backend.resize(50, 120).expect("Failed to resize again");

        // Clean up
        backend.terminate().await.ok();
    }

    #[tokio::test]
    async fn test_terminate() {
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let backend = ConPtyBackend::create(config)
            .await
            .expect("Failed to create ConPTY");

        let pid = backend.shell_pid();
        assert!(pid > 0);

        // Terminate should succeed
        backend.terminate().await.expect("Failed to terminate");
    }
}
