//! Integration coverage for the browser's local-daemon probe contract
//! (see web/src/lib/local-daemon.ts): a plain, UNAUTHENTICATED WebSocket
//! connection sending `DashboardRequest{command:"account_peer_id"}` must get
//! back a well-formed peer_id — for ANY daemon, not just one started with
//! `--relay-url` (Phase 1 of the computer-identity-unification work).

#![allow(clippy::field_reassign_with_default)]

use monoterminal_master::{
    auth::Ed25519AuthService,
    server::{Server, ServerConfig},
    webrtc::PairingCodeCache,
};
use monoterminal_protocol::{envelope, Envelope};
use prost::Message as ProstMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

mod common;
use common::ws_client::TestWsClient;

async fn start_test_server_with_identity(
    peer_id: &str,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let session_manager = Arc::new(monoterminal_master::session::manager::SessionManager::new(None));
    let auth_service = Arc::new(Ed25519AuthService::new_with_auto_keypair().unwrap());
    let (health_tx, _health_rx) = broadcast::channel(16);
    let (startup_tx, startup_rx) = oneshot::channel();

    let mut config = ServerConfig::default();
    config.bind_addr = "127.0.0.1:0".parse().unwrap();
    config.dev_mode = true;

    // No relay configured — this is exactly the "pure local daemon" case
    // Phase 1 fixes: peer_id must still be answerable.
    let pairing_cache = PairingCodeCache::new(None, peer_id.to_string());

    let server = Server::with_startup_notification(
        config,
        session_manager,
        Arc::new(monoterminal_master::auth::RateLimiter::new()),
        auth_service,
        health_tx,
        Some(pairing_cache),
        startup_tx,
    )
    .expect("Failed to create server");

    let server_handle = tokio::spawn(async move {
        server.run().await.ok();
    });

    let bound_addr = tokio::time::timeout(Duration::from_secs(5), startup_rx)
        .await
        .expect("Startup notification timeout")
        .expect("Startup notification channel closed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    (bound_addr, server_handle)
}

#[tokio::test]
async fn test_account_peer_id_answered_without_relay_or_auth() {
    let expected_peer_id = "a".repeat(64); // shape of a real hex-encoded Ed25519 pubkey
    let (server_addr, _server_handle) = start_test_server_with_identity(&expected_peer_id).await;

    // No AttachRequest, no auth_token anywhere — this is the exact shape of
    // the browser's local-daemon probe: connect, ask, disconnect.
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let request = monoterminal_protocol::DashboardRequest {
        command: "account_peer_id".to_string(),
        params: Default::default(),
    };
    let envelope = Envelope {
        sequence_number: 1,
        message: Some(envelope::Message::DashboardRequest(request)),
    };
    let mut buf = Vec::with_capacity(envelope.encoded_len());
    envelope.encode(&mut buf).expect("Failed to encode");
    client.send_binary(buf).await.expect("Failed to send");

    let response = tokio::time::timeout(Duration::from_secs(5), client.recv())
        .await
        .expect("Timeout waiting for response")
        .expect("Failed to receive");

    match response {
        tokio_tungstenite::tungstenite::Message::Binary(data) => {
            let response_envelope = Envelope::decode(&data[..]).expect("Failed to decode envelope");
            match response_envelope.message {
                Some(envelope::Message::DashboardResponse(resp)) => {
                    assert_eq!(resp.error, 0, "expected no error, got: {}", resp.json_data);
                    let parsed: serde_json::Value =
                        serde_json::from_str(&resp.json_data).expect("response must be valid JSON");
                    assert_eq!(
                        parsed["peer_id"].as_str(),
                        Some(expected_peer_id.as_str()),
                        "peer_id in response must match this daemon's configured identity"
                    );
                }
                other => panic!("Unexpected response type: {:?}", other),
            }
        }
        other => panic!("Expected binary message, got: {:?}", other),
    }

    client.close().await.ok();
}
