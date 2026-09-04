// Clipboard Error Types
// Phase 4: Bidirectional Clipboard (ADR-020)

use thiserror::Error;

/// Clipboard operation errors
#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Rate limit exceeded: retry after {retry_after} seconds")]
    RateLimitExceeded { retry_after: u64 },

    #[error("Clipboard content size exceeds 1MB limit")]
    SizeLimitExceeded,

    #[error("Authorization required for clipboard access")]
    AuthorizationRequired,

    #[error("Authorization denied by user")]
    AuthorizationDenied,

    #[error("System clipboard access failed: {0}")]
    SystemClipboardError(String),

    #[error("Invalid clipboard request: {0}")]
    InvalidRequest(String),
}

pub type Result<T> = std::result::Result<T, ClipboardError>;
