// Discovery Services Module
// ADR-011 §4: Hybrid Discovery (mDNS + Directory Service)
// SRS §2.3.3: Peer Discovery

pub mod directory;
pub mod error;
pub mod hybrid;
pub mod mdns;

#[cfg(test)]
mod tests;

pub use directory::{DirectoryClient, PeerEndpoint, RegistrationInfo};
pub use error::{DiscoveryError, Result};
pub use hybrid::{DiscoveryMethod, DiscoveryResult, HybridDiscovery};
pub use mdns::{MdnsDiscovery, ServiceInfo};

use std::time::Duration;

/// Discovery configuration
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// mDNS service type (ADR-011: _monoterminal._tcp.local)
    pub service_type: String,

    /// mDNS service name (e.g., "monoterminal-alice")
    pub service_name: String,

    /// Directory service base URL (e.g., "https://directory.monoterminal.io")
    pub directory_url: Option<String>,

    /// Registration TTL (ADR-011: 1 hour default)
    pub ttl_seconds: u64,

    /// Discovery timeout (both mDNS and directory race)
    pub discovery_timeout: Duration,

    /// Enable mDNS (LAN discovery)
    pub enable_mdns: bool,

    /// Enable directory service (internet discovery)
    pub enable_directory: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_type: "_monoterminal._tcp.local".to_string(),
            service_name: "monoterminal".to_string(),
            directory_url: None, // Week 5-6: Directory server deployment
            ttl_seconds: 3600,   // 1 hour per ADR-011
            discovery_timeout: Duration::from_secs(5),
            enable_mdns: true,
            enable_directory: false, // Enable when directory deployed
        }
    }
}

impl DiscoveryConfig {
    /// Create configuration for testing (local only)
    pub fn test_config() -> Self {
        Self {
            service_type: "_monoterminal._tcp.local".to_string(),
            service_name: "test-monoterminal".to_string(),
            directory_url: Some("http://localhost:8080".to_string()),
            ttl_seconds: 300, // 5 minutes for testing
            discovery_timeout: Duration::from_secs(2),
            enable_mdns: true,
            enable_directory: false, // Local testing only
        }
    }
}
