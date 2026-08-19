// Server error types

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(Box<std::io::Error>),

    #[error("TLS handshake failed: {0}")]
    TlsHandshake(String),

    #[error("WebSocket upgrade failed: {0}")]
    WebSocketUpgrade(String),

    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),

    #[error("Protocol decode error: {0}")]
    ProtocolDecode(Box<prost::DecodeError>),

    #[error("Protocol encode error: {0}")]
    ProtocolEncode(Box<prost::EncodeError>),

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

// Manual From implementations for boxed error types
impl From<std::io::Error> for ServerError {
    fn from(err: std::io::Error) -> Self {
        ServerError::Io(Box::new(err))
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ServerError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        ServerError::WebSocket(Box::new(err))
    }
}

impl From<prost::DecodeError> for ServerError {
    fn from(err: prost::DecodeError) -> Self {
        ServerError::ProtocolDecode(Box::new(err))
    }
}

impl From<prost::EncodeError> for ServerError {
    fn from(err: prost::EncodeError) -> Self {
        ServerError::ProtocolEncode(Box::new(err))
    }
}
