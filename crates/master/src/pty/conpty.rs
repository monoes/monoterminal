// Windows ConPTY Backend Implementation
// SRS Reference: §2.1.2.3 Windows ConPTY [D1.2.3]
//
// Implements the PtyBackend trait for Windows using portable-pty, which wraps
// the Console Pseudo-console API (Windows 10 1809+) internally.
//
// The hand-rolled Win32 FFI version of this file (raw CreatePipe/CreatePseudoConsole/
// CreateProcessW plumbing with a PeekNamedPipe-based AsyncRead) had a real, unresolved
// bug: PeekNamedPipe on the ConPTY output pipe reported 0 bytes available forever, even
// with a live, CPU-active shell process that had just received input — output never
// reached the client despite the session "attaching" successfully. That code also left
// behind a trail of leftover diagnostic tracing (search git history) from a previous
// engineer who found the same symptom and left the exact test (`test_write_read`,
// see below) permanently `#[ignore]`d rather than fixed.
//
// portable-pty is already used successfully for the Unix backend (see unix.rs) and its
// `NativePtySystem` supports Windows via ConPTY internally too — this file mirrors
// unix.rs's proven spawn_blocking-based async wrapper instead of re-implementing
// low-level Win32 pipe/IOCP plumbing by hand.

use super::{
    error::{PtyError, PtyResult},
    PtyBackend, PtyConfig,
};
use async_trait::async_trait;
use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::future::Future;
use std::io::{self, Read as StdRead, Write as StdWrite};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

/// 4KB buffer size per SRS §3.1.4
const PTY_BUFFER_SIZE: usize = 4096;

/// Windows ConPTY backend using portable-pty
///
/// Implements PtyBackend trait for Windows using portable-pty's native ConPTY support.
/// Session Manager calls methods on this struct via `Box<dyn PtyBackend>`.
pub struct ConPtyBackend {
    /// PTY pair (master + slave) — kept alive for resize() and to hold ConPTY open
    pty_pair: Arc<Mutex<PtyPair>>,

    /// Child process handle
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,

    /// Buffered output reader (ConPTY → Session Manager)
    output_reader: BufReader<PtyReader>,

    /// Direct input writer (Session Manager → ConPTY) - unbuffered for immediate delivery
    input_writer: PtyWriter,

    /// Shell process ID
    shell_pid: u32,
}

// SAFETY: portable-pty types are Send + Sync when wrapped in Arc<Mutex<>>
unsafe impl Send for ConPtyBackend {}
unsafe impl Sync for ConPtyBackend {}

/// Async wrapper around portable-pty reader
///
/// Implements tokio::io::AsyncRead via spawn_blocking, since portable-pty's
/// Read/Write handles are synchronous.
///
/// The in-flight spawn_blocking task is stored on the struct (`pending`)
/// rather than recreated on every poll_read call. A prior version spawned a
/// brand-new blocking task from scratch on each poll, discarding the
/// previous attempt's JoinHandle future the moment poll_read returned
/// Pending. Nothing ever kept that discarded future alive to observe its
/// completion, and — since `reader` is behind a Mutex — a still-running
/// first read (blocked waiting for real PTY output) held the lock while
/// every subsequent poll spawned ANOTHER task that immediately blocked
/// trying to acquire the same lock. Net effect: a hard, permanent hang the
/// instant a read didn't resolve on its very first poll — confirmed by a
/// standalone repro that isolated this exact struct as the only difference
/// between a working plain-synchronous portable-pty read and a hanging one.
struct PtyReader {
    reader: Arc<Mutex<Box<dyn StdRead + Send>>>,
    pending: Option<tokio::task::JoinHandle<(io::Result<usize>, Vec<u8>)>>,
}

struct PtyWriter {
    writer: Arc<Mutex<Box<dyn StdWrite + Send>>>,
    pending_write: Option<tokio::task::JoinHandle<io::Result<usize>>>,
    pending_flush: Option<tokio::task::JoinHandle<io::Result<()>>>,
}

// SAFETY: Wrapped in Arc<Mutex<>> for thread-safety
unsafe impl Send for PtyReader {}
unsafe impl Sync for PtyReader {}
unsafe impl Send for PtyWriter {}
unsafe impl Sync for PtyWriter {}

impl PtyReader {
    fn new(reader: Box<dyn StdRead + Send>) -> Self {
        Self {
            reader: Arc::new(Mutex::new(reader)),
            pending: None,
        }
    }
}

impl PtyWriter {
    fn new(writer: Box<dyn StdWrite + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            pending_write: None,
            pending_flush: None,
        }
    }
}

