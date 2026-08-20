// Session Management Module
// Phase 1: Windows + Web MVP
// SRS §2.1.3, Architecture: docs/architecture/phase1-overview.md §2

#![allow(clippy::module_inception)]
#![allow(dead_code)] // Session features not all integrated yet, cleanup tracked in task-63

pub mod manager;
pub mod scrollback;
pub mod session;

pub use manager::SessionManager;
pub use session::{ClientId, Session, SessionContainer, SessionId, SessionSnapshot, SessionState};

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

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

pub type Result<T> = std::result::Result<T, SessionError>;
