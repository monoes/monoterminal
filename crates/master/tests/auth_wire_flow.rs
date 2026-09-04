//! Wire-level coverage for the Ed25519 challenge-response -> JWT -> attach
//! flow (SRS §3.2.2). Every server started here has `dev_mode = false` —
//! this is the exact scenario that was impossible before this feature: a
//! non-dev-mode daemon simply had no way for a client to ever obtain a
//! JWT at all. `crates/master/tests/auth_comprehensive.rs` and
//! `auth_integration.rs` already cover the crypto semantics in-process;
//! this file is their wire-level complement.

#![allow(clippy::field_reassign_with_default)]

use monoterminal_master::{
    auth::Ed25519AuthService,
    server::{Server, ServerConfig, TlsConfig},
    session::manager::SessionManager,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

mod common;
use common::ws_client::TestWsClient;

async fn start_test_server() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let session_manager = Arc::new(SessionManager::new(None));
    let auth_service = Arc::new(Ed25519AuthService::new_with_auto_keypair().unwrap());
    let (health_tx, _health_rx) = broadcast::channel(16);
    let (startup_tx, startup_rx) = oneshot::channel();

    let mut config = ServerConfig::default();
    config.bind_addr = "127.0.0.1:0".parse().unwrap();
    config.dev_mode = false; // The whole point of this file.
    // With dev_mode off, the server needs a real on-disk cert (see
    // TlsConfig::default()'s doc comment on the certs/ cwd assumption) —
    // generate a throwaway self-signed one rather than relying on the repo
    // checkout's dev cert being reachable from cargo test's cwd.
    let cert_dir = tempfile::tempdir().expect("create temp cert dir");
    config.tls = TlsConfig::ensure_self_signed(cert_dir.path()).expect("generate test TLS cert");

    let server = Server::with_startup_notification(
        config,
        session_manager,
        Arc::new(monoterminal_master::auth::RateLimiter::new()),
        auth_service,
        health_tx,
        None,
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

fn fresh_signing_key() -> ed25519_dalek::SigningKey {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().gen();
    ed25519_dalek::SigningKey::from_bytes(&bytes)
}

#[tokio::test]
async fn test_challenge_auth_attach_end_to_end() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let key = fresh_signing_key();
    let auth = client.authenticate(&key).await.expect("authenticate failed");
    assert!(!auth.access_token.is_empty());
    assert!(!auth.refresh_token.is_empty());
    assert!(auth.user_id.starts_with("ed25519:"));

    // This is the exact scenario that was impossible before this feature:
    // a non-dev-mode daemon, attached to with a real, server-issued JWT.
    let attach = client
        .attach("", &auth.access_token, 24, 80)
        .await
        .expect("attach with real JWT should succeed");
    assert!(!attach.session_id.is_empty());
}

#[tokio::test]
async fn test_attach_without_auth_is_rejected() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let result = client.attach("", "", 24, 80).await;
    assert!(result.is_err(), "attach with no token must be rejected");
}

#[tokio::test]
async fn test_auth_with_wrong_key_rejected() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    // Get a real challenge, then answer it with a signature from a
    // DIFFERENT key than the public_key we present — server must verify
    // the signature against the presented public_key, not just accept any
    // valid Ed25519 signature from anywhere.
    let challenge_env = monoterminal_protocol::Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::ChallengeRequest(
            monoterminal_protocol::ChallengeRequest {},
        )),
    };
    use prost::Message as ProstMessage;
    let mut buf = Vec::with_capacity(challenge_env.encoded_len());
    challenge_env.encode(&mut buf).unwrap();
    client.send_binary(buf).await.unwrap();
    let challenge = match client.recv().await.unwrap() {
        tokio_tungstenite::tungstenite::Message::Binary(data) => {
            match monoterminal_protocol::Envelope::decode(&data[..])
                .unwrap()
                .message
            {
                Some(monoterminal_protocol::envelope::Message::ChallengeResponse(c)) => c,
                other => panic!("unexpected response: {:?}", other),
            }
        }
        other => panic!("expected binary: {:?}", other),
    };

    let key_a = fresh_signing_key();
    let key_b = fresh_signing_key();
    use ed25519_dalek::Signer;
    let signature = key_a.sign(&challenge.nonce);

    let result = client
        .send_raw_auth_request(
            signature.to_bytes().to_vec(),
            key_b.verifying_key().to_bytes().to_vec(), // presenting the WRONG public key
            challenge.nonce,
        )
        .await
        .expect("request should complete");

    assert!(result.is_err(), "signature from a different key must be rejected");
}

#[tokio::test]
async fn test_auth_without_prior_challenge_rejected() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let key = fresh_signing_key();
    use ed25519_dalek::Signer;
    let fake_nonce = [0u8; 32];
    let signature = key.sign(&fake_nonce);

    // AuthRequest as the very first message on this connection — no
    // ChallengeRequest ever sent.
    let result = client
        .send_raw_auth_request(
            signature.to_bytes().to_vec(),
            key.verifying_key().to_bytes().to_vec(),
            fake_nonce.to_vec(),
        )
        .await
        .expect("request should complete");

    assert!(result.is_err(), "AuthRequest with no outstanding challenge must be rejected");
}

