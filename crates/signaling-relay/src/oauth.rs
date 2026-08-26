//! OAuth 2.0 authorization-code + PKCE login against monoes.me.
//!
//! monoes.me (via `better-auth` + `@better-auth/oauth-provider`) is the sole
//! identity provider for monoterminal accounts. This module runs the
//! browser-redirect flow as a confidential client: it starts the flow,
//! receives the callback, exchanges the code for tokens, resolves the
//! monoes.me user via `/oauth2/userinfo`, and then issues the relay's own
//! short-lived session JWT via `auth::issue_session` — every existing
//! downstream route and the `AuthUser` extractor are unaffected.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth;
use crate::SharedState;

/// How long an in-flight `state`/PKCE pair is honored before it's rejected.
const STATE_TTL_SECONDS: i64 = 10 * 60;

const OAUTH_SCOPE: &str = "openid profile email community:read community:write";

pub struct OAuthConfig {
    monoes_base_url: String,
    client_id: String,
    client_secret: String,
    relay_public_url: String,
    /// Exact-match allowlist for `return_to` — without this, the callback is
    /// an open redirect that hands a session credential to any origin.
    allowed_return_origins: Vec<String>,
}

impl OAuthConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        fn required(name: &str) -> anyhow::Result<String> {
            std::env::var(name).map_err(|_| anyhow::anyhow!("missing required env var {name}"))
        }

        let base_url = required("MONOES_BASE_URL")?;
        let id = required("MONOES_OAUTH_CLIENT_ID")?;
        let credential = required("MONOES_OAUTH_CLIENT_SECRET")?;
        let public_url = required("RELAY_PUBLIC_URL")?;
        let origins = required("RELAY_ALLOWED_RETURN_ORIGINS")?;

        let allowed_return_origins = origins
            .split(',')
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Self {
            monoes_base_url: base_url.trim_end_matches('/').to_string(),
            client_id: id,
            client_secret: credential,
            relay_public_url: public_url.trim_end_matches('/').to_string(),
            allowed_return_origins,
        })
    }

    /// Dummy config for tests that never exercise the network OAuth calls —
    /// they bootstrap a signed-in user by inserting a row directly and
    /// calling `auth::issue_session`, the same way `callback` does.
    pub fn for_tests() -> Self {
        let secret_placeholder = "unused-in-tests".to_string();
        Self {
            monoes_base_url: "http://127.0.0.1:0".to_string(),
            client_id: "test-client".to_string(),
            client_secret: secret_placeholder,
            relay_public_url: "http://127.0.0.1:0".to_string(),
            allowed_return_origins: vec!["http://127.0.0.1:0".to_string()],
        }
    }

    fn redirect_uri(&self) -> String {
        format!("{}/api/oauth/callback", self.relay_public_url)
    }

    fn return_to_allowed(&self, return_to: &str) -> bool {
        self.allowed_return_origins.iter().any(|allowed| {
            return_to == *allowed
                || return_to.starts_with(&format!("{allowed}/"))
                || return_to.starts_with(&format!("{allowed}#"))
        })
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn err(status: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": message })))
}

fn random_urlsafe(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Minimal RFC 3986 percent-encoding for a single query/fragment component.
/// Avoids pulling in a URL-encoding crate solely for this.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn build_authorize_url(cfg: &OAuthConfig, state_param: &str, challenge: &str) -> String {
    format!(
        "{base}/api/auth/oauth2/authorize?response_type=code&client_id={id}&redirect_uri={redirect}&scope={scope}&state={state}&code_challenge={chal}&code_challenge_method=S256",
        base = cfg.monoes_base_url,
        id = percent_encode(&cfg.client_id),
        redirect = percent_encode(&cfg.redirect_uri()),
        scope = percent_encode(OAUTH_SCOPE),
        state = percent_encode(state_param),
        chal = percent_encode(challenge),
    )
}

#[derive(Deserialize)]
pub struct StartQuery {
    return_to: String,
}

/// `GET /api/oauth/start` — begins the login flow. Returns the monoes.me
/// authorize URL for the browser to navigate to.
pub async fn start(
    State(state): State<Arc<SharedState>>,
    Query(query): Query<StartQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !state.oauth.return_to_allowed(&query.return_to) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "return_to is not an allowed origin",
        ));
    }

    let state_param = random_urlsafe(32);
    let verifier = random_urlsafe(64);
    let challenge = pkce_challenge(&verifier);

    let conn = state
        .db
        .get_conn()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;
    conn.execute(
        "INSERT INTO oauth_states (state, code_verifier, return_to, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![state_param, verifier, query.return_to, now_secs()],
    )
    .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to persist oauth state"))?;

    let authorize_url = build_authorize_url(&state.oauth, &state_param, &challenge);
    Ok(Json(json!({ "authorize_url": authorize_url })))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokenExchangeResult {
    access_token: String,
}

