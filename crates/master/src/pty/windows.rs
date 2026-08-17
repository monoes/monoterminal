// Windows ConPTY Implementation with Async I/O
// SRS Reference: §2.1.2.3 Windows ConPTY [D1.2.3]
//
// Implements Windows Console Pseudo-console API (Windows 10 1809+) with tokio async I/O.
//
// Architecture:
// - Main thread: ConPTY setup via FFI
// - Background task 1: Read from ConPTY output pipe → send to mpsc channel
// - Background task 2: Receive from mpsc channel → write to ConPTY input pipe
// - Session Manager: Calls read()/write() on channels (no direct FFI)
//
// Safety: All unsafe FFI calls have documented safety invariants.

use super::{PtyError, PtyResult};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle, RawHandle};
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, GetProcessId, InitializeProcThreadAttributeList, TerminateProcess,
    UpdateProcThreadAttribute, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

/// 4KB buffer size per SRS §3.1.4
const PTY_BUFFER_SIZE: usize = 4096;

/// Channel buffer size (256 chunks = ~1MB total per SRS §2.1.4)
const CHANNEL_BUFFER_SIZE: usize = 256;

/// Handle wrapper with automatic cleanup
struct Handle(HANDLE);

impl Handle {
    fn new(handle: HANDLE) -> PtyResult<Self> {
        if handle == INVALID_HANDLE_VALUE || handle.0 == 0 {
            return Err(PtyError::CreateFailed("Invalid handle".to_string()));
        }
        Ok(Self(handle))
    }

    fn as_raw(&self) -> HANDLE {
        self.0
    }

    /// Transfer ownership of the raw handle, preventing Drop from closing it
    ///
    /// # Safety
    /// The caller must ensure the returned HANDLE is eventually closed,
    /// either manually or by transferring ownership to another wrapper.
    fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self); // Prevent Drop from closing the handle
        handle
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: We only close valid handles that we own
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// ConPTY handle wrapper with automatic cleanup
struct HpconHandle(HPCON);

impl Drop for HpconHandle {
    fn drop(&mut self) {
        // SAFETY: ClosePseudoConsole is safe to call on a valid HPCON
        unsafe {
            ClosePseudoConsole(self.0);
        }
    }
}

/// Wrapper for HANDLE that implements AsyncRead/AsyncWrite for tokio
///
/// Note: This is a simplified implementation for Phase 1.
/// For production, consider using a dedicated crate like `tokio-pipe` or
/// implementing proper overlapped I/O with IOCP.
struct AsyncHandle {
    inner: std::fs::File,
}

impl AsyncHandle {
    /// Create from raw Windows HANDLE
    ///
    /// # Safety
    /// The handle must be valid and owned by this wrapper.
    /// The handle will be closed when this wrapper is dropped.
    unsafe fn from_raw_handle(handle: HANDLE) -> PtyResult<Self> {
        let file = std::fs::File::from_raw_handle(handle.0 as RawHandle);
        Ok(Self { inner: file })
    }
}

