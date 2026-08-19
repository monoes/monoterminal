// mDNS Service Discovery
// ADR-011 §4.1: Local Discovery (mDNS/Bonjour)
// Service type: _monoterminal._tcp.local

use crate::discovery::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Service information advertised via mDNS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Service name (e.g., "monoterminal-alice")
    pub name: String,

    /// Hostname (e.g., "alice-desktop.local")
    pub hostname: String,

    /// IP addresses (IPv4/IPv6)
    pub addresses: Vec<IpAddr>,

    /// Port (default: 9443)
    pub port: u16,

    /// TXT record properties
    pub properties: HashMap<String, String>,
}

impl ServiceInfo {
    /// Create new service info
    pub fn new(name: String, hostname: String, addresses: Vec<IpAddr>, port: u16) -> Self {
        Self {
            name,
            hostname,
            addresses,
            port,
            properties: HashMap::new(),
        }
    }

    /// Add property to TXT record
    pub fn with_property(mut self, key: &str, value: &str) -> Self {
        self.properties.insert(key.to_string(), value.to_string());
        self
    }

    /// Get WebSocket URL (wss://hostname:port)
    pub fn websocket_url(&self) -> String {
        format!("wss://{}:{}", self.hostname, self.port)
    }
}

/// mDNS service discovery client
pub struct MdnsDiscovery {
    service_type: String,
    service_name: String,
    port: u16,
}

impl MdnsDiscovery {
    /// Create new mDNS discovery client
    pub fn new(service_type: String, service_name: String, port: u16) -> Self {
        Self {
            service_type,
            service_name,
            port,
        }
    }

    /// Register service for discovery (master side)
    ///
    /// ADR-011 §4.1: Advertise service with TXT records:
    /// - version: "1.0"
    /// - peer_id: "ed25519:abcd1234..."
    /// - protocol: "ws+wss+webrtc"
    pub async fn register(
        &self,
        peer_id: String,
        properties: HashMap<String, String>,
    ) -> Result<()> {
        info!(
            "Registering mDNS service: {} (type: {})",
            self.service_name, self.service_type
        );

        // TODO: Implement actual mDNS registration using mdns-sd crate
        // This is deferred to task-46 for proper API research

        debug!("mDNS registration with peer_id: {}", peer_id);
        debug!("Additional properties: {:?}", properties);

        // Placeholder: Would use mdns_sd::ServiceDaemon here
        warn!("mDNS registration deferred to task-46 (Week 5-6)");

        Ok(())
    }

    /// Discover services via mDNS (client side)
    ///
    /// ADR-011 §4.1: Query for _monoterminal._tcp.local
    /// Returns list of discovered services
    pub async fn discover(&self, timeout: Duration) -> Result<Vec<ServiceInfo>> {
        info!(
            "Discovering mDNS services: {} (timeout: {:?})",
            self.service_type, timeout
        );

        // TODO: Implement actual mDNS discovery using mdns-sd crate
        // This is deferred to task-46 for proper API research

        debug!(
            "Starting mDNS browse for service type: {}",
            self.service_type
        );

        // Placeholder: Would use mdns_sd::ServiceDaemon::browse() here
        warn!("mDNS discovery deferred to task-46 (Week 5-6)");

        // Return empty list for now
        Ok(Vec::new())
    }

    /// Unregister service (cleanup on shutdown)
    pub async fn unregister(&self) -> Result<()> {
        info!("Unregistering mDNS service: {}", self.service_name);

        // TODO: Implement actual mDNS unregistration
        // Placeholder for Week 5-6 implementation

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_info_creation() {
        let info = ServiceInfo::new(
            "test-service".to_string(),
            "test.local".to_string(),
            vec!["192.168.1.100".parse().unwrap()],
            9443,
        )
        .with_property("version", "1.0")
        .with_property("peer_id", "ed25519:test123");

        assert_eq!(info.name, "test-service");
        assert_eq!(info.port, 9443);
        assert_eq!(info.properties.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_websocket_url() {
        let info = ServiceInfo::new(
            "test".to_string(),
            "test.local".to_string(),
            vec!["192.168.1.100".parse().unwrap()],
            9443,
        );

        assert_eq!(info.websocket_url(), "wss://test.local:9443");
    }

    #[tokio::test]
    async fn test_mdns_discovery_creation() {
        let mdns = MdnsDiscovery::new(
            "_monoterminal._tcp.local".to_string(),
            "test".to_string(),
            9443,
        );

        assert_eq!(mdns.service_type, "_monoterminal._tcp.local");
    }
}
