// PTY Error Types
// SRS Reference: §2.1.2 PTY Management [D1.2]

use std::io;
use thiserror::Error;

/// Errors that can occur during PTY operations
#[derive(Error, Debug)]
pub enum PtyError {
    /// Failed to create the pseudo-console
    #[error("Failed to create pseudo-console: {0}")]
    CreateFailed(String),

    /// Failed to spawn the child process
    #[error("Failed to spawn child process: {0}")]
    SpawnFailed(String),

    /// Failed to resize the PTY
    #[error("Failed to resize PTY: {0}")]
    ResizeFailed(String),

    /// I/O error during read/write operations
    #[error("PTY I/O error: {0}")]
    Io(#[from] io::Error),

    /// The child process has already exited
    #[error("Child process has exited")]
    ProcessExited,

    /// The PTY session is already closed
    #[error("PTY session is already closed")]
    AlreadyClosed,

    /// Invalid configuration
    #[error("Invalid PTY configuration: {0}")]
    InvalidConfig(String),

    /// Platform-specific error
    #[cfg(windows)]
    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),

    /// Timeout waiting for process operation
    #[error("Timeout waiting for process: {0}")]
    Timeout(String),

    /// PTY disconnected (channel closed)
    #[error("PTY disconnected")]
    Disconnected,
}

/// Result type for PTY operations
pub type PtyResult<T> = Result<T, PtyError>;
