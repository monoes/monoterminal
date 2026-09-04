// Fetches short-lived TURN credentials from the signaling relay's
// `/api/turn-credentials` endpoint, so this daemon can fall back to a
// relayed WebRTC connection when direct STUN-based NAT traversal fails
// (see docs/decisions/011-p2p-networking-architecture.md §3). The daemon
// never holds the relay's TURN shared secret — only the minted,
// time-limited credential the relay hands back.

use serde::Deserialize;

use crate::webrtc::config::TurnServerConfig;
use crate::webrtc::error::{Result, WebRtcError};
use crate::webrtc::pairing::relay_url_to_http;

#[derive(Deserialize)]
struct TurnCredentialsResponse {
    urls: Vec<String>,
    username: String,
    credential: String,
    expires_at: u64,
}

/// Requests fresh TURN credentials from the relay for the given peer_id.
///
/// GET {relay_url as http(s)}/api/turn-credentials?peer_id=...
pub async fn fetch_turn_credentials(relay_url: &str, peer_id: &str) -> Result<TurnServerConfig> {
    let url = format!(
        "{}/api/turn-credentials?peer_id={}",
        relay_url_to_http(relay_url),
        peer_id
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| WebRtcError::Internal(format!("TURN credentials request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(WebRtcError::Internal(format!(
            "TURN credentials request failed: HTTP {} {}",
            status, body
        )));
    }

    let parsed: TurnCredentialsResponse = response.json().await.map_err(|e| {
        WebRtcError::Internal(format!("Failed to parse TURN credentials response: {}", e))
    })?;

    Ok(TurnServerConfig {
        urls: parsed.urls,
        username: parsed.username,
        credential: parsed.credential,
        expires_at_ms: parsed.expires_at * 1000,
    })
}
