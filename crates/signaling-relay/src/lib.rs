mod accounts;
mod auth;
mod db;
mod handler;
mod oauth;
mod protocol;
mod state;
mod turn;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::routing::{delete, get, post};
use axum::Router;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

pub use auth::{issue_session, load_or_generate_signing_key};
pub use db::Database;
pub use oauth::OAuthConfig;
pub use state::AppState;
pub use turn::load_or_generate_turn_secret;

/// In-memory rolling-window rate limiter for the pairing-codes endpoint —
/// keyed by peer_id, doesn't need to be persistent or fancy.
#[derive(Default)]
pub struct PairingRateLimiter {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl PairingRateLimiter {
    const WINDOW_SECS: u64 = 60;
    const MAX_PER_WINDOW: usize = 5;

    /// Records a request for `peer_id` and returns `true` if it is allowed
    /// under the rolling 60s / 5-request limit, `false` if it should be
    /// rejected.
    pub async fn check(&self, peer_id: &str) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(Self::WINDOW_SECS);
        let mut hits = self.hits.lock().await;
        let entry = hits.entry(peer_id.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() >= Self::MAX_PER_WINDOW {
            return false;
        }
        entry.push(now);
        true
    }
}

/// Combined Axum state: the existing WS relay state plus the new accounts
/// database and JWT signing key, so both the `/ws` route and the new REST
/// routes can share one router.
pub struct SharedState {
    pub relay: Arc<AppState>,
    pub db: Arc<Database>,
    pub jwt_signing_key: Arc<Vec<u8>>,
    pub pairing_rate_limiter: PairingRateLimiter,
    pub oauth: OAuthConfig,
    /// coturn's `static-auth-secret` value — used only to mint short-lived
    /// per-request credentials, never handed to a client directly.
    pub turn_shared_secret: String,
    /// `host:port` clients should connect to for TURN, e.g.
    /// `91.99.106.218:3478`.
    pub turn_server_host: String,
}

static TEST_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Convenience constructor used by existing WS-only tests: builds a full
/// router backed by an ephemeral, uniquely-named SQLite file so parallel
/// test processes don't collide.
pub fn build_router() -> Router {
    let n = TEST_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let db_path = std::env::temp_dir().join(format!(
        "signaling-relay-{}-{}.db",
        std::process::id(),
        n
    ));
    let db = Arc::new(Database::new(&db_path).expect("failed to initialize database"));
    let key = load_or_generate_signing_key();
    let shared = Arc::new(SharedState {
        relay: AppState::new(),
        db,
        jwt_signing_key: Arc::new(key),
        pairing_rate_limiter: PairingRateLimiter::default(),
        oauth: OAuthConfig::for_tests(),
        turn_shared_secret: load_or_generate_turn_secret(),
        turn_server_host: "127.0.0.1:3478".to_string(),
    });
    build_router_with_state(shared)
}

pub fn build_router_with_state(state: Arc<SharedState>) -> Router {
    // Permissive CORS: this API is Bearer-token authenticated (never
    // cookies), so allowing any origin doesn't open a CSRF hole — a
    // cross-origin page can't read/attach the caller's token without the
    // caller's own JS already having it. Self-hosted deployments have no
    // single fixed frontend origin to allowlist ahead of time anyway.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(handler::ws_handler))
        .route("/api/oauth/start", get(oauth::start))
        .route("/api/oauth/callback", get(oauth::callback))
        .route("/api/pairing-codes", post(accounts::create_pairing_code))
        .route("/api/link", post(accounts::link_computer))
        .route("/api/computers", get(accounts::list_computers))
        .route("/api/computers/:id", delete(accounts::delete_computer))
        .route("/api/turn-credentials", get(turn::get_turn_credentials))
        .layer(cors)
        .with_state(state)
}
