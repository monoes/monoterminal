// Directory Service HTTP Client
// ADR-011 §4.2: Internet Discovery (Directory Service)
// Endpoints: POST/GET/DELETE /api/v1/peers

use crate::discovery::error::{DiscoveryError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Peer endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEndpoint {
    /// Endpoint type ("websocket" | "webrtc")
    #[serde(rename = "type")]
    pub endpoint_type: String,

    /// Endpoint URL (e.g., "wss://203.0.113.45:9443")
    pub url: String,

    /// Whether endpoint has been verified by directory
    pub verified: bool,
}

/// Peer registration information
///
/// ADR-011 §4.2: Directory Service Registration Payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationInfo {
    /// Ed25519 public key (hex-encoded)
    pub peer_id: String,

    /// List of endpoints (WebSocket, WebRTC)
    pub endpoints: Vec<PeerEndpoint>,

    /// TTL in seconds (default: 3600 = 1 hour)
    pub ttl_seconds: u64,

    /// Ed25519 signature over payload (prevents spoofing)
    pub signature: String,
}

/// Peer lookup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerLookupResponse {
    /// Ed25519 public key
    pub peer_id: String,

    /// Available endpoints
    pub endpoints: Vec<PeerEndpoint>,

    /// Whether any endpoints are verified
    pub verified: bool,
}

/// Directory service HTTP client
///
/// ADR-011 §4.2: Directory Service Design
/// Base URL: https://directory.monoterminal.io
pub struct DirectoryClient {
    base_url: String,
    client: reqwest::Client,
    timeout: Duration,
    max_retries: u32,
}

impl DirectoryClient {
    /// Create new directory service client
    pub fn new(base_url: String) -> Self {
        let timeout = Duration::from_secs(10);

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            base_url,
            client,
            timeout,
            max_retries: 3,
        }
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;

        // Rebuild client with new timeout
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("Failed to build reqwest client");

        self
    }

    /// Set max retries for exponential backoff
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Register peer with directory service
    ///
    /// POST /api/v1/peers/register
    ///
    /// ADR-011 §4.2: Ed25519 signature required to prevent spoofing
    pub async fn register(&self, info: RegistrationInfo) -> Result<()> {
        let url = format!("{}/api/v1/peers/register", self.base_url);

        info!(
            "Registering with directory service: {} (peer_id: {})",
            url, info.peer_id
        );

        debug!("Registration payload: {:?}", info);

        // Retry with exponential backoff
        let mut attempt = 0;
        let mut backoff = Duration::from_millis(100);

        loop {
            match self.client.post(&url).json(&info).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Directory registration successful");
                        return Ok(());
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();

                        if status.is_client_error() {
                            // 4xx errors - don't retry
                            return Err(DiscoveryError::DirectoryRegistrationFailed(format!(
                                "HTTP {}: {}",
                                status, body
                            )));
                        }

                        // 5xx errors - retry
                        warn!(
                            "Directory registration failed (attempt {}): HTTP {}",
                            attempt + 1,
                            status
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Directory registration error (attempt {}): {}",
                        attempt + 1,
                        e
                    );
                }
            }

            attempt += 1;
            if attempt >= self.max_retries {
                return Err(DiscoveryError::DirectoryRegistrationFailed(format!(
                    "Max retries ({}) exceeded",
                    self.max_retries
                )));
            }

            // Exponential backoff
            tokio::time::sleep(backoff).await;
            backoff = backoff * 2;
        }
    }

    /// Lookup peer by Ed25519 public key
    ///
    /// GET /api/v1/peers/{peer_id}
    ///
    /// Returns peer's endpoints (WebSocket, WebRTC)
    pub async fn lookup(&self, peer_id: &str) -> Result<PeerLookupResponse> {
        let url = format!("{}/api/v1/peers/{}", self.base_url, peer_id);

        debug!("Looking up peer in directory: {}", peer_id);

        // Single attempt for lookup (no retry on read operations)
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DiscoveryError::DirectoryLookupFailed(e.to_string()))?;

        if response.status().is_success() {
            let peer_info = response.json::<PeerLookupResponse>().await.map_err(|e| {
                DiscoveryError::DirectoryLookupFailed(format!("Failed to parse response: {}", e))
            })?;

            info!("Directory lookup successful: peer_id={}", peer_id);
            Ok(peer_info)
        } else if response.status() == 404 {
            Err(DiscoveryError::ServiceNotFound(format!(
                "Peer not found: {}",
                peer_id
            )))
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(DiscoveryError::DirectoryLookupFailed(format!(
                "HTTP {}: {}",
                status, body
            )))
        }
    }

    /// Deregister peer (cleanup on shutdown)
    ///
    /// DELETE /api/v1/peers/{peer_id}
    pub async fn deregister(&self, peer_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/peers/{}", self.base_url, peer_id);

        info!("Deregistering from directory service: {}", peer_id);

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| DiscoveryError::DirectoryUnavailable(e.to_string()))?;

        if response.status().is_success() || response.status() == 404 {
            // Success or already gone - both are OK
            info!("Directory deregistration successful");
            Ok(())
        } else {
            let status = response.status();
            warn!("Directory deregistration failed: HTTP {}", status);
            // Don't fail on deregistration errors (cleanup is best-effort)
            Ok(())
        }
    }

    /// Health check directory service
    ///
    /// GET /health
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);

        debug!("Directory service health check: {}", url);

        match tokio::time::timeout(Duration::from_secs(2), self.client.get(&url).send()).await {
            Ok(Ok(response)) if response.status().is_success() => {
                debug!("Directory service is healthy");
                Ok(true)
            }
            Ok(Ok(response)) => {
                warn!("Directory service unhealthy: HTTP {}", response.status());
                Ok(false)
            }
            Ok(Err(e)) => {
                warn!("Directory service unreachable: {}", e);
                Ok(false)
            }
            Err(_) => {
                warn!("Directory service health check timeout");
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_endpoint_creation() {
        let endpoint = PeerEndpoint {
            endpoint_type: "websocket".to_string(),
            url: "wss://example.com:9443".to_string(),
            verified: true,
        };

        assert_eq!(endpoint.endpoint_type, "websocket");
        assert!(endpoint.verified);
    }

    #[test]
    fn test_registration_info_serialization() {
        let info = RegistrationInfo {
            peer_id: "ed25519:test123".to_string(),
            endpoints: vec![PeerEndpoint {
                endpoint_type: "websocket".to_string(),
                url: "wss://test.local:9443".to_string(),
                verified: false,
            }],
            ttl_seconds: 3600,
            signature: "signature_here".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test123"));
    }

    #[tokio::test]
    async fn test_directory_client_creation() {
        let client = DirectoryClient::new("https://directory.test.io".to_string());
        assert_eq!(client.base_url, "https://directory.test.io");
    }
}
