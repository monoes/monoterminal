// WebRTC configuration
// ADR-011 §3: NAT Traversal Strategy (STUN + TURN)

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// STUN server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StunServerConfig {
    /// STUN server URLs (e.g., "stun:stun.l.google.com:19302")
    pub urls: Vec<String>,
}

impl Default for StunServerConfig {
    fn default() -> Self {
        Self {
            // ADR-011: Google STUN servers (free, public, reliable)
            urls: vec![
                "stun:stun.l.google.com:19302".to_string(),
                "stun:stun1.l.google.com:19302".to_string(),
            ],
        }
    }
}

/// TURN server configuration
/// Note: TURN server deployment is Week 3-4 (not Week 1-2)
/// This config is stubbed for future use
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnServerConfig {
    /// TURN server URLs (e.g., "turn:turn.monoterminal.io:3478")
    pub urls: Vec<String>,

    /// TURN username (time-limited, format: "timestamp:peer_id")
    pub username: String,

    /// TURN credential (HMAC-SHA256)
    pub credential: String,

    /// Credential expiry (15-minute TTL per ADR-011 §7.2)
    pub expires_at_ms: u64,
}

/// WebRTC configuration
#[derive(Debug, Clone)]
pub struct WebRtcConfig {
    /// STUN servers for NAT traversal
    pub stun_servers: StunServerConfig,

    /// TURN servers (optional, deferred to Week 3-4)
    pub turn_servers: Option<TurnServerConfig>,

    /// ICE gathering timeout (ADR-011: 10s for STUN, 15s total with TURN)
    pub ice_gathering_timeout: Duration,

    /// Total WebRTC negotiation timeout (ADR-011 §5.2: 15 seconds)
    pub negotiation_timeout: Duration,

    /// Data channel buffer size (bytes)
    pub data_channel_buffer_size: usize,

    /// Enable trickle ICE (send candidates as they're discovered)
    pub trickle_ice: bool,
}

impl Default for WebRtcConfig {
    fn default() -> Self {
        Self {
            stun_servers: StunServerConfig::default(),
            turn_servers: None,                             // Week 3-4
            ice_gathering_timeout: Duration::from_secs(10), // STUN only
            negotiation_timeout: Duration::from_secs(15),   // Total timeout
            data_channel_buffer_size: 256 * 1024,           // 256 KB
            trickle_ice: true,
        }
    }
}

impl WebRtcConfig {
    /// Create configuration for testing (local network only)
    pub fn test_config() -> Self {
        Self {
            stun_servers: StunServerConfig {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
            },
            turn_servers: None,
            ice_gathering_timeout: Duration::from_secs(5), // Shorter for tests
            negotiation_timeout: Duration::from_secs(10),  // Shorter for tests
            data_channel_buffer_size: 64 * 1024,           // 64 KB
            trickle_ice: true,
        }
    }
}
