// WebRTC error types
// ADR-011: P2P Networking Architecture

use thiserror::Error;

pub type Result<T> = std::result::Result<T, WebRtcError>;

#[derive(Error, Debug)]
pub enum WebRtcError {
    #[error("WebRTC initialization failed: {0}")]
    InitializationFailed(String),

    #[error("ICE candidate gathering failed: {0}")]
    IceGatheringFailed(String),

    #[error("ICE candidate gathering timeout after {0}s")]
    IceGatheringTimeout(u64),

    #[error("Peer connection failed: {0}")]
    PeerConnectionFailed(String),

    #[error("Data channel creation failed: {0}")]
    DataChannelCreationFailed(String),

    #[error("Data channel closed")]
    DataChannelClosed,

    #[error("SDP offer/answer failed: {0}")]
    SdpNegotiationFailed(String),

    #[error("STUN server unreachable: {0}")]
    StunServerUnreachable(String),

    #[error("TURN server unreachable: {0}")]
    TurnServerUnreachable(String),

    #[error("PeerHandshake verification failed: {0}")]
    HandshakeVerificationFailed(String),

    #[error("Ed25519 signature invalid")]
    InvalidSignature,

    #[error("Challenge expired (timestamp delta: {0}ms)")]
    ChallengeExpired(i64),

    #[error("Protocol version mismatch: got {got}, expected {expected}")]
    ProtocolVersionMismatch { got: u32, expected: u32 },

    #[error("WebRTC not supported on this platform")]
    PlatformNotSupported,

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] prost::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<webrtc::Error> for WebRtcError {
    fn from(err: webrtc::Error) -> Self {
        WebRtcError::Internal(err.to_string())
    }
}
