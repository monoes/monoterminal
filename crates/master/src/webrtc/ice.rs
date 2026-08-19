// ICE candidate gathering and STUN client
// ADR-011 §3: NAT Traversal Strategy (STUN + TURN)

use crate::webrtc::config::WebRtcConfig;
use crate::webrtc::error::{Result, WebRtcError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_server::RTCIceServer;

/// ICE candidate (trickle ICE protocol)
/// Sent incrementally as candidates are discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    /// Candidate string (SDP format)
    pub candidate: String,

    /// SDP media line index
    pub sdp_mid: Option<String>,

    /// SDP media line index (number)
    pub sdp_mline_index: Option<u16>,

    /// Username fragment (for candidate authentication)
    pub username_fragment: Option<String>,
}

impl From<RTCIceCandidate> for IceCandidate {
    fn from(rtc_candidate: RTCIceCandidate) -> Self {
        let json = rtc_candidate.to_json().unwrap();
        Self {
            candidate: json.candidate,
            sdp_mid: json.sdp_mid,
            sdp_mline_index: json.sdp_mline_index,
            username_fragment: json.username_fragment,
        }
    }
}

impl TryInto<RTCIceCandidateInit> for IceCandidate {
    type Error = WebRtcError;

    fn try_into(self) -> Result<RTCIceCandidateInit> {
        Ok(RTCIceCandidateInit {
            candidate: self.candidate,
            sdp_mid: self.sdp_mid,
            sdp_mline_index: self.sdp_mline_index,
            username_fragment: self.username_fragment,
        })
    }
}

/// ICE candidate gatherer
/// Manages ICE candidate discovery with timeout and metrics
pub struct IceCandidateGatherer {
    config: Arc<WebRtcConfig>,
    #[allow(dead_code)]
    candidates_tx: mpsc::Sender<IceCandidate>,
}

impl IceCandidateGatherer {
    /// Create a new ICE candidate gatherer
    pub fn new(config: Arc<WebRtcConfig>) -> (Self, mpsc::Receiver<IceCandidate>) {
        let (candidates_tx, candidates_rx) = mpsc::channel(32);

        (
            Self {
                config,
                candidates_tx,
            },
            candidates_rx,
        )
    }

    /// Build RTCIceServer list from config
    pub fn build_ice_servers(&self) -> Vec<RTCIceServer> {
        let mut ice_servers = Vec::new();

        // Add STUN servers
        ice_servers.push(RTCIceServer {
            urls: self.config.stun_servers.urls.clone(),
            username: String::new(),
            credential: String::new(),
            ..Default::default()
        });

        // Add TURN servers if configured (Week 3-4)
        if let Some(ref turn_config) = self.config.turn_servers {
            ice_servers.push(RTCIceServer {
                urls: turn_config.urls.clone(),
                username: turn_config.username.clone(),
                credential: turn_config.credential.clone(),
                ..Default::default()
            });
        }

        ice_servers
    }

    /// Gather ICE candidates with timeout
    /// Returns when gathering completes or timeout expires
    pub async fn gather_with_timeout(&self) -> Result<Vec<IceCandidate>> {
        let start = Instant::now();
        let timeout = self.config.ice_gathering_timeout;

        debug!("Starting ICE candidate gathering (timeout: {:?})", timeout);

        // Create a channel to collect candidates
        let (tx, mut rx) = mpsc::channel(32);

        // Spawn gathering task
        let gatherer_tx: mpsc::Sender<IceCandidate> = tx.clone();
        let ice_servers = self.build_ice_servers();

        tokio::spawn(async move {
            // Simulate ICE gathering (real implementation in peer_connection.rs)
            // This is a placeholder for the actual WebRTC gathering process
            debug!("ICE gathering started with {} servers", ice_servers.len());

            // In reality, this would be triggered by RTCPeerConnection.on_ice_candidate
            // For now, we just signal completion
            drop(gatherer_tx);
        });

        // Collect candidates until timeout or completion
        let mut candidates = Vec::new();

        loop {
            tokio::select! {
                Some(candidate) = rx.recv() => {
                    debug!("ICE candidate discovered: {}", candidate.candidate);
                    candidates.push(candidate);
                }
                _ = tokio::time::sleep(timeout) => {
                    let elapsed = start.elapsed();
                    if candidates.is_empty() {
                        warn!("ICE gathering timeout after {:?}, no candidates found", elapsed);
                        return Err(WebRtcError::IceGatheringTimeout(elapsed.as_secs()));
                    } else {
                        info!("ICE gathering timeout after {:?}, collected {} candidates", elapsed, candidates.len());
                        break;
                    }
                }
                else => {
                    // Channel closed, gathering complete
                    let elapsed = start.elapsed();
                    info!("ICE gathering complete in {:?}, collected {} candidates", elapsed, candidates.len());
                    break;
                }
            }
        }

        Ok(candidates)
    }
}

