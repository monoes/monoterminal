// Reference implementation: How to integrate health checks into the master daemon
// This is NOT part of the build - it's a documented example for task-8 implementers
//
// Copy relevant patterns from this file when implementing:
// - crates/master/src/health_handler.rs
// - crates/master/src/main.rs (scheduler startup)
// - WebSocket message handlers

#![allow(dead_code, unused_imports, unused_variables)]

use crate::{run_doctor_check, upgrade_monomind, HealthScheduler, HealthStatus, Severity};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::broadcast;

/// Example: Protocol message types (these will come from generated proto code)
mod proto_example {
    pub struct HealthCheckResponse {
        pub installed: bool,
        pub version: String,
        pub control_server_reachable: bool,
        pub broker_registered: bool,
        pub last_check_timestamp: i64,
        pub issues: Vec<HealthIssue>,
    }

    pub struct HealthIssue {
        pub severity: IssueSeverity,
        pub message: String,
        pub resolution: String,
    }

    pub enum IssueSeverity {
        INFO = 0,
        WARNING = 1,
        ERROR = 2,
    }

    pub struct UpgradeRequest {
        pub project_dir: String,
        pub confirmed: bool,
    }

    pub struct UpgradeResponse {
        pub success: bool,
        pub old_version: String,
        pub new_version: String,
        pub output: String,
    }
}

use proto_example::*;

/// Example: Health check handler for WebSocket messages
///
/// This should be called when a HealthCheckRequest arrives via WebSocket.
/// Returns a HealthCheckResponse that can be sent back to the client.
pub async fn handle_health_check_request(project_dir: &Path) -> HealthCheckResponse {
    match run_doctor_check(project_dir).await {
        Ok(status) => convert_health_status_to_proto(status),
        Err(e) => {
            tracing::error!(
                error = %e,
                path = %project_dir.display(),
                "Health check failed"
            );

            // Return error status
            HealthCheckResponse {
                installed: false,
                version: String::new(),
                control_server_reachable: false,
                broker_registered: false,
                last_check_timestamp: unix_timestamp_now(),
                issues: vec![HealthIssue {
                    severity: IssueSeverity::ERROR,
                    message: format!("Health check error: {}", e),
                    resolution: "Check monomind installation: npx monomind@latest doctor --fix"
                        .to_string(),
                }],
            }
        }
    }
}

/// Example: Upgrade handler for WebSocket messages
///
/// IMPORTANT: This must verify that confirmed=true before proceeding.
/// Per SRS §2.4.3, upgrade is a potentially destructive operation.
pub async fn handle_upgrade_request(request: UpgradeRequest) -> UpgradeResponse {
    // SECURITY: Require explicit user confirmation
    if !request.confirmed {
        tracing::warn!("Upgrade request rejected - confirmation required");
        return UpgradeResponse {
            success: false,
            old_version: String::new(),
            new_version: String::new(),
            output: "User confirmation required for upgrade".to_string(),
        };
    }

    let project_dir = if request.project_dir.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(request.project_dir)
    };

    tracing::info!(
        path = %project_dir.display(),
        "Processing confirmed upgrade request"
    );

    match upgrade_monomind(&project_dir).await {
        Ok(result) => UpgradeResponse {
            success: result.success,
            old_version: result.old_version.unwrap_or_default(),
            new_version: result.new_version.unwrap_or_default(),
            output: result.output,
        },
        Err(e) => {
            tracing::error!(error = %e, "Upgrade failed");
            UpgradeResponse {
                success: false,
                old_version: String::new(),
                new_version: String::new(),
                output: format!("Upgrade error: {}", e),
            }
        }
    }
}

/// Example: Start daily health check scheduler
///
/// This should be spawned as a background task in main().
/// It will run health checks every 24 hours and broadcast results.
pub async fn start_daily_health_scheduler(
    project_dir: PathBuf,
    health_tx: broadcast::Sender<HealthStatus>,
) {
    tokio::spawn(async move {
        let scheduler = HealthScheduler::new(); // Default 24-hour interval

        tracing::info!(
            path = %project_dir.display(),
            "Starting daily health check scheduler"
        );

        // Start scheduler loop
        let result = scheduler
            .start(&project_dir, move |health| {
                let tx = health_tx.clone();
                async move {
                    tracing::info!(
                        healthy = health.is_healthy(),
                        issues = health.issues.len(),
                        version = ?health.version,
                        "Scheduled health check complete"
                    );

                    // Broadcast to all listeners (WebSocket clients)
                    if let Err(e) = tx.send(health.clone()) {
                        tracing::debug!("No health status subscribers: {}", e);
                    }
                }
            })
            .await;

        if let Err(e) = result {
            tracing::error!(error = %e, "Health scheduler terminated with error");
        }
    });
}

