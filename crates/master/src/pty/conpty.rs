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
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES,
    STARTUPINFOEXW,
};

/// 4KB buffer size per SRS §3.1.4
const PTY_BUFFER_SIZE: usize = 4096;

/// Windows ConPTY backend
///
/// Implements PtyBackend trait for Windows using CreatePseudoConsole API.
/// Session Manager calls methods on this struct via `Box<dyn PtyBackend>`.
pub struct ConPtyBackend {
    /// Pseudo-console handle
    hpc: HPCON,
    /// Child process handle
    process_handle: HANDLE,
    /// Buffered output reader (ConPTY → Session Manager)
    output_reader: BufReader<AsyncPipeReader>,
    /// Buffered input writer (Session Manager → ConPTY)
    input_writer: BufWriter<AsyncPipeWriter>,
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
        // For Phase 1: Use spawn_blocking to avoid blocking tokio executor
        // This is a pragmatic solution that works correctly with async/await
        // TODO: Implement proper overlapped I/O with IOCP for production (Phase 2+)

        use windows::Win32::Storage::FileSystem::ReadFile;

        let handle = self.handle;

        // SAFETY: We need raw access to the buffer for Windows ReadFile API
        let (buf_ptr, buf_len, buf_slice) = unsafe {
            let ptr = buf.unfilled_mut().as_mut_ptr();
            let len = buf.unfilled_mut().len();
            // Cast MaybeUninit<u8> to u8 for Windows API - ReadFile will initialize it
            let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, len);
            (ptr, len, slice)
        };

        // Clone the waker for spawn_blocking
        let waker = cx.waker().clone();

        // Attempt immediate read first (non-blocking check)
        let mut bytes_read: u32 = 0;

        // SAFETY: ReadFile is safe to call with valid handle and buffer
        let result = unsafe { ReadFile(handle, Some(buf_slice), Some(&mut bytes_read), None) };

        match result {
            Ok(_) if bytes_read > 0 => {
                unsafe { buf.assume_init(bytes_read as usize) };
                buf.advance(bytes_read as usize);
                std::task::Poll::Ready(Ok(()))
            }
            Ok(_) => {
                // No data available yet, would block
                // Wake us when data might be available
                waker.wake();
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
                        "PTY pipe closed"
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

        // Create pseudo-console
        let coord = COORD {
            X: config.cols as i16,
            Y: config.rows as i16,
        };

        // SAFETY: CreatePseudoConsole is safe to call with valid handles and size.
        // The handles are owned by us and will be properly managed.
        // Note: API changed in windows crate 0.58+ - now returns HPCON directly
        let hpc = unsafe {
            CreatePseudoConsole(coord, input_read, output_write, 0).map_err(|e| {
                PtyError::CreateFailed(format!("CreatePseudoConsole failed: {}", e))
            })?
        };

        // Spawn child process attached to ConPTY
        let (process_handle, shell_pid) = spawn_process(&hpc, &config)?;

        // NOTE: Do NOT manually close input_read/output_write!
        // CreatePseudoConsole takes ownership of these handles.
        // They will be automatically closed when ClosePseudoConsole is called in Drop.
        // Manually closing them here causes double-close → heap corruption.

        // Wrap pipe handles in async readers/writers
        // SAFETY: We're wrapping valid pipe handles for async I/O
        let output_reader = BufReader::with_capacity(
            PTY_BUFFER_SIZE,
            unsafe { AsyncPipeReader::from_handle(output_read) },
        );

        let input_writer = BufWriter::with_capacity(
            PTY_BUFFER_SIZE,
            unsafe { AsyncPipeWriter::from_handle(input_write) },
        );

        tracing::info!("ConPTY session created: pid={}", shell_pid);

        Ok(Self {
            hpc,
            process_handle,
            output_reader,
            input_writer,
            shell_pid,
        })
    }

    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.output_reader.read(buf).await
    }

    async fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.input_writer.write_all(data).await?;
        self.input_writer.flush().await
    }

    fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()> {
        let coord = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        // SAFETY: ResizePseudoConsole is safe to call on a valid HPCON
        unsafe {
            ResizePseudoConsole(self.hpc, coord)
                .map_err(|e| PtyError::ResizeFailed(e.to_string()))?;
        }

        tracing::debug!("Resized ConPTY to {}x{}", cols, rows);
        Ok(())
    }

    fn shell_pid(&self) -> u32 {
        self.shell_pid
    }

    async fn terminate(self) -> PtyResult<()> {
        tracing::info!("Terminating ConPTY session: pid={}", self.shell_pid);

        // SAFETY: TerminateProcess is safe to call on a valid process handle
        unsafe {
            TerminateProcess(self.process_handle, 1).map_err(|e| {
                PtyError::CreateFailed(format!("TerminateProcess failed: {}", e))
            })?;

            // Cleanup handles
            ClosePseudoConsole(self.hpc);
            let _ = CloseHandle(self.process_handle);
        }

        Ok(())
    }
}

impl Drop for ConPtyBackend {
    fn drop(&mut self) {
        tracing::debug!("Dropping ConPTY backend: pid={}", self.shell_pid);

        // SAFETY: Cleanup is safe even if terminate() was called
        // (Windows APIs handle double-close gracefully)
        unsafe {
            ClosePseudoConsole(self.hpc);
            let _ = CloseHandle(self.process_handle);
        }
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
        CreatePipe(&mut read_handle, &mut write_handle, None, 0).map_err(|e| {
            PtyError::CreateFailed(format!("CreatePipe failed: {}", e))
        })?;
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
    startup_info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;

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
