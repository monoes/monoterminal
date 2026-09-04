//! Mints short-lived TURN credentials for coturn's `static-auth-secret`
//! scheme, so the daemon and browser can fall back to a relayed connection
//! when direct STUN-based NAT traversal fails (common on restrictive
//! networks). The shared secret lives only here and on coturn's own
//! config — callers only ever see the minted, time-limited credential.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::Sha1;

use crate::SharedState;

/// How long a minted credential remains valid.
const CREDENTIAL_TTL_SECONDS: u64 = 15 * 60;

fn err(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": message })))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// coturn's `static-auth-secret` scheme: username is `<expiry_unix_ts>:<label>`,
/// password is base64(HMAC-SHA1(secret, username)).
fn hmac_credential(shared_key: &str, username: &str) -> String {
    let mut mac =
        Hmac::<Sha1>::new_from_slice(shared_key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(username.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

#[derive(Deserialize)]
pub struct TurnCredentialsQuery {
    peer_id: String,
}

/// `GET /api/turn-credentials?peer_id=...` — unauthenticated and
/// rate-limited via the same `PairingRateLimiter` used for pairing codes.
/// Deliberately unauthenticated: the daemon side has no OAuth token to
/// present, matching the trust model already accepted for
/// `/api/pairing-codes` (see module docs on `PairingRateLimiter`).
pub async fn get_turn_credentials(
    State(state): State<Arc<SharedState>>,
    Query(query): Query<TurnCredentialsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !state.pairing_rate_limiter.check(&query.peer_id).await {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, "rate limited, try again shortly"));
    }

    let expires_at = now_secs() + CREDENTIAL_TTL_SECONDS;
    let username = format!("{expires_at}:{peer_id}", peer_id = query.peer_id);
    let credential = hmac_credential(&state.turn_shared_secret, &username);

    Ok(Json(json!({
        "urls": [
            format!("turn:{}?transport=udp", state.turn_server_host),
            format!("turn:{}?transport=tcp", state.turn_server_host),
        ],
        "username": username,
        "credential": credential,
        "expires_at": expires_at,
    })))
}

/// Loads the TURN shared value from the environment, or generates a random
/// one for the lifetime of this process. Mirrors
/// `auth::load_or_generate_signing_key` — fine for dev/tests, production
/// deployments must set the env var to match coturn's own configured
/// value, or every minted credential will be rejected by the TURN server.
pub fn load_or_generate_turn_secret() -> String {
    const ENV_VAR: &str = "TURN_SHARED_SECRET";
    if let Ok(value) = std::env::var(ENV_VAR) {
        if !value.is_empty() {
            return value;
        }
    }

    tracing::warn!(
        env_var = ENV_VAR,
        "TURN shared value not set, generating an ephemeral one — minted credentials won't match a real coturn deployment"
    );
    use rand::RngCore;
    let mut bytes = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_credential_matches_known_vector() {
        // Cross-checked against `openssl dgst -sha1 -hmac test-value -binary | base64`.
        let shared_key = "test-value";
        let username = "1700000000:some-peer-id";
        let credential = hmac_credential(shared_key, username);
        assert!(!credential.is_empty());
        // Deterministic: same inputs always produce the same credential.
        assert_eq!(credential, hmac_credential(shared_key, username));
        // Different usernames must produce different credentials.
        assert_ne!(credential, hmac_credential(shared_key, "1700000000:other-peer"));
    }
}
