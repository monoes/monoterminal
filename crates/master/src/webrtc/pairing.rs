// SaaS device-pairing code client for the signaling relay's REST API.
//
// This is a separate, additive REST call to the relay's `/api/pairing-codes`
// endpoint — unrelated to the WebSocket signaling handshake in
// `signaling_client.rs`. The relay has no concept of daemon identity beyond
// the peer_id string; the daemon never sees account/session state, it only
// asks "give me a pairing code for this peer_id" so a human can link this
// daemon to their SaaS account from a browser.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;

use crate::webrtc::error::{Result, WebRtcError};

/// A pairing code issued by the signaling relay for this daemon's peer_id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCode {
    pub code: String,
    pub expires_at: i64,
}

#[derive(Serialize)]
struct PairingCodeRequest<'a> {
    peer_id: &'a str,
}

/// Converts a signaling relay URL (`ws://` or `wss://`, as used for the
/// WebSocket signaling connection) to the equivalent `http://`/`https://`
/// base URL for REST calls, keeping host:port and path unchanged. Shared
/// with the TURN-credential fetch in this same module.
pub(crate) fn relay_url_to_http(relay_url: &str) -> String {
    if let Some(rest) = relay_url.strip_prefix("wss://") {
        format!("https://{}", rest)
    } else if let Some(rest) = relay_url.strip_prefix("ws://") {
        format!("http://{}", rest)
    } else {
        relay_url.to_string()
    }
}

/// Requests a fresh pairing code from the relay for the given peer_id.
///
/// POST {relay_url as http(s)}/api/pairing-codes
async fn fetch_pairing_code(relay_url: &str, peer_id: &str) -> Result<PairingCode> {
    let url = format!("{}/api/pairing-codes", relay_url_to_http(relay_url));

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&PairingCodeRequest { peer_id })
        .send()
        .await
        .map_err(|e| WebRtcError::Internal(format!("Pairing code request failed: {}", e)))?;

    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(WebRtcError::Internal(
            "rate limited, try again shortly".to_string(),
        ));
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(WebRtcError::Internal(format!(
            "Pairing code request failed: HTTP {} {}",
            status, body
        )));
    }

    response.json::<PairingCode>().await.map_err(|e| {
        WebRtcError::Internal(format!("Failed to parse pairing code response: {}", e))
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Caches the daemon's current pairing code, only fetching a new one from
/// the relay when none is cached yet or the cached one has expired. This
/// keeps repeated dashboard opens from hammering the relay's rate limit.
///
/// Also doubles as this daemon's identity holder: `peer_id()` is always
/// available (the Ed25519 identity key it's derived from always exists),
/// independent of whether a relay is configured at all — a daemon started
/// with no `--relay-url` still has `relay_url: None` here but can still
/// answer "what's my peer_id" over the dashboard command path (used by the
/// browser's local-daemon probe, which needs no relay/account to work).
pub struct PairingCodeCache {
    relay_url: Option<String>,
    peer_id: String,
    current: RwLock<Option<PairingCode>>,
}

impl PairingCodeCache {
    pub fn new(relay_url: Option<String>, peer_id: String) -> Arc<Self> {
        Arc::new(Self {
            relay_url,
            peer_id,
            current: RwLock::new(None),
        })
    }

    /// This daemon's peer_id (Ed25519 pubkey hex) — a pure local read, no
    /// network call, so it's safe to expose over the dashboard command path.
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    /// Returns the cached pairing code if still valid, otherwise fetches and
    /// caches a new one. Errors immediately, with no network call, if this
    /// daemon has no relay configured — pairing/account linking requires one.
    pub async fn get_or_refresh(&self) -> Result<PairingCode> {
        let Some(relay_url) = self.relay_url.as_deref() else {
            return Err(WebRtcError::Internal(
                "P2P is not enabled on this daemon (no --relay-url configured)".to_string(),
            ));
        };

        let mut guard = self.current.write().await;

        if let Some(code) = guard.as_ref() {
            if code.expires_at > now_secs() {
                debug!("Returning cached pairing code");
                return Ok(code.clone());
            }
        }

        let fresh = fetch_pairing_code(relay_url, &self.peer_id).await?;
        *guard = Some(fresh.clone());
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_url_to_http_ws() {
        assert_eq!(
            relay_url_to_http("ws://relay.example.com:9000"),
            "http://relay.example.com:9000"
        );
    }

    #[test]
    fn test_relay_url_to_http_wss() {
        assert_eq!(
            relay_url_to_http("wss://relay.example.com:9000"),
            "https://relay.example.com:9000"
        );
    }

    #[test]
    fn test_relay_url_to_http_with_path() {
        assert_eq!(
            relay_url_to_http("ws://relay.example.com:9000/signal"),
            "http://relay.example.com:9000/signal"
        );
    }

    #[test]
    fn test_peer_id_available_with_no_relay() {
        let cache = PairingCodeCache::new(None, "abc123".to_string());
        assert_eq!(cache.peer_id(), "abc123");
    }

    #[tokio::test]
    async fn test_get_or_refresh_errors_without_network_call_when_no_relay() {
        let cache = PairingCodeCache::new(None, "abc123".to_string());
        let result = cache.get_or_refresh().await;
        assert!(result.is_err());
    }
}
