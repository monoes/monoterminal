use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    Register {
        peer_id: String,
        /// Untyped: the daemon's handshake payload shape (protocol_version,
        /// timestamp_ms, signature bytes, ...) isn't verified here, only its
        /// `peer_id` field is checked against the top-level `peer_id`.
        handshake: serde_json::Value,
    },
    Connect {
        peer_id: String,
    },
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ServerMessage {
    Registered,
    Connected,
    PeerConnectRequest,
    PeerDisconnected,
    Error { message: String },
}