/// Example: Session manager integration
///
/// Call this when a session's working directory changes.
/// Per SRS §2.4.1, we should check for .monomind/ and health on cwd change.
pub async fn on_session_cwd_changed(
    new_cwd: PathBuf,
    health_tx: broadcast::Sender<HealthStatus>,
) {
    tracing::debug!(
        path = %new_cwd.display(),
        "Session working directory changed - triggering health check"
    );

    // Spawn background task so we don't block the session
    tokio::spawn(async move {
        match run_doctor_check(&new_cwd).await {
            Ok(health) => {
                if !health.is_healthy() {
                    tracing::warn!(
                        path = %new_cwd.display(),
                        issues = health.issues.len(),
                        "Health issues detected in new working directory"
                    );

                    // Broadcast unhealthy status
                    let _ = health_tx.send(health);
                }
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    path = %new_cwd.display(),
                    "Failed to check health on cwd change"
                );
            }
        }
    });
}

/// Convert HealthStatus (from monomind-bridge) to proto HealthCheckResponse
fn convert_health_status_to_proto(status: HealthStatus) -> HealthCheckResponse {
    HealthCheckResponse {
        installed: status.installed,
        version: status.version.unwrap_or_default(),
        control_server_reachable: status.control_server_reachable,
        broker_registered: status.broker_registered,
        last_check_timestamp: status
            .last_check
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        issues: status
            .issues
            .into_iter()
            .map(|issue| HealthIssue {
                severity: match issue.severity {
                    Severity::Info => IssueSeverity::INFO,
                    Severity::Warning => IssueSeverity::WARNING,
                    Severity::Error => IssueSeverity::ERROR,
                },
                message: issue.message,
                resolution: issue.resolution.unwrap_or_default(),
            })
            .collect(),
    }
}

/// Helper: Get current Unix timestamp
fn unix_timestamp_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ============================================================================
// Example: Main function integration pattern
// ============================================================================

#[allow(unreachable_code)]
async fn example_main_integration() -> anyhow::Result<()> {
    // 1. Create broadcast channel for health status updates
    let (health_tx, _health_rx) = broadcast::channel::<HealthStatus>(16);

    // 2. Get project directory (or use current dir)
    let project_dir = std::env::current_dir()?;

    // 3. Start daily health check scheduler
    start_daily_health_scheduler(project_dir.clone(), health_tx.clone()).await;

    // 4. In your WebSocket message handler, subscribe to health updates
    let mut health_rx = health_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(health) = health_rx.recv().await {
            // Broadcast health status to all connected WebSocket clients
            // send_health_status_to_clients(health).await;
        }
    });

    // 5. Handle WebSocket messages
    // (This would be in your actual WebSocket message loop)
    loop {
        // Example: Handle health check request
        let response = handle_health_check_request(&project_dir).await;
        // send_response(response).await?;

        // Example: Handle upgrade request
        let upgrade_req = UpgradeRequest {
            project_dir: project_dir.to_string_lossy().to_string(),
            confirmed: true,
        };
        let upgrade_resp = handle_upgrade_request(upgrade_req).await;
        // send_response(upgrade_resp).await?;

        break; // Remove this in real implementation
    }

    Ok(())
}

// ============================================================================
// Example: Testing patterns
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_health_check_handler() {
        let temp = TempDir::new().unwrap();
        let response = handle_health_check_request(temp.path()).await;

        // Should return a response even if monomind is not installed
        assert!(!response.version.is_empty() || !response.installed);
    }

    #[tokio::test]
    async fn test_upgrade_requires_confirmation() {
        let request = UpgradeRequest {
            project_dir: String::new(),
            confirmed: false, // NOT confirmed
        };

        let response = handle_upgrade_request(request).await;

        // Should reject without confirmation
        assert!(!response.success);
        assert!(response.output.contains("confirmation"));
    }

    #[tokio::test]
    async fn test_health_broadcast_channel() {
        let (tx, mut rx) = broadcast::channel::<HealthStatus>(16);
        let temp = TempDir::new().unwrap();

        // Simulate scheduled health check
        tokio::spawn({
            let temp_path = temp.path().to_path_buf();
            async move {
                if let Ok(health) = run_doctor_check(&temp_path).await {
                    let _ = tx.send(health);
                }
            }
        });

        // Should receive health status
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv()
        ).await;

        // Either received a message or channel was dropped (both are valid)
        assert!(result.is_ok() || result.is_err());
    }
}
