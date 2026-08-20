// WebRTC P2P Networking (Phase 2)
// Implements ADR-011: P2P Networking Architecture
// SRS §2.3: P2P Networking, §7.2: Phase 2 Acceptance Criteria

#![allow(dead_code)] // Phase 2 placeholder implementations, cleanup tracked in task-63
#![allow(unused_imports)] // Phase 2 placeholder imports, cleanup tracked in task-63

pub mod config;
pub mod error;
pub mod handshake;
pub mod ice;
pub mod peer_connection;
pub mod transport;

#[cfg(test)]
mod tests;

pub use config::WebRtcConfig;
pub use handshake::{HandshakeVerifier, PeerHandshake};
pub use ice::IceCandidateGatherer;
pub use peer_connection::{PeerConnection, PeerConnectionState};
pub use transport::DualTransport;

use prometheus::{Counter, Gauge, Histogram, HistogramOpts, Registry};

/// WebRTC metrics for Prometheus
/// Per ADR-011 §8: Health check endpoints
#[derive(Clone)]
pub struct WebRtcMetrics {
    /// WebRTC connection success rate (0-1)
    pub webrtc_success_rate: Gauge,

    /// Total WebRTC connection attempts
    pub webrtc_attempts_total: Counter,

    /// Successful WebRTC connections
    pub webrtc_success_total: Counter,

    /// Failed WebRTC connections
    pub webrtc_failed_total: Counter,

    /// Current WebRTC connection state (0=disconnected, 1=connecting, 2=connected, 3=failed)
    pub webrtc_connection_state: Gauge,

    /// ICE candidate gathering duration (seconds)
    pub ice_gathering_duration: Histogram,

    /// TURN server health status (0=unknown, 1=healthy, 2=unhealthy)
    pub turn_health_status: Gauge,

    /// STUN server health status (0=unknown, 1=healthy, 2=unhealthy)
    pub stun_health_status: Gauge,
}

impl WebRtcMetrics {
    /// Create new WebRTC metrics and register with Prometheus
    pub fn new(registry: &Registry) -> prometheus::Result<Self> {
        let webrtc_success_rate = Gauge::new(
            "webrtc_success_rate",
            "WebRTC connection success rate (0-1)",
        )?;
        registry.register(Box::new(webrtc_success_rate.clone()))?;

        let webrtc_attempts_total =
            Counter::new("webrtc_attempts_total", "Total WebRTC connection attempts")?;
        registry.register(Box::new(webrtc_attempts_total.clone()))?;

        let webrtc_success_total =
            Counter::new("webrtc_success_total", "Successful WebRTC connections")?;
        registry.register(Box::new(webrtc_success_total.clone()))?;

        let webrtc_failed_total = Counter::new("webrtc_failed_total", "Failed WebRTC connections")?;
        registry.register(Box::new(webrtc_failed_total.clone()))?;

        let webrtc_connection_state = Gauge::new(
            "webrtc_connection_state",
            "Current WebRTC connection state (0=disconnected, 1=connecting, 2=connected, 3=failed)",
        )?;
        registry.register(Box::new(webrtc_connection_state.clone()))?;

        let ice_gathering_duration = Histogram::with_opts(HistogramOpts::new(
            "ice_gathering_duration_seconds",
            "ICE candidate gathering duration in seconds",
        ))?;
        registry.register(Box::new(ice_gathering_duration.clone()))?;

        let turn_health_status = Gauge::new(
            "turn_health_status",
            "TURN server health status (0=unknown, 1=healthy, 2=unhealthy)",
        )?;
        registry.register(Box::new(turn_health_status.clone()))?;

        let stun_health_status = Gauge::new(
            "stun_health_status",
            "STUN server health status (0=unknown, 1=healthy, 2=unhealthy)",
        )?;
        registry.register(Box::new(stun_health_status.clone()))?;

        Ok(Self {
            webrtc_success_rate,
            webrtc_attempts_total,
            webrtc_success_total,
            webrtc_failed_total,
            webrtc_connection_state,
            ice_gathering_duration,
            turn_health_status,
            stun_health_status,
        })
    }

    /// Update success rate based on attempts and successes
    pub fn update_success_rate(&self) {
        let attempts = self.webrtc_attempts_total.get();
        let successes = self.webrtc_success_total.get();

        if attempts > 0.0 {
            let rate = successes / attempts;
            self.webrtc_success_rate.set(rate);
        }
    }
}
