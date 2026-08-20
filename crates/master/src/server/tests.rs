//! Server Module Unit Tests
//!
//! Comprehensive test coverage for WebSocket server components
//! Target: ≥70% coverage per SRS §6.1
//!

#![allow(clippy::field_reassign_with_default)]
//! Coverage areas:
//! - Server configuration and defaults
//! - TLS configuration
//! - Connection state management
//! - Error handling
//! - Rate limiting integration

#[cfg(test)]
mod server_tests {
    use super::super::*;
    use std::net::{IpAddr, Ipv4Addr};

    // ===== Configuration Tests =====

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();

        // Phase 1: local only - 127.0.0.1:5000
        assert_eq!(
            config.bind_addr.ip(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
        );
        assert_eq!(config.bind_addr.port(), 5000);

        // SRS §2.3.4: 1000 concurrent connections max
        assert_eq!(config.max_connections, 1000);

        // SRS §3.2.4: 100 connections/minute rate limit
        assert_eq!(config.rate_limit_per_minute, 100);

        // Production mode by default
        assert!(!config.dev_mode);
    }

    #[test]
    fn test_server_config_custom_bind_addr() {
        let mut config = ServerConfig::default();
        config.bind_addr = "127.0.0.1:8080".parse().unwrap();

        assert_eq!(config.bind_addr.port(), 8080);
    }

    #[test]
    fn test_server_config_custom_limits() {
        let mut config = ServerConfig::default();
        config.max_connections = 500;
        config.rate_limit_per_minute = 50;

        assert_eq!(config.max_connections, 500);
        assert_eq!(config.rate_limit_per_minute, 50);
    }

    #[test]
    fn test_server_config_dev_mode_flag() {
        let mut config = ServerConfig::default();
        config.dev_mode = true;

        assert!(
            config.dev_mode,
            "Dev mode should be enabled for E2E testing"
        );
    }

    // ===== TLS Configuration Tests =====

    #[test]
    fn test_tls_config_default() {
        let tls_config = TlsConfig::default();

        // Should have default cert/key paths per tls.rs Default impl
        assert!(tls_config
            .cert_path
            .to_str()
            .unwrap()
            .contains("server.crt"));
        assert!(tls_config.key_path.to_str().unwrap().contains("server.key"));
    }

    #[test]
    fn test_tls_config_custom_paths() {
        use std::path::PathBuf;

        let mut tls_config = TlsConfig::default();
        tls_config.cert_path = PathBuf::from("custom/cert.pem");
        tls_config.key_path = PathBuf::from("custom/key.pem");

        assert_eq!(tls_config.cert_path.to_str().unwrap(), "custom/cert.pem");
        assert_eq!(tls_config.key_path.to_str().unwrap(), "custom/key.pem");
    }

    // ===== Server Error Tests =====

    #[test]
    fn test_server_error_display() {
        use super::super::error::ServerError;

        let err = ServerError::RateLimitExceeded;
        assert!(err.to_string().contains("Rate limit exceeded"));

        let err = ServerError::TlsHandshake("Invalid certificate".to_string());
        assert!(err.to_string().contains("TLS handshake failed"));

        let err = ServerError::WebSocketUpgrade("Upgrade failed".to_string());
        assert!(err.to_string().contains("WebSocket upgrade failed"));
    }

    #[test]
    fn test_server_error_is_send_sync() {
        use super::super::error::ServerError;

        // Verify ServerError can be sent across threads
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ServerError>();
        assert_sync::<ServerError>();
    }

    // ===== Connection Limit Tests =====

    #[test]
    fn test_max_connections_boundary() {
        let config = ServerConfig::default();

        // Boundary test: max_connections should be within SRS limits
        assert!(config.max_connections > 0);
        assert!(config.max_connections <= 1000);
    }

    #[test]
    fn test_rate_limit_boundary() {
        let config = ServerConfig::default();

        // Boundary test: rate limit should be reasonable
        assert!(config.rate_limit_per_minute > 0);
        assert!(config.rate_limit_per_minute <= 1000);
    }
}

// ===== Connection Tests =====
// Note: Connection tests already exist in connection.rs
// These tests verify the Connection type's integration with server
