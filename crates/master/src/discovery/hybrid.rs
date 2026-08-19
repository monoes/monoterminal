// Hybrid Discovery (mDNS + Directory)
// ADR-011 §4.3: Discovery Priority Order
// Race mDNS vs Directory, first to respond wins

use crate::discovery::directory::{DirectoryClient, PeerEndpoint, RegistrationInfo};
use crate::discovery::error::{DiscoveryError, Result};
use crate::discovery::mdns::{MdnsDiscovery, ServiceInfo};
use crate::discovery::DiscoveryConfig;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Discovery method that succeeded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMethod {
    /// Discovered via mDNS (LAN)
    Mdns,
    /// Discovered via directory service (internet)
    Directory,
    /// Manual configuration (fallback)
    Manual,
}

/// Discovery result
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// How the peer was discovered
    pub method: DiscoveryMethod,

    /// WebSocket URL (e.g., "wss://peer.local:9443")
    pub websocket_url: String,

    /// Ed25519 peer_id (if available)
    pub peer_id: Option<String>,

    /// Discovery latency (milliseconds)
    pub latency_ms: u64,
}

/// Hybrid discovery client
///
/// ADR-011 §4.3: Parallel race between mDNS and directory
pub struct HybridDiscovery {
    config: DiscoveryConfig,
    mdns: Option<MdnsDiscovery>,
    directory: Option<DirectoryClient>,
}

impl HybridDiscovery {
    /// Create new hybrid discovery client
    pub fn new(config: DiscoveryConfig) -> Result<Self> {
        // Initialize mDNS if enabled
        let mdns = if config.enable_mdns {
            Some(MdnsDiscovery::new(
                config.service_type.clone(),
                config.service_name.clone(),
                9443, // Default port
            ))
        } else {
            None
        };

        // Initialize directory client if enabled and URL provided
        let directory = if config.enable_directory {
            config
                .directory_url
                .as_ref()
                .map(|url| DirectoryClient::new(url.clone()).with_timeout(config.discovery_timeout))
        } else {
            None
        };

        if mdns.is_none() && directory.is_none() {
            return Err(DiscoveryError::NoDiscoveryMethods);
        }

        Ok(Self {
            config,
            mdns,
            directory,
        })
    }

