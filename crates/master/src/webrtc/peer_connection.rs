// WebRTC PeerConnection wrapper
// ADR-011 §2: Hub-and-Spoke Topology (Client-to-Master)
// ADR-011 §5: Connection Lifecycle

use crate::webrtc::config::WebRtcConfig;
use crate::webrtc::error::{Result, WebRtcError};
use crate::webrtc::ice::{IceCandidate, IceCandidateGatherer};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage as WebRtcDataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// PeerConnection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerConnectionState {
    /// Initial state
    New,
    /// Connecting (ICE gathering in progress)
    Connecting,
    /// Connected (DataChannel open)
    Connected,
    /// Failed (timeout or error)
    Failed,
    /// Disconnected (graceful close)
    Disconnected,
}

impl From<RTCPeerConnectionState> for PeerConnectionState {
    fn from(state: RTCPeerConnectionState) -> Self {
        match state {
            RTCPeerConnectionState::New => Self::New,
            RTCPeerConnectionState::Connecting => Self::Connecting,
            RTCPeerConnectionState::Connected => Self::Connected,
            RTCPeerConnectionState::Disconnected => Self::Disconnected,
            RTCPeerConnectionState::Failed => Self::Failed,
            RTCPeerConnectionState::Closed => Self::Disconnected,
            _ => Self::Failed,
        }
    }
}

/// DataChannel message (received from peer)
#[derive(Debug, Clone)]
pub struct DataChannelMessage {
    /// Raw message data
    pub data: Vec<u8>,
    /// Whether the message is binary (vs text)
    pub is_binary: bool,
}

/// PeerConnection wrapper
/// Manages WebRTC peer connection lifecycle and DataChannel
pub struct PeerConnection {
    /// WebRTC peer connection
    peer_connection: Arc<RTCPeerConnection>,

    /// Data channel for P2P messages
    data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,

    /// Configuration
    #[allow(dead_code)]
    config: Arc<WebRtcConfig>,

    /// State
    state: Arc<Mutex<PeerConnectionState>>,

    /// Outgoing ICE candidates channel
    #[allow(dead_code)]
    ice_candidates_tx: mpsc::Sender<IceCandidate>,

    /// Incoming data channel messages
    messages_tx: mpsc::Sender<DataChannelMessage>,
}

impl PeerConnection {
    /// Create a new PeerConnection (as offerer - client side)
    pub async fn new_as_offerer(
        config: Arc<WebRtcConfig>,
    ) -> Result<(
        Self,
        mpsc::Receiver<IceCandidate>,
        mpsc::Receiver<DataChannelMessage>,
    )> {
        debug!("Creating PeerConnection as offerer");

        // Build ICE servers from config
        let (gatherer, _) = IceCandidateGatherer::new(config.clone());
        let ice_servers = gatherer.build_ice_servers();

        // Create WebRTC API
        let api = Self::create_api()?;

        // Create peer connection
        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let peer_connection = api
            .new_peer_connection(rtc_config)
            .await
            .map_err(|e| WebRtcError::PeerConnectionFailed(e.to_string()))?;

        let (ice_candidates_tx, ice_candidates_rx) = mpsc::channel(32);
        let (messages_tx, messages_rx) = mpsc::channel(256);

        let pc = Arc::new(peer_connection);
        let state = Arc::new(Mutex::new(PeerConnectionState::New));

        // Set up ICE candidate handler
        Self::setup_ice_candidate_handler(pc.clone(), ice_candidates_tx.clone());

        // Set up connection state handler
        Self::setup_connection_state_handler(pc.clone(), state.clone());

        let conn = Self {
            peer_connection: pc,
            data_channel: Arc::new(Mutex::new(None)),
            config,
            state,
            ice_candidates_tx,
            messages_tx,
        };

        Ok((conn, ice_candidates_rx, messages_rx))
    }

