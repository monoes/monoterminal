// Health check and metrics endpoints
// ADR-011 §8: Health check endpoints (/health, TURN probe, directory probe)

#![allow(dead_code)]  // Health check features not all integrated yet, cleanup tracked in task-63

use crate::webrtc::config::StunServerConfig;
use crate::webrtc::ice::probe_stun_server;
use crate::webrtc::WebRtcMetrics;
use prometheus::{Encoder, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Health status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status ("healthy" | "degraded" | "unhealthy")
    pub status: String,

    /// Component-specific health checks
    pub checks: HealthChecks,

    /// Timestamp (ISO 8601)
    pub timestamp: String,
}

/// Individual health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecks {
    /// WebSocket server status
    pub websocket: ComponentHealth,

    /// STUN server reachability
    pub stun: ComponentHealth,

    /// TURN server reachability (Week 3-4, optional)
    pub turn: Option<ComponentHealth>,

    /// Directory service reachability (Week 5-6, optional)
    pub directory: Option<ComponentHealth>,
}

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Status ("healthy" | "unhealthy" | "unknown")
    pub status: String,

    /// Optional error message
    pub message: Option<String>,

    /// Last check timestamp (ISO 8601)
    pub last_checked: String,
}

impl ComponentHealth {
    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            message: None,
            last_checked: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn unhealthy(message: String) -> Self {
        Self {
            status: "unhealthy".to_string(),
            message: Some(message),
            last_checked: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn unknown() -> Self {
        Self {
            status: "unknown".to_string(),
            message: None,
            last_checked: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Health checker
pub struct HealthChecker {
    /// WebRTC metrics
    metrics: Arc<WebRtcMetrics>,

    /// STUN server config
    stun_config: Arc<StunServerConfig>,
}

impl HealthChecker {
    pub fn new(metrics: Arc<WebRtcMetrics>, stun_config: Arc<StunServerConfig>) -> Self {
        Self {
            metrics,
            stun_config,
        }
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> HealthResponse {
        debug!("Performing health check");

        // Check STUN server
        let stun_health = self.check_stun_server().await;

        // WebSocket is always healthy if we can respond to this request
        let websocket_health = ComponentHealth::healthy();

        // TURN and directory deferred to Week 3-4, 5-6
        let turn_health = None;
        let directory_health = None;

        let checks = HealthChecks {
            websocket: websocket_health.clone(),
            stun: stun_health.clone(),
            turn: turn_health,
            directory: directory_health,
        };

        // Determine overall status
        let status = if stun_health.status == "unhealthy" {
            "degraded".to_string() // WebSocket works but P2P may fail
        } else {
            "healthy".to_string()
        };

        HealthResponse {
            status,
            checks,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Check STUN server reachability
    async fn check_stun_server(&self) -> ComponentHealth {
        if self.stun_config.urls.is_empty() {
            return ComponentHealth::unhealthy("No STUN servers configured".to_string());
        }

        // Probe first STUN server
        let stun_url = &self.stun_config.urls[0];
        let timeout = Duration::from_secs(5);

        match probe_stun_server(stun_url, timeout).await {
            Ok(_) => {
                // Update metrics
                self.metrics.stun_health_status.set(1.0); // healthy
                ComponentHealth::healthy()
            }
            Err(e) => {
                warn!("STUN server probe failed: {}", e);
                self.metrics.stun_health_status.set(2.0); // unhealthy
                ComponentHealth::unhealthy(format!("STUN server unreachable: {}", e))
            }
        }
    }

    /// Get Prometheus metrics as text
    pub fn get_metrics(&self, registry: &Registry) -> Result<String, std::fmt::Error> {
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();

        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|_| std::fmt::Error)?;

        String::from_utf8(buffer).map_err(|_| std::fmt::Error)
    }
}

/// Health endpoint handler (returns JSON)
pub async fn handle_health_check(checker: Arc<HealthChecker>) -> Result<String, String> {
    let health = checker.check_health().await;

    serde_json::to_string_pretty(&health).map_err(|e| format!("Serialization error: {}", e))
}

/// Metrics endpoint handler (returns Prometheus text format)
pub async fn handle_metrics(
    checker: Arc<HealthChecker>,
    registry: Arc<Registry>,
) -> Result<String, String> {
    checker
        .get_metrics(&registry)
        .map_err(|e| format!("Metrics encoding error: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy();
        assert_eq!(healthy.status, "healthy");
        assert!(healthy.message.is_none());

        let unhealthy = ComponentHealth::unhealthy("Test error".to_string());
        assert_eq!(unhealthy.status, "unhealthy");
        assert_eq!(unhealthy.message, Some("Test error".to_string()));

        let unknown = ComponentHealth::unknown();
        assert_eq!(unknown.status, "unknown");
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        use prometheus::Registry;

        let registry = Registry::new();
        let metrics = Arc::new(WebRtcMetrics::new(&registry).unwrap());
        let stun_config = Arc::new(StunServerConfig::default());

        let checker = HealthChecker::new(metrics, stun_config);

        // Just verify creation works
        let health = checker.check_health().await;
        assert!(!health.status.is_empty());
    }

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            checks: HealthChecks {
                websocket: ComponentHealth::healthy(),
                stun: ComponentHealth::healthy(),
                turn: None,
                directory: None,
            },
            timestamp: "2026-08-19T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string_pretty(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("websocket"));
    }
}