#[derive(Deserialize)]
struct MonoesUser {
    sub: String,
    email: String,
}

/// `GET /api/oauth/callback` — monoes.me redirects here with `code`/`state`
/// after login+consent. Exchanges the code, resolves the user, issues a
/// relay session JWT, and redirects back to the web app with it in the URL
/// fragment (never a query param or cookie — see module docs / plan for why).
pub async fn callback(
    State(state): State<Arc<SharedState>>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    match callback_inner(state, query).await {
        Ok(redirect) => redirect.into_response(),
        Err((status, body)) => (status, body).into_response(),
    }
}

async fn callback_inner(
    state: Arc<SharedState>,
    query: CallbackQuery,
) -> Result<Redirect, (StatusCode, Json<Value>)> {
    if let Some(denial_reason) = query.error {
        return Err(err(
            StatusCode::BAD_REQUEST,
            &format!("monoes.me denied the request: {denial_reason}"),
        ));
    }
    let auth_code = query
        .code
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "missing code"))?;
    let state_param = query
        .state
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "missing state"))?;

    let (verifier, return_to) = {
        let conn = state
            .db
            .get_conn()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;

        let row: Option<(String, String, i64)> = conn
            .query_row(
                "SELECT code_verifier, return_to, created_at FROM oauth_states WHERE state = ?1",
                rusqlite::params![state_param],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

        // One-shot: consume the state regardless of outcome so a leaked
        // callback URL can't be replayed.
        conn.execute(
            "DELETE FROM oauth_states WHERE state = ?1",
            rusqlite::params![state_param],
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

        let (verifier, return_to, created_at) =
            row.ok_or_else(|| err(StatusCode::BAD_REQUEST, "unknown or expired state"))?;

        if now_secs() - created_at > STATE_TTL_SECONDS {
            return Err(err(StatusCode::BAD_REQUEST, "state expired"));
        }

        (verifier, return_to)
    };

    // Re-validated here too: the allowlist may have changed since `start`,
    // and this is the value we're about to redirect a session credential to.
    if !state.oauth.return_to_allowed(&return_to) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "return_to is not an allowed origin",
        ));
    }

    let http = reqwest::Client::new();

    let exchange: TokenExchangeResult = http
        .post(format!(
            "{}/api/auth/oauth2/token",
            state.oauth.monoes_base_url
        ))
        .basic_auth(&state.oauth.client_id, Some(&state.oauth.client_secret))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", auth_code.as_str()),
            ("redirect_uri", state.oauth.redirect_uri().as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|_| {
            err(
                StatusCode::BAD_GATEWAY,
                "failed to reach monoes.me token endpoint",
            )
        })?
        .error_for_status()
        .map_err(|_| {
            err(
                StatusCode::BAD_GATEWAY,
                "monoes.me rejected the authorization code",
            )
        })?
        .json()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "malformed token exchange response"))?;

    let profile: MonoesUser = http
        .get(format!(
            "{}/api/auth/oauth2/userinfo",
            state.oauth.monoes_base_url
        ))
        .bearer_auth(&exchange.access_token)
        .send()
        .await
        .map_err(|_| {
            err(
                StatusCode::BAD_GATEWAY,
                "failed to reach monoes.me userinfo endpoint",
            )
        })?
        .error_for_status()
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "failed to fetch user info"))?
        .json()
        .await
        .map_err(|_| err(StatusCode::BAD_GATEWAY, "malformed userinfo response"))?;

    {
        let conn = state
            .db
            .get_conn()
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "database unavailable"))?;
        conn.execute(
            "INSERT INTO users (id, email, created_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET email = excluded.email",
            rusqlite::params![profile.sub, profile.email, now_secs()],
        )
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to upsert user"))?;
    }

    let relay_session =
        auth::issue_session(&state.jwt_signing_key, &profile.sub, &profile.email)
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "failed to issue session"))?;

    let separator = if return_to.contains('#') { '&' } else { '#' };
    let fragment_token = percent_encode(&relay_session);
    let fragment_email = percent_encode(&profile.email);
    let redirect_url =
        format!("{return_to}{separator}monoterminal_token={fragment_token}&email={fragment_email}");

    Ok(Redirect::to(&redirect_url))
}
