// WebSocket Server with TLS 1.3
// Implements SRS §3.1.2 (WebSocket Framing) and §3.2.1 (Transport Security)
// Implements SRS §3.2.4 (Rate Limiting)

#![allow(dead_code)] // Server features not all integrated yet, cleanup tracked in task-63

pub mod connection;
pub mod error;
pub mod handler;
pub mod health;
pub mod tls; // Phase 2: Health check endpoints

#[cfg(test)]
mod tests;

pub use error::{Result, ServerError};
pub use tls::TlsConfig;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use crate::auth::{AuthService, Ed25519AuthService, RateLimiter};
use crate::session::manager::SessionManager;
use monoterminal_monomind_bridge::HealthStatus;

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_addr: SocketAddr,

    /// TLS configuration
    pub tls: TlsConfig,

    /// Maximum concurrent connections (SRS §2.3.4: 1000 global limit)
    pub max_connections: usize,

    /// Connection rate limit (SRS §2.3.4: 100/minute)
    pub rate_limit_per_minute: u32,

    /// Development mode (bypasses Ed25519 challenge-response auth)
    /// WARNING: Auto-issues JWT tokens without signature verification.
    /// DO NOT use in production - for E2E testing only.
    pub dev_mode: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            // Phase 1: local only - 127.0.0.1:5000 per eng-director
            bind_addr: "127.0.0.1:5000".parse().unwrap(),
            tls: TlsConfig::default(),
            max_connections: 1000,
            rate_limit_per_minute: 100,
            dev_mode: false,
        }
    }
}

/// WebSocket server with TLS 1.3 support
pub struct Server {
    config: ServerConfig,
    session_manager: Arc<SessionManager>,
    tls_acceptor: TlsAcceptor,
    rate_limiter: Arc<RateLimiter>,
    auth_service: Arc<Ed25519AuthService>,
    #[allow(dead_code)]
    health_tx: broadcast::Sender<HealthStatus>,
    dev_mode: bool,
    /// Optional startup notification (sent once TCP listener is bound)
    startup_tx: Option<oneshot::Sender<SocketAddr>>,
}

impl Server {
    /// Create a new WebSocket server
    pub fn new(
        config: ServerConfig,
        session_manager: Arc<SessionManager>,
        rate_limiter: Arc<RateLimiter>,
        auth_service: Arc<Ed25519AuthService>,
        health_tx: broadcast::Sender<HealthStatus>,
    ) -> Result<Self> {
        // Use embedded test certificates in dev_mode to avoid filesystem dependencies
        let tls_acceptor = if config.dev_mode {
            TlsConfig::build_dev_acceptor()?
        } else {
            config.tls.build_acceptor()?
        };
        let dev_mode = config.dev_mode;

        Ok(Self {
            config,
            session_manager,
            tls_acceptor,
            rate_limiter,
            auth_service,
            health_tx,
            dev_mode,
            startup_tx: None,
        })
    }

    /// Create a new WebSocket server with startup notification
    ///
    /// The provided oneshot sender will receive the bound SocketAddr once
    /// the TCP listener is successfully created. This allows callers (e.g.,
    /// benchmarks) to wait for server readiness instead of blind sleeps.
    pub fn with_startup_notification(
        config: ServerConfig,
        session_manager: Arc<SessionManager>,
        rate_limiter: Arc<RateLimiter>,
        auth_service: Arc<Ed25519AuthService>,
        health_tx: broadcast::Sender<HealthStatus>,
        startup_tx: oneshot::Sender<SocketAddr>,
    ) -> Result<Self> {
        // Use embedded test certificates in dev_mode to avoid filesystem dependencies
        let tls_acceptor = if config.dev_mode {
            TlsConfig::build_dev_acceptor()?
        } else {
            config.tls.build_acceptor()?
        };
        let dev_mode = config.dev_mode;

        Ok(Self {
            config,
            session_manager,
            tls_acceptor,
            rate_limiter,
            auth_service,
            health_tx,
            dev_mode,
            startup_tx: Some(startup_tx),
        })
    }

    /// Start the WebSocket server
    pub async fn run(mut self) -> Result<()> {
        debug!(
            "Attempting to bind TCP listener to {}",
            self.config.bind_addr
        );

        let listener = TcpListener::bind(self.config.bind_addr)
            .await
            .map_err(|e| {
                error!("Failed to bind to {}: {}", self.config.bind_addr, e);
                e
            })?;

        let bound_addr = listener.local_addr().map_err(|e| {
            error!("Failed to get local address: {}", e);
            e
        })?;

        info!("WebSocket server listening on {}", bound_addr);
        info!("TLS 1.3 only, cipher suites: TLS_AES_256_GCM_SHA384, TLS_AES_128_GCM_SHA256, TLS_CHACHA20_POLY1305_SHA256");

        // Send startup notification if channel exists
        if let Some(tx) = self.startup_tx.take() {
            debug!("Sending startup notification for {}", bound_addr);
            let _ = tx.send(bound_addr); // Ignore send errors (receiver may have dropped)
        }

        let server = Arc::new(self);
        let connection_count = Arc::new(tokio::sync::Semaphore::new(server.config.max_connections));

        loop {
            // Acquire connection permit (blocks if max connections reached)
            let permit = connection_count
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| {
                    ServerError::Internal(format!("Failed to acquire connection permit: {}", e))
                })?;

            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    let server = Arc::clone(&server);

                    // Spawn connection handler
                    tokio::spawn(async move {
                        info!("New connection from {}", peer_addr);

                        if let Err(e) = server.handle_connection(stream, peer_addr).await {
                            error!("Connection error from {}: {}", peer_addr, e);
                        }

                        drop(permit); // Release connection slot
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single connection
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<()> {
        // Check connection rate limit (SRS §3.2.4: 100 connections/min per IP)
        self.rate_limiter
            .check_connection(&peer_addr)
            .map_err(|e| {
                warn!("Connection rate limit exceeded for {}: {}", peer_addr, e);
                ServerError::RateLimitExceeded
            })?;

        // TLS handshake
        let tls_stream = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| ServerError::TlsHandshake(e.to_string()))?;

        info!("TLS handshake completed with {}", peer_addr);

        // WebSocket upgrade
        let ws_stream = tokio_tungstenite::accept_async(tls_stream)
            .await
            .map_err(|e| ServerError::WebSocketUpgrade(e.to_string()))?;

        info!("WebSocket handshake completed with {}", peer_addr);

        // Handle WebSocket messages
        handler::handle_websocket(
            ws_stream,
            peer_addr,
            Arc::clone(&self.session_manager),
            Arc::clone(&self.auth_service) as Arc<dyn AuthService>,
            self.dev_mode,
        )
        .await
    }
}
