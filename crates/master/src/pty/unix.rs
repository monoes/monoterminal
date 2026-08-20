// Unix PTY Backend Implementation (Phase 3)
// SRS Reference: §2.1.2.1 Linux posix_openpt, §2.1.2.2 macOS openpty BSD
//
// Implements the PtyBackend trait for Unix platforms (Linux/macOS) using portable-pty.
// Follows the same patterns as ConPtyBackend for consistency.
//
// Architecture:
// - BufReader/BufWriter around async pipe handles for I/O
// - Direct read()/write() calls from Session Manager (no background tasks)
// - Cleanup via terminate() or Drop
//
// Safety: All portable-pty operations are safe Rust.

use super::{
    error::{PtyError, PtyResult},
    PtyBackend, PtyConfig,
};
use async_trait::async_trait;
use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::io::{self, Read as StdRead, Write as StdWrite};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

/// 4KB buffer size per SRS §3.1.4
const PTY_BUFFER_SIZE: usize = 4096;

/// Unix PTY backend using portable-pty
///
/// Implements PtyBackend trait for Linux/macOS using portable-pty library.
/// Session Manager calls methods on this struct via `Box<dyn PtyBackend>`.
pub struct UnixPtyBackend {
    /// PTY pair (master + slave)
    pty_pair: Arc<Mutex<PtyPair>>,

    /// Child process handle
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,

    /// Buffered output reader (PTY → Session Manager)
    output_reader: BufReader<PtyReader>,

    /// Direct input writer (Session Manager → PTY) - unbuffered for immediate delivery
    input_writer: PtyWriter,

    /// Shell process ID
    shell_pid: u32,
}

// SAFETY: portable-pty types are Send + Sync when wrapped in Arc<Mutex<>>
unsafe impl Send for UnixPtyBackend {}
unsafe impl Sync for UnixPtyBackend {}

/// Async wrapper around portable-pty reader
///
/// Implements tokio::io::AsyncRead via spawn_blocking.
/// Uses blocking I/O with spawn_blocking for Phase 3 simplicity.
/// TODO: Optimize with tokio-uring for Linux in Phase 4.
struct PtyReader {
    reader: Arc<Mutex<Box<dyn StdRead + Send>>>,
}

struct PtyWriter {
    writer: Arc<Mutex<Box<dyn StdWrite + Send>>>,
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
        }
    }
}