    /// Create a new PeerConnection (as answerer - master side)
    pub async fn new_as_answerer(
        config: Arc<WebRtcConfig>,
    ) -> Result<(
        Self,
        mpsc::Receiver<IceCandidate>,
        mpsc::Receiver<DataChannelMessage>,
    )> {
        debug!("Creating PeerConnection as answerer");

        // Build ICE servers
        let (gatherer, _) = IceCandidateGatherer::new(config.clone());
        let ice_servers = gatherer.build_ice_servers();

        // Create WebRTC API
        let api = Self::create_api()?;

        // Create peer connection
        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        let peer_connection = api
            .new_peer_connection(rtc_config)
            .await
            .map_err(|e| WebRtcError::PeerConnectionFailed(e.to_string()))?;

        let (ice_candidates_tx, ice_candidates_rx) = mpsc::channel(32);
        let (messages_tx, messages_rx) = mpsc::channel(256);

        let pc = Arc::new(peer_connection);
        let state = Arc::new(Mutex::new(PeerConnectionState::New));

        // Set up handlers
        Self::setup_ice_candidate_handler(pc.clone(), ice_candidates_tx.clone());
        Self::setup_connection_state_handler(pc.clone(), state.clone());

        let data_channel = Arc::new(Mutex::new(None));

        // Answerer never calls create_data_channel itself — the offerer
        // (browser) creates it, so we must listen for the remote-created
        // channel via on_data_channel instead, or `data_channel` stays None
        // forever and `send()` always fails with DataChannelClosed.
        Self::setup_data_channel_handler(pc.clone(), data_channel.clone(), messages_tx.clone());

        let conn = Self {
            peer_connection: pc,
            data_channel,
            config,
            state,
            ice_candidates_tx,
            messages_tx,
        };

        Ok((conn, ice_candidates_rx, messages_rx))
    }

    /// Create WebRTC API
    fn create_api() -> Result<webrtc::api::API> {
        // Create API with default configuration
        // The interceptor registry is created internally by the API builder
        let api = APIBuilder::new().build();

        Ok(api)
    }

    /// Set up ICE candidate handler
    fn setup_ice_candidate_handler(pc: Arc<RTCPeerConnection>, ice_tx: mpsc::Sender<IceCandidate>) {
        pc.on_ice_candidate(Box::new(move |candidate| {
            let tx = ice_tx.clone();
            Box::pin(async move {
                if let Some(candidate) = candidate {
                    debug!("ICE candidate discovered: {}", candidate);
                    let ice_candidate = IceCandidate::from(candidate);
                    let _ = tx.send(ice_candidate).await;
                }
            })
        }));
    }

