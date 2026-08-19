// Session Management Module
// Phase 1: Windows + Web MVP
// SRS §2.1.3, Architecture: docs/architecture/phase1-overview.md §2

pub mod session;
pub mod scrollback;
pub mod manager;

pub use session::{Session, SessionId, SessionState, SessionSnapshot, ClientId, SessionContainer};
pub use manager::SessionManager;

#[cfg(test)]
mod tests;

use thiserror::Error;

/// Session management errors
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(SessionId),

    #[error("PTY creation failed: {0}")]
    PtyCreateFailed(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Invalid dimensions: rows={0}, cols={1}")]
    InvalidDimensions(u16, u16),

    #[error("PTY error: {0}")]
    PtyError(#[from] crate::pty::error::PtyError),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SessionError>;