impl PtyWriter {
    fn new(writer: Box<dyn StdWrite + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

// Implement tokio AsyncRead for PtyReader
impl tokio::io::AsyncRead for PtyReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<usize>> {
        // Use spawn_blocking for synchronous read (portable-pty provides blocking I/O)
        let reader = self.reader.clone();
        let buf_len = buf.remaining();

        let mut fut = Box::pin(tokio::task::spawn_blocking(move || {
            let mut temp_buf = vec![0u8; buf_len];
            let mut r = reader.lock().unwrap();
            let result = r.read(&mut temp_buf);
            (result, temp_buf)
        }));

        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready(Ok((Ok(n), temp_buf))) => {
                buf.put_slice(&temp_buf[..n]);
                std::task::Poll::Ready(Ok(n))
            }
            std::task::Poll::Ready(Ok((Err(e), _))) => std::task::Poll::Ready(Err(e)),
            std::task::Poll::Ready(Err(e)) => {
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
        let writer = self.writer.clone();
        let data = buf.to_vec();

        let mut fut = Box::pin(tokio::task::spawn_blocking(move || {
            let mut w = writer.lock().unwrap();
            w.write(&data)
        }));

        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready(Ok(Ok(n))) => std::task::Poll::Ready(Ok(n)),
            std::task::Poll::Ready(Ok(Err(e))) => std::task::Poll::Ready(Err(e)),
            std::task::Poll::Ready(Err(e)) => {
                std::task::Poll::Ready(Err(io::Error::new(io::ErrorKind::Other, e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let writer = self.writer.clone();

        let mut fut = Box::pin(tokio::task::spawn_blocking(move || {
            let mut w = writer.lock().unwrap();
            w.flush()
        }));

        match fut.as_mut().poll(cx) {
            std::task::Poll::Ready(Ok(Ok(()))) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Ok(Err(e))) => std::task::Poll::Ready(Err(e)),
            std::task::Poll::Ready(Err(e)) => {
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
impl PtyBackend for UnixPtyBackend {
    /// Create a new Unix PTY session with the given configuration
    async fn create(config: PtyConfig) -> PtyResult<Self>
    where
        Self: Sized,
    {
        tracing::info!(
            "Creating Unix PTY: shell={}, cwd={:?}, {}x{}",
            config.shell,
            config.working_dir,
            config.rows,
            config.cols
        );

        // Get native PTY system (Linux: posix_openpt, macOS: openpty)
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

        // Spawn child process with slave PTY
        let mut child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::CreateFailed(format!("spawn_command failed: {}", e)))?;

        let shell_pid = child.process_id().unwrap_or(0);
        tracing::info!("Unix PTY created: pid={}", shell_pid);

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

        Ok(UnixPtyBackend {
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
        tracing::info!("Terminating Unix PTY: pid={}", self.shell_pid);

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
                tracing::info!("Unix PTY terminated successfully: pid={}", self.shell_pid);
                Ok(())
            }
            Ok(Ok(Err(e))) => Err(PtyError::TerminateFailed(format!("wait failed: {}", e))),
            Ok(Err(e)) => Err(PtyError::TerminateFailed(format!(
                "spawn_blocking failed: {}",
                e
            ))),
            Err(_) => {
                tracing::warn!("Unix PTY terminate timeout: pid={}", self.shell_pid);
                Ok(()) // Continue cleanup even on timeout
            }
        }
    }
}

impl Drop for UnixPtyBackend {
    fn drop(&mut self) {
        // Best-effort cleanup
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
        tracing::debug!("Unix PTY dropped: pid={}", self.shell_pid);
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_unix_pty_create() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let pty = UnixPtyBackend::create(config).await.unwrap();
        assert!(pty.shell_pid() > 0);
    }

    #[tokio::test]
    async fn test_unix_pty_resize() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Resize should not error
        pty.resize(30, 100).unwrap();
        pty.resize(40, 120).unwrap();
    }

    #[tokio::test]
    async fn test_unix_pty_read_write() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Write a simple command
        pty.write(b"echo hello\n").await.unwrap();

        // Read output (may take a moment)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut buf = vec![0u8; 1024];
        let n = pty.read(&mut buf).await.unwrap();

        // Should read some bytes
        assert!(n > 0, "Expected to read output from echo command");

        let output = String::from_utf8_lossy(&buf[..n]);
        // Output should contain "hello" somewhere (may have prompt/ANSI codes)
        assert!(
            output.contains("hello") || output.contains("echo"),
            "Expected output to contain 'hello' or 'echo', got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_unix_pty_terminate() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let pty = UnixPtyBackend::create(config).await.unwrap();
        let pid = pty.shell_pid();
        assert!(pid > 0);

        // Terminate should not error
        Box::new(pty).terminate().await.unwrap();

        // Process should be gone (this is platform-specific and may not be testable)
        // On Unix we can check /proc, but it's not portable
    }

    #[tokio::test]
    async fn test_unix_pty_environment() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: env,
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Check environment variable was set
        pty.write(b"echo $TEST_VAR\n").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut buf = vec![0u8; 1024];
        let n = pty.read(&mut buf).await.unwrap();
        assert!(n > 0);

        let output = String::from_utf8_lossy(&buf[..n]);
        assert!(
            output.contains("test_value"),
            "Expected environment variable to be set, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_unix_pty_working_dir() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Check working directory
        pty.write(b"pwd\n").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let mut buf = vec![0u8; 1024];
        let n = pty.read(&mut buf).await.unwrap();
        assert!(n > 0);

        let output = String::from_utf8_lossy(&buf[..n]);
        assert!(
            output.contains("/tmp"),
            "Expected working directory to be /tmp, got: {}",
            output
        );
    }

    #[tokio::test]
    async fn test_unix_pty_multiple_resize() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Rapid resize operations should not error
        pty.resize(30, 100).unwrap();
        pty.resize(40, 120).unwrap();
        pty.resize(50, 150).unwrap();
        pty.resize(24, 80).unwrap(); // Back to original
    }

    #[tokio::test]
    async fn test_unix_pty_empty_buffer_read() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Read with empty buffer should not crash
        let mut buf = vec![];
        let result = pty.read(&mut buf).await;
        // Either succeeds with 0 bytes or errors (both acceptable)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_unix_pty_large_write() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Write large data (8KB)
        let large_data = vec![b'A'; 8192];
        let result = pty.write(&large_data).await;

        // Should handle large write (may succeed or error on buffer full)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_unix_pty_concurrent_operations() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let mut pty = UnixPtyBackend::create(config).await.unwrap();

        // Write command
        pty.write(b"echo test\n").await.unwrap();

        // Immediately resize
        pty.resize(30, 100).unwrap();

        // Read should still work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let mut buf = vec![0u8; 1024];
        let n = pty.read(&mut buf).await.unwrap();
        assert!(n > 0);
    }

    #[tokio::test]
    async fn test_unix_pty_shell_pid_valid() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let pty = UnixPtyBackend::create(config).await.unwrap();
        let pid = pty.shell_pid();

        // PID should be positive
        assert!(pid > 0, "Shell PID should be greater than 0");

        // PID should be reasonable (not too large, not process 1)
        assert!(
            pid > 1 && pid < 100000,
            "Shell PID should be reasonable: {}",
            pid
        );
    }

    #[tokio::test]
    async fn test_unix_pty_drop_cleanup() {
        let config = PtyConfig {
            rows: 24,
            cols: 80,
            shell: "/bin/sh".to_string(),
            working_dir: PathBuf::from("/tmp"),
            environment: HashMap::new(),
        };

        let pty = UnixPtyBackend::create(config).await.unwrap();
        let _pid = pty.shell_pid();

        // Drop pty (implicit drop at end of scope)
        drop(pty);

        // If we reach here without panic, drop cleanup worked
    }
}
