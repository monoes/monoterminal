use std::sync::Arc;

use clap::Parser;
use monoterminal_signaling_relay::{
    build_router_with_state, load_or_generate_signing_key, load_or_generate_turn_secret,
    AppState, Database, OAuthConfig, PairingRateLimiter, SharedState,
};

/// MONOTERMINAL WebRTC signaling relay
#[derive(Parser, Debug)]
#[command(name = "signaling-relay")]
#[command(about = "Minimal WebSocket signaling relay for WebRTC peer setup", long_about = None)]
#[command(version)]
struct Args {
    /// Address to bind the WebSocket server to
    #[arg(long, default_value = "0.0.0.0:9000")]
    bind_addr: String,

    /// Path to the accounts/pairing SQLite database file
    #[arg(long, default_value = "./signaling-relay.db")]
    db_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).compact().init();

    let args = Args::parse();

    let db = Arc::new(Database::new(&args.db_path)?);
    let oauth = OAuthConfig::from_env()?;
    let turn_server_host = std::env::var("TURN_SERVER_HOST")
        .map_err(|_| anyhow::anyhow!("missing required env var TURN_SERVER_HOST"))?;
    let shared = Arc::new(SharedState {
        relay: AppState::new(),
        db,
        jwt_signing_key: Arc::new(load_or_generate_signing_key()),
        pairing_rate_limiter: PairingRateLimiter::default(),
        oauth,
        turn_shared_secret: load_or_generate_turn_secret(),
        turn_server_host,
    });

    let router = build_router_with_state(shared);
    let listener = tokio::net::TcpListener::bind(&args.bind_addr).await?;

    tracing::info!(bind_addr = %args.bind_addr, db_path = %args.db_path, "signaling relay listening");

    axum::serve(listener, router).await?;

    Ok(())
}