// Implement tokio AsyncRead for PtyReader
impl tokio::io::AsyncRead for PtyReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();

        if this.pending.is_none() {
            let reader = this.reader.clone();
            let buf_len = buf.remaining();
            this.pending = Some(tokio::task::spawn_blocking(move || {
                let mut temp_buf = vec![0u8; buf_len];
                let mut r = reader.lock().unwrap();
                let result = r.read(&mut temp_buf);
                (result, temp_buf)
            }));
        }

        let handle = this.pending.as_mut().unwrap();
        match std::pin::Pin::new(handle).poll(cx) {
            std::task::Poll::Ready(Ok((Ok(n), temp_buf))) => {
                this.pending = None;
                buf.put_slice(&temp_buf[..n]);
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Ready(Ok((Err(e), _))) => {
                this.pending = None;
                std::task::Poll::Ready(Err(e))
            }
            std::task::Poll::Ready(Err(e)) => {
                this.pending = None;
                std::task::Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

// Implement tokio AsyncWrite for PtyWriter
impl tokio::io::AsyncWrite for PtyWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        let this = self.get_mut();

        if this.pending_write.is_none() {
            let writer = this.writer.clone();
            let data = buf.to_vec();
            this.pending_write = Some(tokio::task::spawn_blocking(move || {
                let mut w = writer.lock().unwrap();
                w.write(&data)
            }));
        }

        let handle = this.pending_write.as_mut().unwrap();
        match std::pin::Pin::new(handle).poll(cx) {
            std::task::Poll::Ready(Ok(Ok(n))) => {
                this.pending_write = None;
                std::task::Poll::Ready(Ok(n))
            }
            std::task::Poll::Ready(Ok(Err(e))) => {
                this.pending_write = None;
                std::task::Poll::Ready(Err(e))
            }
            std::task::Poll::Ready(Err(e)) => {
                this.pending_write = None;
                std::task::Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let this = self.get_mut();

        if this.pending_flush.is_none() {
            let writer = this.writer.clone();
            this.pending_flush = Some(tokio::task::spawn_blocking(move || {
                let mut w = writer.lock().unwrap();
                w.flush()
            }));
        }

        let handle = this.pending_flush.as_mut().unwrap();
        match std::pin::Pin::new(handle).poll(cx) {
            std::task::Poll::Ready(Ok(Ok(()))) => {
                this.pending_flush = None;
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Ready(Ok(Err(e))) => {
                this.pending_flush = None;
                std::task::Poll::Ready(Err(e))
            }
            std::task::Poll::Ready(Err(e)) => {
                this.pending_flush = None;
                std::task::Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

#[async_trait]
impl PtyBackend for ConPtyBackend {
    /// Create a new ConPTY session with the given configuration
    async fn create(config: PtyConfig) -> PtyResult<Self>
    where
        Self: Sized,
    {
        tracing::info!(
            "Creating ConPTY: shell={}, cwd={:?}, {}x{}",
            config.shell,
            config.working_dir,
            config.rows,
            config.cols
        );

        // Get native PTY system (Windows: ConPTY via portable-pty)
        let pty_system = NativePtySystem::default();

        // Create PTY pair with requested dimensions
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,  // Not used
                pixel_height: 0, // Not used
            })
            .map_err(|e| PtyError::CreateFailed(format!("openpty failed: {}", e)))?;

        // Build command from config
        let mut cmd = CommandBuilder::new(&config.shell);
        cmd.cwd(config.working_dir);

        // Set environment variables
        for (key, val) in config.environment {
            cmd.env(key, val);
        }

        // Spawn child process attached to ConPTY
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::CreateFailed(format!("spawn_command failed: {}", e)))?;

        let shell_pid = child.process_id().unwrap_or(0);
        tracing::info!("ConPTY created: pid={}", shell_pid);

        // Get master I/O handles
        let master_reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::CreateFailed(format!("clone reader failed: {}", e)))?;

        let master_writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| PtyError::CreateFailed(format!("take writer failed: {}", e)))?;

        // Wrap I/O in async readers/writers with 4KB buffer (SRS §3.1.4)
        let output_reader =
            BufReader::with_capacity(PTY_BUFFER_SIZE, PtyReader::new(master_reader));

        let input_writer = PtyWriter::new(master_writer);

        Ok(ConPtyBackend {
            pty_pair: Arc::new(Mutex::new(pty_pair)),
            child: Arc::new(Mutex::new(child)),
            output_reader,
            input_writer,
            shell_pid,
        })
    }

    /// Read output from the PTY (non-blocking)
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.output_reader.read(buf).await
    }

    /// Write input to the PTY (flushes immediately)
    async fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.input_writer.write_all(data).await?;
        self.input_writer.flush().await?;
        Ok(())
    }

    /// Resize the PTY to new dimensions
    fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()> {
        let pty_pair = self.pty_pair.lock().unwrap();
        pty_pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::ResizeFailed(format!("resize failed: {}", e)))?;
        Ok(())
    }

    /// Get the shell process ID
    fn shell_pid(&self) -> u32 {
        self.shell_pid
    }

    /// Terminate the PTY session (kill process and cleanup)
    async fn terminate(self: Box<Self>) -> PtyResult<()> {
        tracing::info!("Terminating ConPTY: pid={}", self.shell_pid);

        // Kill child process
        {
            let mut child = self.child.lock().unwrap();
            child
                .kill()
                .map_err(|e| PtyError::TerminateFailed(format!("kill failed: {}", e)))?;
        }

        // Wait for process exit (with timeout)
        let child_clone = self.child.clone();
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                let mut child = child_clone.lock().unwrap();
                child.wait()
            }),
        )
        .await
        {
            Ok(Ok(Ok(_))) => {
                tracing::info!("ConPTY terminated successfully: pid={}", self.shell_pid);
                Ok(())
            }
            Ok(Ok(Err(e))) => Err(PtyError::TerminateFailed(format!("wait failed: {}", e))),
            Ok(Err(e)) => Err(PtyError::TerminateFailed(format!(
                "spawn_blocking failed: {}",
                e
            ))),
            Err(_) => {
                tracing::warn!("ConPTY terminate timeout: pid={}", self.shell_pid);
                Ok(()) // Continue cleanup even on timeout
            }
        }
    }
}

impl Drop for ConPtyBackend {
    fn drop(&mut self) {
        // Best-effort cleanup
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
        tracing::debug!("ConPTY dropped: pid={}", self.shell_pid);
    }
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
        Box::new(backend).terminate().await.ok();
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
        Box::new(backend).terminate().await.ok();
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
        Box::new(backend).terminate().await.ok();
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
        Box::new(backend).terminate().await.expect("Failed to terminate");
    }
}
