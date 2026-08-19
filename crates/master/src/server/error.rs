// Server error types

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),

    #[error("WebSocket upgrade failed: {0}")]
    WebSocketUpgrade(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Protocol decode error: {0}")]
    ProtocolDecode(#[from] prost::DecodeError),

    #[error("Protocol encode error: {0}")]
    ProtocolEncode(#[from] prost::EncodeError),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Internal error: {0}")]
    Internal(String),
}
