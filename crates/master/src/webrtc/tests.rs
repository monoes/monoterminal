// WebRTC module integration tests
// ADR-011: P2P Networking Architecture

#[cfg(test)]
mod integration_tests {
    use crate::webrtc::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_webrtc_handshake_round_trip() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        // Generate key pair
        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        // Create handshake
        let handshake = PeerHandshake::new(&signing_key).unwrap();

        // Verify handshake
        let mut verifier = HandshakeVerifier::new();
        let response = verifier.verify(&handshake).unwrap();

        assert!(response.accepted);
        assert!(!response.nonce.is_empty());
        assert!(verifier.is_verified(&handshake.peer_id));
    }

    #[tokio::test]
    async fn test_ice_candidate_gathering_config() {
        let config = Arc::new(WebRtcConfig::test_config());
        let (gatherer, _rx) = IceCandidateGatherer::new(config);

        // Verify ICE servers are configured
        let ice_servers = gatherer.build_ice_servers();
        assert!(!ice_servers.is_empty());

        // Should have STUN server
        assert!(ice_servers[0].urls[0].starts_with("stun:"));
    }

    #[tokio::test]
    async fn test_peer_connection_offer_answer_flow() {
        // This test creates two peer connections and simulates offer/answer exchange
        let config = Arc::new(WebRtcConfig::test_config());

        // Offerer (client)
        let (offerer, _ice_rx1, _msg_rx1) = PeerConnection::new_as_offerer(config.clone())
            .await
            .unwrap();

        // Create data channel
        offerer.create_data_channel("monoterminal").await.unwrap();

        // Create offer
        let offer_sdp = offerer.create_offer().await.unwrap();
        assert!(!offer_sdp.is_empty());

        // Answerer (master)
        let (answerer, _ice_rx2, _msg_rx2) = PeerConnection::new_as_answerer(config.clone())
            .await
            .unwrap();

        // Set remote offer
        answerer.set_remote_offer(offer_sdp).await.unwrap();

        // Create answer
        let answer_sdp = answerer.create_answer().await.unwrap();
        assert!(!answer_sdp.is_empty());

        // Set remote answer
        offerer.set_remote_answer(answer_sdp).await.unwrap();

        // Note: Full connection establishment requires ICE candidate exchange
        // which is async and may not complete in this synchronous test
    }

    #[tokio::test]
    async fn test_dual_transport_websocket_baseline() {
        use tokio::sync::mpsc;

        let (ws_tx, mut ws_rx) = mpsc::channel(256);
        let transport = DualTransport::new(ws_tx);

        // Send data
        let data = b"test message";
        transport.send_dual(data).await.unwrap();

        // Receive via WebSocket
        let received = ws_rx.recv().await.unwrap();
        assert_eq!(received, data);

        // Check stats
        let stats = transport.stats().await;
        assert_eq!(stats.websocket_bytes_sent, data.len() as u64);
        assert!(!stats.webrtc_connected);
    }

    #[test]
    fn test_webrtc_metrics_creation() {
        use prometheus::Registry;

        let registry = Registry::new();
        let metrics = WebRtcMetrics::new(&registry).unwrap();

        // Verify metrics are initialized
        assert_eq!(metrics.webrtc_success_rate.get(), 0.0);
        assert_eq!(metrics.webrtc_attempts_total.get(), 0.0);

        // Simulate some attempts
        metrics.webrtc_attempts_total.inc();
        metrics.webrtc_success_total.inc();
        metrics.update_success_rate();

        assert_eq!(metrics.webrtc_success_rate.get(), 1.0);
    }

    #[test]
    fn test_connection_state_mapping() {
        use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;

        assert_eq!(
            PeerConnectionState::from(RTCPeerConnectionState::New),
            PeerConnectionState::New
        );
        assert_eq!(
            PeerConnectionState::from(RTCPeerConnectionState::Connecting),
            PeerConnectionState::Connecting
        );
        assert_eq!(
            PeerConnectionState::from(RTCPeerConnectionState::Connected),
            PeerConnectionState::Connected
        );
        assert_eq!(
            PeerConnectionState::from(RTCPeerConnectionState::Failed),
            PeerConnectionState::Failed
        );
    }
}