impl tokio::io::AsyncRead for AsyncHandle {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Use tokio's blocking in place for synchronous reads
        // TODO: Implement proper overlapped I/O for production
        let inner = &mut self.inner;
        std::pin::Pin::new(inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for AsyncHandle {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let inner = &mut self.inner;
        std::pin::Pin::new(inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = &mut self.inner;
        std::pin::Pin::new(inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let inner = &mut self.inner;
        std::pin::Pin::new(inner).poll_shutdown(cx)
    }
}

/// Windows ConPTY handle with async I/O
///
/// Session Manager integration contract (from rust-backend-lead):
/// - spawn() to create sessions
/// - read() in a loop for output fan-out
/// - write() for client input
/// - resize() on client resize requests
/// - kill() on session termination
pub struct PtyHandle {
    /// Pseudo-console handle
    hpc: Arc<HpconHandle>,
    /// Child process handle
    process: Arc<Handle>,
    /// Process ID
    pid: u32,
    /// Output receiver (ConPTY → Session Manager)
    output_rx: mpsc::Receiver<Vec<u8>>,
    /// Input sender (Session Manager → ConPTY)
    input_tx: mpsc::Sender<Vec<u8>>,
    /// Output reader task handle (aborted on drop to prevent leaks)
    output_task: tokio::task::JoinHandle<()>,
    /// Input writer task handle (aborted on drop to prevent leaks)
    input_task: tokio::task::JoinHandle<()>,
}

impl PtyHandle {
    /// Spawn a new ConPTY session
    ///
    /// # Arguments
    /// * `shell` - Shell executable path (e.g., "powershell.exe", "cmd.exe")
    /// * `working_dir` - Initial working directory
    /// * `rows` - Terminal rows
    /// * `cols` - Terminal columns
    ///
    /// # Returns
    /// A new PtyHandle with background I/O tasks running
    pub async fn spawn(
        shell: &str,
        working_dir: &Path,
        rows: u16,
        cols: u16,
    ) -> PtyResult<Self> {
        tracing::info!(
            "Spawning ConPTY: shell={}, cwd={:?}, size={}x{}",
            shell,
            working_dir,
            cols,
            rows
        );

        // Create pipes for ConPTY I/O
        let (input_read, input_write) = Self::create_pipe()?;
        let (output_read, output_write) = Self::create_pipe()?;

        // Create pseudo-console
        let coord = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        let mut hpc = HPCON::default();

        // SAFETY: CreatePseudoConsole is safe to call with valid handles.
        // The handles are owned by us and will be managed appropriately.
        unsafe {
            CreatePseudoConsole(coord, input_read.as_raw(), output_write.as_raw(), 0, &mut hpc)
                .map_err(|e| {
                    PtyError::CreateFailed(format!("CreatePseudoConsole failed: {}", e))
                })?;
        }

        let hpc = Arc::new(HpconHandle(hpc));

        // Spawn child process attached to ConPTY
        let (process_handle, pid) = Self::spawn_process(&hpc.0, shell, working_dir)?;
        let process = Arc::new(Handle::new(process_handle)?);

        // Close ConPTY-owned pipe ends (ConPTY holds these now)
        drop(input_read);
        drop(output_write);

        // Create async I/O channels
        let (output_tx, output_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let (input_tx, input_rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);

        // Spawn output reader task (ConPTY → Session Manager)
        // SAFETY: output_read ownership transferred to AsyncHandle via into_raw()
        let output_read_async = unsafe { AsyncHandle::from_raw_handle(output_read.into_raw())? };

        let output_task = tokio::spawn(async move {
            Self::output_reader_task(output_read_async, output_tx).await;
        });

        // Spawn input writer task (Session Manager → ConPTY)
        // SAFETY: input_write ownership transferred to AsyncHandle via into_raw()
        let input_write_async = unsafe { AsyncHandle::from_raw_handle(input_write.into_raw())? };

        let input_task = tokio::spawn(async move {
            Self::input_writer_task(input_write_async, input_rx).await;
        });

        tracing::info!("ConPTY session spawned: pid={}", pid);

        Ok(Self {
            hpc,
            process,
            pid,
            output_rx,
            input_tx,
            output_task,
            input_task,
        })
    }

    /// Read output from the PTY (non-blocking)
    ///
    /// Returns None when the child process has terminated (EOF).
    /// Session Manager calls this in a loop for output fan-out.
    pub async fn read(&mut self) -> Option<Vec<u8>> {
        self.output_rx.recv().await
    }

    /// Write input to the PTY
    ///
    /// Session Manager forwards client input here.
    pub async fn write(&self, data: &[u8]) -> PtyResult<()> {
        self.input_tx
            .send(data.to_vec())
            .await
            .map_err(|_| PtyError::Disconnected)
    }

    /// Resize the terminal
    ///
    /// Session Manager calls this on client ResizeRequest messages.
    pub fn resize(&self, rows: u16, cols: u16) -> PtyResult<()> {
        let coord = COORD {
            X: cols as i16,
            Y: rows as i16,
        };

        // SAFETY: ResizePseudoConsole is safe to call on a valid HPCON
        unsafe {
            ResizePseudoConsole(self.hpc.0, coord).map_err(|e| {
                PtyError::ResizeFailed(format!("ResizePseudoConsole failed: {}", e).into())
            })?;
        }

        tracing::debug!("Resized ConPTY to {}x{}", cols, rows);
        Ok(())
    }

    /// Kill the child process
    ///
    /// Session Manager calls this on session termination.
    pub fn kill(&self) -> PtyResult<()> {
        tracing::warn!("Force-killing ConPTY process: pid={}", self.pid);

        // SAFETY: TerminateProcess is safe to call on a valid process handle
        unsafe {
            TerminateProcess(self.process.as_raw(), 1).map_err(|e| {
                PtyError::CreateFailed(format!("TerminateProcess failed: {}", e))
            })?;
        }

        Ok(())
    }

    /// Get the child process ID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    // ========== Internal Helper Methods ==========

    /// Create a pipe for ConPTY I/O
    fn create_pipe() -> PtyResult<(Handle, Handle)> {
        let mut read_handle = HANDLE::default();
        let mut write_handle = HANDLE::default();

        // SAFETY: CreatePipe is safe to call with valid out-pointers.
        // We pass NULL for security attributes (default) and 0 for buffer size (default).
        unsafe {
            CreatePipe(&mut read_handle, &mut write_handle, None, 0).map_err(|e| {
                PtyError::CreateFailed(format!("CreatePipe failed: {}", e))
            })?;
        }

        Ok((Handle::new(read_handle)?, Handle::new(write_handle)?))
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
    fn spawn_process(hpc: &HPCON, shell: &str, working_dir: &Path) -> PtyResult<(HANDLE, u32)> {
        let mut command_line = Self::to_wide_string(shell);

        let cwd = working_dir
            .to_str()
            .ok_or_else(|| PtyError::InvalidConfig("Invalid working directory".to_string()))?;
        let cwd_wide = Self::to_wide_string(cwd);

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
        let attr_list_ptr =
            LPPROC_THREAD_ATTRIBUTE_LIST(attr_list_buffer.as_mut_ptr() as *mut _);

        unsafe {
            InitializeProcThreadAttributeList(attr_list_ptr, 1, 0, &mut attr_size).map_err(
                |e| PtyError::SpawnFailed(format!("InitializeProcThreadAttributeList failed: {}", e)),
            )?;
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
            .map_err(|e| {
                PtyError::SpawnFailed(format!("UpdateProcThreadAttribute failed: {}", e))
            })?;
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

    /// Background task: Read from ConPTY output pipe and send to channel
    ///
    /// Runs until EOF (process terminated) or error.
    async fn output_reader_task(mut reader: AsyncHandle, tx: mpsc::Sender<Vec<u8>>) {
        let mut buffer = vec![0u8; PTY_BUFFER_SIZE];

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    // EOF - process terminated
                    tracing::debug!("ConPTY output EOF (process terminated)");
                    break;
                }
                Ok(n) => {
                    let chunk = buffer[..n].to_vec();
                    if tx.send(chunk).await.is_err() {
                        // Receiver dropped - session closed
                        tracing::debug!("ConPTY output receiver dropped");
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("ConPTY read error: {}", e);
                    break;
                }
            }
        }

        tracing::debug!("ConPTY output reader task exiting");
    }

    /// Background task: Receive from channel and write to ConPTY input pipe
    ///
    /// Runs until channel is closed or error.
    async fn input_writer_task(mut writer: AsyncHandle, mut rx: mpsc::Receiver<Vec<u8>>) {
        while let Some(data) = rx.recv().await {
            if let Err(e) = writer.write_all(&data).await {
                tracing::error!("ConPTY write error: {}", e);
                break;
            }

            // Flush immediately for low-latency input
            let _ = writer.flush().await;
        }

        tracing::debug!("ConPTY input writer task exiting");
    }
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        // Abort background I/O tasks immediately to prevent resource leaks
        // This ensures Arc<Handle> and Arc<HpconHandle> are dropped promptly
        // instead of waiting for natural EOF, preventing memory leaks in
        // rapid session creation/termination scenarios (e.g., soak tests)
        self.output_task.abort();
        self.input_task.abort();
        tracing::debug!("PtyHandle dropped: aborted I/O tasks for pid={}", self.pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[tokio::test]
    async fn test_spawn_conpty() {
        let pty = PtyHandle::spawn("cmd.exe", Path::new("C:\\"), 24, 80)
            .await
            .expect("Failed to spawn ConPTY");

        assert!(pty.pid() > 0);
        assert_eq!(pty.pid, pty.pid());
    }

    #[tokio::test]
    async fn test_spawn_powershell() {
        let pty = PtyHandle::spawn("powershell.exe", Path::new("C:\\"), 24, 80)
            .await
            .expect("Failed to spawn PowerShell");

        assert!(pty.pid() > 0);

        // Clean up
        let _ = pty.kill();
    }

    #[tokio::test]
    async fn test_write_read_cmd() {
        let mut pty = PtyHandle::spawn("cmd.exe", Path::new("C:\\"), 24, 80)
            .await
            .expect("Failed to spawn cmd.exe");

        // Send echo command
        pty.write(b"echo hello\r\n")
            .await
            .expect("Failed to write");

        // Read output (should contain "hello" somewhere)
        let mut found = false;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(100)).await;

            if let Some(output) = pty.read().await {
                let text = String::from_utf8_lossy(&output);
                tracing::debug!("Received: {}", text);

                if text.contains("hello") {
                    found = true;
                    break;
                }
            }
        }

        assert!(found, "Expected 'hello' in output");

        // Clean up
        let _ = pty.kill();
    }

    #[tokio::test]
    async fn test_resize() {
        let pty = PtyHandle::spawn("cmd.exe", Path::new("C:\\"), 24, 80)
            .await
            .expect("Failed to spawn cmd.exe");

        // Resize should not fail
        pty.resize(30, 100).expect("Failed to resize");
        pty.resize(50, 120).expect("Failed to resize again");

        // Clean up
        let _ = pty.kill();
    }

    #[tokio::test]
    async fn test_kill() {
        let pty = PtyHandle::spawn("cmd.exe", Path::new("C:\\"), 24, 80)
            .await
            .expect("Failed to spawn cmd.exe");

        let pid = pty.pid();
        assert!(pid > 0);

        // Kill should succeed
        pty.kill().expect("Failed to kill process");

        // Give it a moment to terminate
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