    /// Discover master peer
    ///
    /// ADR-011 §4.3: Race mDNS vs Directory (first to respond wins)
    pub async fn discover_master(&self, peer_id: &str) -> Result<DiscoveryResult> {
        info!("Starting hybrid discovery for peer: {}", peer_id);

        let start = std::time::Instant::now();

        // Step 1: Try mDNS (parallel with directory, race them)
        let mdns_future = self.discover_via_mdns(peer_id);
        let directory_future = self.discover_via_directory(peer_id);

        // Step 2: Race mDNS vs Directory (first to respond wins)
        let result = tokio::select! {
            Ok(result) = mdns_future => {
                info!("Discovered master via mDNS: {}", result.websocket_url);
                result
            }
            Ok(result) = directory_future => {
                info!("Discovered master via directory service: {}", result.websocket_url);
                result
            }
            else => {
                // Step 3: Fallback to manual configuration
                warn!("All discovery methods failed, trying manual configuration");
                return self.get_manual_endpoint();
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;
        debug!(
            "Discovery completed in {}ms via {:?}",
            latency_ms, result.method
        );

        Ok(DiscoveryResult {
            latency_ms,
            ..result
        })
    }

    /// Discover via mDNS (LAN)
    async fn discover_via_mdns(&self, _peer_id: &str) -> Result<DiscoveryResult> {
        if let Some(ref mdns) = self.mdns {
            debug!("Attempting mDNS discovery");

            match mdns.discover(self.config.discovery_timeout).await {
                Ok(services) if !services.is_empty() => {
                    // Return first discovered service
                    let service = &services[0];
                    Ok(DiscoveryResult {
                        method: DiscoveryMethod::Mdns,
                        websocket_url: service.websocket_url(),
                        peer_id: service.properties.get("peer_id").cloned(),
                        latency_ms: 0, // Will be set by caller
                    })
                }
                Ok(_) => {
                    debug!("mDNS discovery returned no results");
                    Err(DiscoveryError::ServiceNotFound(
                        "No mDNS services found".to_string(),
                    ))
                }
                Err(e) => {
                    debug!("mDNS discovery failed: {}", e);
                    Err(e)
                }
            }
        } else {
            // mDNS not enabled, wait forever (directory will win the race)
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Err(DiscoveryError::MdnsDiscoveryFailed(
                "mDNS not enabled".to_string(),
            ))
        }
    }

    /// Discover via directory service (internet)
    async fn discover_via_directory(&self, peer_id: &str) -> Result<DiscoveryResult> {
        if let Some(ref directory) = self.directory {
            debug!("Attempting directory service discovery");

            match directory.lookup(peer_id).await {
                Ok(response) if !response.endpoints.is_empty() => {
                    // Return first WebSocket endpoint
                    let endpoint = response
                        .endpoints
                        .iter()
                        .find(|e| e.endpoint_type == "websocket")
                        .or(response.endpoints.first())
                        .ok_or_else(|| {
                            DiscoveryError::DirectoryLookupFailed("No endpoints found".to_string())
                        })?;

                    Ok(DiscoveryResult {
                        method: DiscoveryMethod::Directory,
                        websocket_url: endpoint.url.clone(),
                        peer_id: Some(response.peer_id),
                        latency_ms: 0, // Will be set by caller
                    })
                }
                Ok(_) => {
                    debug!("Directory lookup returned no endpoints");
                    Err(DiscoveryError::ServiceNotFound(
                        "No endpoints in directory".to_string(),
                    ))
                }
                Err(e) => {
                    debug!("Directory lookup failed: {}", e);
                    Err(e)
                }
            }
        } else {
            // Directory not enabled, wait forever (mDNS will win the race)
            tokio::time::sleep(Duration::from_secs(3600)).await;
            Err(DiscoveryError::DirectoryUnavailable(
                "Directory not enabled".to_string(),
            ))
        }
    }

    /// Fallback to manual configuration
    ///
    /// ADR-011 §4.3: Environment variable MONOTERMINAL_MASTER_URL
    fn get_manual_endpoint(&self) -> Result<DiscoveryResult> {
        if let Ok(url) = std::env::var("MONOTERMINAL_MASTER_URL") {
            info!("Using manual configuration: {}", url);
            Ok(DiscoveryResult {
                method: DiscoveryMethod::Manual,
                websocket_url: url,
                peer_id: None,
                latency_ms: 0,
            })
        } else {
            Err(DiscoveryError::AllMethodsFailed)
        }
    }

    /// Register this peer with available discovery services
    pub async fn register(&self, peer_id: String, port: u16) -> Result<()> {
        info!("Registering peer {} on port {}", peer_id, port);

        let mut mdns_ok = false;
        let mut directory_ok = false;

        // Register with mDNS if available
        if let Some(ref mdns) = self.mdns {
            let mut properties = std::collections::HashMap::new();
            properties.insert("version".to_string(), "1.0".to_string());
            properties.insert("peer_id".to_string(), peer_id.clone());
            properties.insert("protocol".to_string(), "ws+wss+webrtc".to_string());

            match mdns.register(peer_id.clone(), properties).await {
                Ok(_) => {
                    info!("mDNS registration successful");
                    mdns_ok = true;
                }
                Err(e) => {
                    warn!("mDNS registration failed: {}", e);
                }
            }
        }

        // Register with directory if available
        if let Some(ref directory) = self.directory {
            // TODO: Generate Ed25519 signature
            let registration = RegistrationInfo {
                peer_id: peer_id.clone(),
                endpoints: vec![PeerEndpoint {
                    endpoint_type: "websocket".to_string(),
                    url: format!("wss://localhost:{}", port),
                    verified: false,
                }],
                ttl_seconds: self.config.ttl_seconds,
                signature: "TODO_SIGN_WITH_ED25519".to_string(),
            };

            match directory.register(registration).await {
                Ok(_) => {
                    info!("Directory registration successful");
                    directory_ok = true;
                }
                Err(e) => {
                    warn!("Directory registration failed: {}", e);
                }
            }
        }

        // Graceful degradation: succeed if at least one method worked
        if mdns_ok || directory_ok {
            Ok(())
        } else {
            Err(DiscoveryError::AllMethodsFailed)
        }
    }

    /// Unregister from all discovery services (cleanup on shutdown)
    pub async fn unregister(&self, peer_id: &str) -> Result<()> {
        info!("Unregistering peer: {}", peer_id);

        // Unregister from mDNS
        if let Some(ref mdns) = self.mdns {
            let _ = mdns.unregister().await;
        }

        // Unregister from directory
        if let Some(ref directory) = self.directory {
            let _ = directory.deregister(peer_id).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hybrid_discovery_creation() {
        let config = DiscoveryConfig {
            enable_mdns: true,
            enable_directory: false,
            ..Default::default()
        };

        let discovery = HybridDiscovery::new(config).unwrap();
        assert!(discovery.mdns.is_some());
        assert!(discovery.directory.is_none());
    }

    #[tokio::test]
    async fn test_hybrid_discovery_no_methods_error() {
        let config = DiscoveryConfig {
            enable_mdns: false,
            enable_directory: false,
            ..Default::default()
        };

        let result = HybridDiscovery::new(config);
        assert!(matches!(result, Err(DiscoveryError::NoDiscoveryMethods)));
    }

    #[test]
    fn test_discovery_result() {
        let result = DiscoveryResult {
            method: DiscoveryMethod::Mdns,
            websocket_url: "wss://test.local:9443".to_string(),
            peer_id: Some("ed25519:test".to_string()),
            latency_ms: 150,
        };

        assert_eq!(result.method, DiscoveryMethod::Mdns);
        assert_eq!(result.latency_ms, 150);
    }
}