    /// Set up connection state handler
    fn setup_connection_state_handler(
        pc: Arc<RTCPeerConnection>,
        state: Arc<Mutex<PeerConnectionState>>,
    ) {
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let state = state.clone();
            Box::pin(async move {
                info!("PeerConnection state changed: {:?}", s);
                let mut guard = state.lock().await;
                *guard = PeerConnectionState::from(s);
            })
        }));
    }

    /// Create a data channel (offerer side)
    pub async fn create_data_channel(&self, label: &str) -> Result<()> {
        debug!("Creating DataChannel: {}", label);

        let dc = self
            .peer_connection
            .create_data_channel(label, None)
            .await
            .map_err(|e| WebRtcError::DataChannelCreationFailed(e.to_string()))?;

        Self::wire_data_channel_messages(dc.clone(), self.messages_tx.clone());

        let mut guard = self.data_channel.lock().await;
        *guard = Some(dc);

        Ok(())
    }

    /// Set up the `ondatachannel` handler (answerer side). The offerer
    /// creates the channel and it arrives here asynchronously — without
    /// this, the answerer's `data_channel` field never gets populated and
    /// `send()` always fails with `DataChannelClosed`.
    fn setup_data_channel_handler(
        pc: Arc<RTCPeerConnection>,
        data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
        messages_tx: mpsc::Sender<DataChannelMessage>,
    ) {
        pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
            let data_channel = data_channel.clone();
            let messages_tx = messages_tx.clone();
            Box::pin(async move {
                debug!("Remote DataChannel received: {}", dc.label());
                Self::wire_data_channel_messages(dc.clone(), messages_tx);
                let mut guard = data_channel.lock().await;
                *guard = Some(dc);
            })
        }));
    }

    /// Forward incoming DataChannel messages onto `messages_tx`. Shared by
    /// both the offerer (channel it created itself) and the answerer
    /// (channel received via `on_data_channel`).
    fn wire_data_channel_messages(dc: Arc<RTCDataChannel>, messages_tx: mpsc::Sender<DataChannelMessage>) {
        dc.on_message(Box::new(move |msg: WebRtcDataChannelMessage| {
            let tx = messages_tx.clone();
            Box::pin(async move {
                debug!("DataChannel message received: {} bytes", msg.data.len());
                let _ = tx
                    .send(DataChannelMessage {
                        data: msg.data.to_vec(),
                        is_binary: !msg.is_string, // is_string=true means text, so is_binary=false
                    })
                    .await;
            })
        }));
    }

    /// Create SDP offer
    pub async fn create_offer(&self) -> Result<String> {
        debug!("Creating SDP offer");

        let offer = self
            .peer_connection
            .create_offer(None)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        let sdp = offer.sdp.clone();

        self.peer_connection
            .set_local_description(offer)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        Ok(sdp)
    }

    /// Set remote SDP offer (answerer side)
    pub async fn set_remote_offer(&self, sdp: String) -> Result<()> {
        debug!("Setting remote SDP offer");

        let offer = RTCSessionDescription::offer(sdp)
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        self.peer_connection
            .set_remote_description(offer)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        Ok(())
    }

    /// Create SDP answer (answerer side)
    pub async fn create_answer(&self) -> Result<String> {
        debug!("Creating SDP answer");

        let answer = self
            .peer_connection
            .create_answer(None)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        let sdp = answer.sdp.clone();

        self.peer_connection
            .set_local_description(answer)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        Ok(sdp)
    }

    /// Set remote SDP answer (offerer side)
    pub async fn set_remote_answer(&self, sdp: String) -> Result<()> {
        debug!("Setting remote SDP answer");

        let answer = RTCSessionDescription::answer(sdp)
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        self.peer_connection
            .set_remote_description(answer)
            .await
            .map_err(|e| WebRtcError::SdpNegotiationFailed(e.to_string()))?;

        Ok(())
    }

    /// Add ICE candidate
    pub async fn add_ice_candidate(&self, candidate: IceCandidate) -> Result<()> {
        debug!("Adding ICE candidate: {}", candidate.candidate);

        let init: webrtc::ice_transport::ice_candidate::RTCIceCandidateInit =
            candidate.try_into()?;

        self.peer_connection
            .add_ice_candidate(init)
            .await
            .map_err(|e| WebRtcError::IceGatheringFailed(e.to_string()))?;

        Ok(())
    }

    /// Send data via DataChannel
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let guard = self.data_channel.lock().await;

        if let Some(ref dc) = *guard {
            dc.send(&Bytes::copy_from_slice(data))
                .await
                .map_err(|_e| WebRtcError::DataChannelClosed)?;
            Ok(())
        } else {
            Err(WebRtcError::DataChannelClosed)
        }
    }

    /// Get current connection state
    pub async fn state(&self) -> PeerConnectionState {
        *self.state.lock().await
    }

    /// Close the peer connection
    pub async fn close(&self) -> Result<()> {
        debug!("Closing PeerConnection");

        self.peer_connection
            .close()
            .await
            .map_err(|e| WebRtcError::Internal(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_connection_creation_offerer() {
        let config = Arc::new(WebRtcConfig::test_config());
        let result = PeerConnection::new_as_offerer(config).await;

        // Should succeed
        assert!(result.is_ok());
        let (conn, _ice_rx, _msg_rx) = result.unwrap();

        // Initial state should be New
        assert_eq!(conn.state().await, PeerConnectionState::New);
    }

    #[tokio::test]
    async fn test_peer_connection_creation_answerer() {
        let config = Arc::new(WebRtcConfig::test_config());
        let result = PeerConnection::new_as_answerer(config).await;

        // Should succeed
        assert!(result.is_ok());
        let (conn, _ice_rx, _msg_rx) = result.unwrap();

        // Initial state should be New
        assert_eq!(conn.state().await, PeerConnectionState::New);
    }

    #[tokio::test]
    async fn test_create_data_channel() {
        let config = Arc::new(WebRtcConfig::test_config());
        let (conn, _ice_rx, _msg_rx) = PeerConnection::new_as_offerer(config).await.unwrap();

        // Create data channel
        let result = conn.create_data_channel("monoterminal").await;
        assert!(result.is_ok());

        // Verify data channel is stored
        let guard = conn.data_channel.lock().await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn test_create_offer() {
        let config = Arc::new(WebRtcConfig::test_config());
        let (conn, _ice_rx, _msg_rx) = PeerConnection::new_as_offerer(config).await.unwrap();

        // Create data channel first
        conn.create_data_channel("monoterminal").await.unwrap();

        // Create offer
        let result = conn.create_offer().await;
        assert!(result.is_ok());

        let sdp = result.unwrap();
        assert!(!sdp.is_empty());
        assert!(sdp.contains("v=0")); // SDP version
    }

    /// End-to-end: full offer/answer + trickle ICE exchange between two
    /// local PeerConnections, then a DataChannel message in both
    /// directions. This specifically exercises the `on_data_channel` fix —
    /// before it, the answerer's `data_channel` field never got populated
    /// (nothing installed `on_data_channel`), so `answerer.send()` always
    /// failed with `DataChannelClosed` and the offerer never received
    /// anything back, even though the offer/answer/ICE handshake itself
    /// looked fine.
    #[tokio::test]
    async fn test_full_connection_and_data_channel_roundtrip() {
        use tokio::time::{timeout, Duration};

        let config = Arc::new(WebRtcConfig::test_config());

        let (offerer, mut offerer_ice_rx, mut offerer_msg_rx) =
            PeerConnection::new_as_offerer(config.clone()).await.unwrap();
        let (answerer, mut answerer_ice_rx, mut answerer_msg_rx) =
            PeerConnection::new_as_answerer(config).await.unwrap();
        let offerer = Arc::new(offerer);
        let answerer = Arc::new(answerer);

        offerer.create_data_channel("monoterminal").await.unwrap();

        let offer_sdp = offerer.create_offer().await.unwrap();
        answerer.set_remote_offer(offer_sdp).await.unwrap();
        let answer_sdp = answerer.create_answer().await.unwrap();
        offerer.set_remote_answer(answer_sdp).await.unwrap();

        // Trickle ICE both ways (loopback host candidates only — no STUN
        // needed since both peers are in the same process).
        let answerer_for_ice = answerer.clone();
        tokio::spawn(async move {
            while let Some(candidate) = offerer_ice_rx.recv().await {
                let _ = answerer_for_ice.add_ice_candidate(candidate).await;
            }
        });
        let offerer_for_ice = offerer.clone();
        tokio::spawn(async move {
            while let Some(candidate) = answerer_ice_rx.recv().await {
                let _ = offerer_for_ice.add_ice_candidate(candidate).await;
            }
        });

        // Wait for both sides to report Connected.
        async fn wait_connected(pc: &PeerConnection) -> bool {
            for _ in 0..100 {
                if pc.state().await == PeerConnectionState::Connected {
                    return true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            false
        }
        assert!(
            timeout(Duration::from_secs(15), wait_connected(&offerer))
                .await
                .unwrap_or(false),
            "offerer never reached Connected state"
        );
        assert!(
            timeout(Duration::from_secs(15), wait_connected(&answerer))
                .await
                .unwrap_or(false),
            "answerer never reached Connected state"
        );

        // The DataChannel itself opens asynchronously slightly after the
        // peer connection state does — poll send() until it stops failing
        // with DataChannelClosed instead of asserting on the very first try.
        let mut sent = false;
        for _ in 0..50 {
            if offerer.send(b"hello from offerer").await.is_ok() {
                sent = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(sent, "offerer could never send on its DataChannel");

        let received = timeout(Duration::from_secs(5), answerer_msg_rx.recv())
            .await
            .expect("timed out waiting for answerer to receive DataChannel message")
            .expect("answerer message channel closed unexpectedly");
        assert_eq!(received.data, b"hello from offerer");

        // And the reverse direction — this is the part that was broken:
        // without on_data_channel, `answerer.send()` always returned
        // Err(DataChannelClosed) because `data_channel` was never Some.
        let mut sent_back = false;
        for _ in 0..50 {
            if answerer.send(b"hello from answerer").await.is_ok() {
                sent_back = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(sent_back, "answerer could never send on its DataChannel (on_data_channel regression)");

        let received_back = timeout(Duration::from_secs(5), offerer_msg_rx.recv())
            .await
            .expect("timed out waiting for offerer to receive DataChannel message")
            .expect("offerer message channel closed unexpectedly");
        assert_eq!(received_back.data, b"hello from answerer");
    }
}