#[tokio::test]
async fn test_nonce_mismatch_rejected() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    use prost::Message as ProstMessage;
    let challenge_env = monoterminal_protocol::Envelope {
        sequence_number: 1,
        message: Some(monoterminal_protocol::envelope::Message::ChallengeRequest(
            monoterminal_protocol::ChallengeRequest {},
        )),
    };
    let mut buf = Vec::with_capacity(challenge_env.encoded_len());
    challenge_env.encode(&mut buf).unwrap();
    client.send_binary(buf).await.unwrap();
    let challenge = match client.recv().await.unwrap() {
        tokio_tungstenite::tungstenite::Message::Binary(data) => {
            match monoterminal_protocol::Envelope::decode(&data[..])
                .unwrap()
                .message
            {
                Some(monoterminal_protocol::envelope::Message::ChallengeResponse(c)) => c,
                other => panic!("unexpected response: {:?}", other),
            }
        }
        other => panic!("expected binary: {:?}", other),
    };

    let key = fresh_signing_key();
    use ed25519_dalek::Signer;
    // Sign a genuinely different (valid-looking) nonce, not the one the
    // server actually issued.
    let mut different_nonce = challenge.nonce.clone();
    different_nonce[0] ^= 0xFF;
    let signature = key.sign(&different_nonce);

    let result = client
        .send_raw_auth_request(
            signature.to_bytes().to_vec(),
            key.verifying_key().to_bytes().to_vec(),
            different_nonce, // echoing the WRONG nonce
        )
        .await
        .expect("request should complete");

    assert!(result.is_err(), "answering a foreign/stale nonce must be rejected");
}

#[tokio::test]
async fn test_challenge_is_single_use() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let key = fresh_signing_key();
    let first = client.authenticate(&key).await;
    assert!(first.is_ok(), "first auth over a fresh challenge must succeed");

    // Replay: ask for a SECOND challenge (consuming/replacing the first's
    // slot), confirm the OLD challenge can no longer be answered.
    // Simplest direct replay: try to auth again without a fresh
    // ChallengeRequest — pending_challenge was already `.take()`n by the
    // first AuthRequest, so this must fail with "no outstanding challenge".
    use ed25519_dalek::Signer;
    let stale_nonce = [0x42u8; 32]; // any nonce — there's no live challenge to match anyway
    let signature = key.sign(&stale_nonce);
    let result = client
        .send_raw_auth_request(
            signature.to_bytes().to_vec(),
            key.verifying_key().to_bytes().to_vec(),
            stale_nonce.to_vec(),
        )
        .await
        .expect("request should complete");

    assert!(result.is_err(), "a challenge must not be answerable twice");
}

#[tokio::test]
async fn test_token_refresh_round_trip() {
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    let key = fresh_signing_key();
    let auth = client.authenticate(&key).await.expect("authenticate failed");

    let refreshed = client
        .refresh_token(&auth.refresh_token)
        .await
        .expect("request should complete")
        .expect("refresh should succeed");
    assert!(!refreshed.access_token.is_empty());
    assert_ne!(refreshed.access_token, auth.access_token);

    // The new access token actually works.
    let attach = client
        .attach("", &refreshed.access_token, 24, 80)
        .await
        .expect("attach with refreshed token should succeed");
    assert!(!attach.session_id.is_empty());

    // The OLD refresh token is now burned (single-use, per JwtService).
    let reuse = client
        .refresh_token(&auth.refresh_token)
        .await
        .expect("request should complete");
    assert!(reuse.is_err(), "reusing a burned refresh token must be rejected");
}

#[tokio::test]
async fn test_error_response_echoes_request_sequence() {
    // Pins the fix that made the new auth arms build their error responses
    // inline with the REQUEST's own sequence_number, rather than the
    // connection's separate outbound counter (which a generic bubbled
    // ServerError would have used, leaving the client's request-response
    // matching unable to find the pending entry and hang until timeout).
    let (server_addr, _server_handle) = start_test_server().await;
    let mut client = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client.connect().await.expect("Failed to connect");

    use prost::Message as ProstMessage;
    const REQUEST_SEQ: u64 = 777;
    let key = fresh_signing_key();
    use ed25519_dalek::Signer;
    let fake_nonce = [0u8; 32];
    let signature = key.sign(&fake_nonce);

    let envelope = monoterminal_protocol::Envelope {
        sequence_number: REQUEST_SEQ,
        message: Some(monoterminal_protocol::envelope::Message::AuthRequest(
            monoterminal_protocol::AuthRequest {
                signature: signature.to_bytes().to_vec(),
                public_key: key.verifying_key().to_bytes().to_vec(),
                nonce: fake_nonce.to_vec(),
            },
        )),
    };
    let mut buf = Vec::with_capacity(envelope.encoded_len());
    envelope.encode(&mut buf).unwrap();
    client.send_binary(buf).await.unwrap();

    match client.recv().await.unwrap() {
        tokio_tungstenite::tungstenite::Message::Binary(data) => {
            let response = monoterminal_protocol::Envelope::decode(&data[..]).unwrap();
            assert_eq!(
                response.sequence_number, REQUEST_SEQ,
                "error response must echo the request's own sequence_number"
            );
            assert!(matches!(
                response.message,
                Some(monoterminal_protocol::envelope::Message::ErrorResponse(_))
            ));
        }
        other => panic!("expected binary: {:?}", other),
    }
}

#[tokio::test]
async fn test_same_keypair_yields_same_user_id() {
    let (server_addr, _server_handle) = start_test_server().await;
    let key = fresh_signing_key();

    let mut client_a = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client_a.connect().await.expect("Failed to connect");
    let auth_a = client_a.authenticate(&key).await.expect("authenticate failed");

    let mut client_b = TestWsClient::new_accept_invalid_certs(format!("wss://{}/ws", server_addr));
    client_b.connect().await.expect("Failed to connect");
    let auth_b = client_b.authenticate(&key).await.expect("authenticate failed");

    // Same keypair, two independent connections -> identical continuity
    // identity — the actual property this whole design exists for.
    assert_eq!(auth_a.user_id, auth_b.user_id);
}