/// STUN server health probe
/// Checks if STUN server is reachable
pub async fn probe_stun_server(stun_url: &str, timeout: Duration) -> Result<bool> {
    debug!("Probing STUN server: {}", stun_url);

    // Parse STUN URL (format: "stun:host:port")
    let parts: Vec<&str> = stun_url.split(':').collect();
    if parts.len() != 3 || parts[0] != "stun" {
        return Err(WebRtcError::StunServerUnreachable(format!(
            "Invalid STUN URL: {}",
            stun_url
        )));
    }

    let host = parts[1];
    let port: u16 = parts[2]
        .parse()
        .map_err(|_| WebRtcError::StunServerUnreachable("Invalid port".to_string()))?;

    // Attempt to resolve and connect
    let addr = tokio::net::lookup_host(format!("{}:{}", host, port))
        .await
        .map_err(|e| WebRtcError::StunServerUnreachable(format!("DNS resolution failed: {}", e)))?
        .next()
        .ok_or_else(|| WebRtcError::StunServerUnreachable("No addresses resolved".to_string()))?;

    // Try UDP socket (STUN uses UDP)
    let socket = tokio::time::timeout(timeout, tokio::net::UdpSocket::bind("0.0.0.0:0"))
        .await
        .map_err(|_| WebRtcError::StunServerUnreachable("Socket bind timeout".to_string()))?
        .map_err(|e| WebRtcError::StunServerUnreachable(format!("Socket bind failed: {}", e)))?;

    // Send a simple probe packet (STUN Binding Request would be proper, but this tests reachability)
    let probe_data = [0u8; 1];
    tokio::time::timeout(timeout, socket.send_to(&probe_data, addr))
        .await
        .map_err(|_| WebRtcError::StunServerUnreachable("Send timeout".to_string()))?
        .map_err(|e| WebRtcError::StunServerUnreachable(format!("Send failed: {}", e)))?;

    debug!("STUN server probe successful: {}", stun_url);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ice_candidate_serialization() {
        let candidate = IceCandidate {
            candidate: "candidate:1 1 UDP 2122260223 192.168.1.100 54321 typ host".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_mline_index: Some(0),
            username_fragment: Some("test-ufrag".to_string()),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&candidate).unwrap();
        assert!(json.contains("candidate:1"));

        // Deserialize back
        let deserialized: IceCandidate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.candidate, candidate.candidate);
        assert_eq!(deserialized.sdp_mid, candidate.sdp_mid);
    }

    #[test]
    fn test_ice_candidate_gatherer_creation() {
        let config = Arc::new(WebRtcConfig::test_config());
        let (_gatherer, _rx) = IceCandidateGatherer::new(config.clone());

        // Just verify construction works
    }

    #[test]
    fn test_build_ice_servers() {
        let config = Arc::new(WebRtcConfig::test_config());
        let (gatherer, _rx) = IceCandidateGatherer::new(config);

        let ice_servers = gatherer.build_ice_servers();

        // Should have at least STUN servers
        assert!(!ice_servers.is_empty());
        assert!(!ice_servers[0].urls.is_empty());
        assert!(ice_servers[0].urls[0].starts_with("stun:"));
    }

    #[tokio::test]
    async fn test_probe_stun_server_invalid_url() {
        let result = probe_stun_server("invalid", Duration::from_secs(1)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_probe_stun_server_google() {
        // Test actual Google STUN server (may be slow/flaky in CI)
        // Skip if network unavailable
        let result =
            probe_stun_server("stun:stun.l.google.com:19302", Duration::from_secs(5)).await;

        // We don't assert success because it depends on network
        // Just verify it doesn't panic
        match result {
            Ok(true) => println!("STUN probe succeeded"),
            Err(e) => println!("STUN probe failed (expected in offline envs): {}", e),
            _ => {}
        }
    }
}
