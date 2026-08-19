// Discovery services integration tests
// ADR-011 §4: Hybrid Discovery

#[cfg(test)]
mod integration_tests {
    use crate::discovery::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_discovery_config_defaults() {
        let config = DiscoveryConfig::default();

        assert_eq!(config.service_type, "_monoterminal._tcp.local");
        assert_eq!(config.ttl_seconds, 3600); // 1 hour
        assert!(config.enable_mdns);
        assert!(!config.enable_directory); // Not deployed yet
    }

    #[tokio::test]
    async fn test_discovery_config_test_mode() {
        let config = DiscoveryConfig::test_config();

        assert_eq!(config.ttl_seconds, 300); // 5 minutes for testing
        assert_eq!(config.discovery_timeout, Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_mdns_service_info() {
        let info = ServiceInfo::new(
            "test-service".to_string(),
            "test.local".to_string(),
            vec!["192.168.1.100".parse().unwrap()],
            9443,
        )
        .with_property("version", "1.0")
        .with_property("peer_id", "ed25519:test");

        assert_eq!(info.websocket_url(), "wss://test.local:9443");
        assert_eq!(info.properties.len(), 2);
    }

    #[tokio::test]
    async fn test_directory_peer_endpoint() {
        let endpoint = PeerEndpoint {
            endpoint_type: "websocket".to_string(),
            url: "wss://example.com:9443".to_string(),
            verified: true,
        };

        let json = serde_json::to_string(&endpoint).unwrap();
        let parsed: PeerEndpoint = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.url, endpoint.url);
        assert!(parsed.verified);
    }

    #[tokio::test]
    async fn test_hybrid_discovery_mdns_only() {
        let config = DiscoveryConfig {
            enable_mdns: true,
            enable_directory: false,
            ..Default::default()
        };

        let discovery = HybridDiscovery::new(config);
        assert!(discovery.is_ok());
    }

    #[tokio::test]
    async fn test_hybrid_discovery_directory_only() {
        let config = DiscoveryConfig {
            enable_mdns: false,
            enable_directory: true,
            directory_url: Some("http://localhost:8080".to_string()),
            ..Default::default()
        };

        let discovery = HybridDiscovery::new(config);
        assert!(discovery.is_ok());
    }

    #[tokio::test]
    async fn test_discovery_method_enum() {
        assert_eq!(DiscoveryMethod::Mdns, DiscoveryMethod::Mdns);
        assert_ne!(DiscoveryMethod::Mdns, DiscoveryMethod::Directory);
    }

    #[test]
    fn test_error_types() {
        let err = DiscoveryError::ServiceNotFound("test".to_string());
        assert!(err.to_string().contains("test"));

        let err = DiscoveryError::DiscoveryTimeout(Duration::from_secs(5));
        assert!(err.to_string().contains("5s"));
    }
}
