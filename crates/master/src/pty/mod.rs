// PTY (Pseudo-Terminal) Management
// Platform-specific implementations for terminal session management
//
// SRS Reference: §2.1.2 PTY Management [D1.2]
// Architecture: Trait-based design for cross-platform support (finalized per rust-backend-lead)
//
// Phase 1: Windows ConPTY (§2.1.2.3 [D1.2.3])
// Phase 3: Linux posix_openpt (§2.1.2.1 [D1.2.1])
// Phase 3: macOS openpty BSD (§2.1.2.2 [D1.2.2])

pub mod error;

// Platform-specific PTY implementations
#[cfg(windows)]
pub mod conpty;

#[cfg(unix)]
pub mod unix;

// Platform-specific backend exports
#[cfg(windows)]
pub use conpty::ConPtyBackend;

#[cfg(unix)]
pub use unix::UnixPtyBackend;

// Export PtyResult for test access (per rust-backend-lead guidance)
pub use error::PtyResult;

#[cfg(test)]
mod tests;

use async_trait::async_trait;
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

/// PTY configuration for creating new sessions
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Terminal rows
    pub rows: u16,
    /// Terminal columns
    pub cols: u16,
    /// Shell executable path (e.g., "powershell.exe", "bash")
    pub shell: String,
    /// Working directory for the shell process
    pub working_dir: PathBuf,
    /// Environment variables for the shell process
    pub environment: HashMap<String, String>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            shell: Self::default_shell(),
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            environment: HashMap::new(),
        }
    }
}

impl PtyConfig {
    /// Get the default shell for the current platform
    #[cfg(windows)]
    fn default_shell() -> String {
        "powershell.exe".to_string()
    }

    #[cfg(not(windows))]
    fn default_shell() -> String {
        "/bin/bash".to_string()
    }
}

/// Platform-agnostic PTY backend trait
///
/// Session Manager uses this trait via `Box<dyn PtyBackend>` for dynamic dispatch.
/// Phase 1: ConPtyBackend (Windows)
/// Phase 3: UnixPtyBackend (Linux/macOS)
#[async_trait]
pub trait PtyBackend: Send + Sync {
    /// Create a new PTY session with the given configuration
    async fn create(config: PtyConfig) -> PtyResult<Self>
    where
        Self: Sized;

    /// Read output from the PTY (non-blocking)
    ///
    /// Returns the number of bytes read into the buffer.
    /// Returns Ok(0) when EOF (process terminated).
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    /// Write input to the PTY
    ///
    /// Flushes immediately for low-latency input.
    async fn write(&mut self, data: &[u8]) -> io::Result<()>;

    /// Resize the PTY to new dimensions
    fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()>;

    /// Get the shell process ID
    fn shell_pid(&self) -> u32;

    /// Terminate the PTY session (kill process and cleanup)
    /// Takes Box<Self> to support dynamic dispatch with trait objects
    async fn terminate(self: Box<Self>) -> PtyResult<()>;
}
