// Dual-transport abstraction (WebSocket + WebRTC DataChannel)
// ADR-011 §1: Transport Strategy - Both active concurrently

use crate::webrtc::error::{Result, WebRtcError};
use crate::webrtc::peer_connection::{PeerConnection, PeerConnectionState};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

/// Transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// WebSocket (baseline, always available)
    WebSocket,
    /// WebRTC DataChannel (P2P optimized)
    WebRtc,
}

/// Transport trait - abstraction over WebSocket and WebRTC
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send data via this transport
    async fn send(&self, data: &[u8]) -> Result<()>;

    /// Check if transport is connected
    async fn is_connected(&self) -> bool;

    /// Get transport type
    fn transport_type(&self) -> TransportType;
}

/// DualTransport - manages both WebSocket and WebRTC concurrently
/// Per ADR-011 §1: Both transports active, client deduplicates
pub struct DualTransport {
    /// WebSocket sender (always active)
    websocket_tx: Arc<Mutex<mpsc::Sender<Vec<u8>>>>,

    /// WebRTC peer connection (optional, may fail to establish)
    webrtc_peer: Arc<Mutex<Option<Arc<PeerConnection>>>>,

    /// Preferred transport (default: WebRTC if connected, else WebSocket)
    preferred_transport: Arc<Mutex<TransportType>>,

    /// Statistics
    websocket_bytes_sent: Arc<Mutex<u64>>,
    webrtc_bytes_sent: Arc<Mutex<u64>>,
}

impl DualTransport {
    /// Create a new DualTransport with WebSocket as baseline
    pub fn new(websocket_tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            websocket_tx: Arc::new(Mutex::new(websocket_tx)),
            webrtc_peer: Arc::new(Mutex::new(None)),
            preferred_transport: Arc::new(Mutex::new(TransportType::WebSocket)),
            websocket_bytes_sent: Arc::new(Mutex::new(0)),
            webrtc_bytes_sent: Arc::new(Mutex::new(0)),
        }
    }

    /// Add WebRTC peer connection (after negotiation succeeds)
    pub async fn set_webrtc_peer(&self, peer: Arc<PeerConnection>) {
        debug!("Setting WebRTC peer connection");
        let mut guard = self.webrtc_peer.lock().await;
        *guard = Some(peer);

        // Switch preferred transport to WebRTC
        let mut pref = self.preferred_transport.lock().await;
        *pref = TransportType::WebRtc;
    }

    /// Send data via both transports (ADR-011 §1: dual broadcast)
    /// Client deduplicates by sequence_number
    pub async fn send_dual(&self, data: &[u8]) -> Result<()> {
        // Always send via WebSocket (baseline fallback)
        let ws_result = self.send_websocket(data).await;

        // Try WebRTC if connected
        let webrtc_guard = self.webrtc_peer.lock().await;
        if let Some(ref peer) = *webrtc_guard {
            match peer.state().await {
                PeerConnectionState::Connected => {
                    // Send via WebRTC
                    if let Err(e) = peer.send(data).await {
                        warn!("WebRTC send failed, falling back to WebSocket only: {}", e);
                        // WebSocket send already happened above
                    } else {
                        // Track WebRTC bytes
                        let mut wrtc_bytes = self.webrtc_bytes_sent.lock().await;
                        *wrtc_bytes += data.len() as u64;
                    }
                }
                _ => {
                    // WebRTC not connected, WebSocket-only mode
                    debug!("WebRTC not connected, using WebSocket only");
                }
            }
        }

        // Return WebSocket result (baseline must succeed)
        ws_result
    }

    /// Send data via preferred transport only (optimize bandwidth)
    /// WARNING: Use only if client confirms receipt via preferred transport
    pub async fn send_preferred(&self, data: &[u8]) -> Result<()> {
        let pref = *self.preferred_transport.lock().await;

        match pref {
            TransportType::WebRtc => {
                // Try WebRTC first
                let webrtc_guard = self.webrtc_peer.lock().await;
                if let Some(ref peer) = *webrtc_guard {
                    if peer.state().await == PeerConnectionState::Connected {
                        return peer.send(data).await;
                    }
                }

                // Fallback to WebSocket
                warn!("WebRTC preferred but not connected, falling back to WebSocket");
                self.send_websocket(data).await
            }
            TransportType::WebSocket => self.send_websocket(data).await,
        }
    }

    /// Send via WebSocket only
    async fn send_websocket(&self, data: &[u8]) -> Result<()> {
        let guard = self.websocket_tx.lock().await;
        guard
            .send(data.to_vec())
            .await
            .map_err(|_| WebRtcError::Internal("WebSocket channel closed".to_string()))?;

        // Track bytes
        let mut ws_bytes = self.websocket_bytes_sent.lock().await;
        *ws_bytes += data.len() as u64;

        Ok(())
    }

    /// Check if WebRTC is connected
    pub async fn is_webrtc_connected(&self) -> bool {
        let guard = self.webrtc_peer.lock().await;
        if let Some(ref peer) = *guard {
            peer.state().await == PeerConnectionState::Connected
        } else {
            false
        }
    }

    /// Get transport statistics
    pub async fn stats(&self) -> TransportStats {
        TransportStats {
            websocket_bytes_sent: *self.websocket_bytes_sent.lock().await,
            webrtc_bytes_sent: *self.webrtc_bytes_sent.lock().await,
            webrtc_connected: self.is_webrtc_connected().await,
            preferred_transport: *self.preferred_transport.lock().await,
        }
    }

    /// Force failover to WebSocket only (e.g., WebRTC timeout)
    pub async fn failover_to_websocket(&self) {
        warn!("Failing over to WebSocket-only mode");
        let mut pref = self.preferred_transport.lock().await;
        *pref = TransportType::WebSocket;
    }
}

/// Transport statistics
#[derive(Debug, Clone)]
pub struct TransportStats {
    pub websocket_bytes_sent: u64,
    pub webrtc_bytes_sent: u64,
    pub webrtc_connected: bool,
    pub preferred_transport: TransportType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dual_transport_creation() {
        let (ws_tx, _ws_rx) = mpsc::channel(256);
        let transport = DualTransport::new(ws_tx);

        // Initially WebSocket only
        assert!(!transport.is_webrtc_connected().await);

        let stats = transport.stats().await;
        assert_eq!(stats.preferred_transport, TransportType::WebSocket);
        assert_eq!(stats.websocket_bytes_sent, 0);
        assert_eq!(stats.webrtc_bytes_sent, 0);
    }

    #[tokio::test]
    async fn test_send_websocket_only() {
        let (ws_tx, mut ws_rx) = mpsc::channel(256);
        let transport = DualTransport::new(ws_tx);

        // Send data
        let data = b"test message";
        transport.send_dual(data).await.unwrap();

        // Should receive via WebSocket
        let received = ws_rx.recv().await.unwrap();
        assert_eq!(received, data);

        // Check stats
        let stats = transport.stats().await;
        assert_eq!(stats.websocket_bytes_sent, data.len() as u64);
    }

    #[tokio::test]
    async fn test_failover_to_websocket() {
        let (ws_tx, _ws_rx) = mpsc::channel(256);
        let transport = DualTransport::new(ws_tx);

        // Initially WebSocket
        assert_eq!(
            *transport.preferred_transport.lock().await,
            TransportType::WebSocket
        );

        // Failover (should be no-op but test the call)
        transport.failover_to_websocket().await;

        assert_eq!(
            *transport.preferred_transport.lock().await,
            TransportType::WebSocket
        );
    }
}
