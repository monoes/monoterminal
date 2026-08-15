// Week 2 Implementation: Health Check & Upgrade (§2.4.3)
//
// Core functionality:
// - run_doctor_check: Execute `npx monomind@latest doctor --json`
// - upgrade_monomind: Execute `npx monomind@latest upgrade`
// - HealthScheduler: Daily background health check scheduler
//
// Design principle: Fail loud, not silent (prevent monoes/monomind#135, #136)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;
use tokio::time::{interval, Duration};

/// Health check status from monomind doctor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthStatus {
    /// Whether monomind CLI is installed and accessible
    pub installed: bool,
    /// Installed CLI version (if available)
    pub version: Option<String>,
    /// Whether control server is reachable
    pub control_server_reachable: bool,
    /// Whether broker is registered
    pub broker_registered: bool,
    /// Timestamp of last health check
    pub last_check: SystemTime,
    /// List of health issues found
    pub issues: Vec<HealthIssue>,
}

impl HealthStatus {
    /// Create a healthy status (all checks pass)
    pub fn healthy(version: String) -> Self {
        Self {
            installed: true,
            version: Some(version),
            control_server_reachable: true,
            broker_registered: true,
            last_check: SystemTime::now(),
            issues: vec![],
        }
    }

    /// Create a status indicating monomind is not installed
    pub fn not_installed() -> Self {
        Self {
            installed: false,
            version: None,
            control_server_reachable: false,
            broker_registered: false,
            last_check: SystemTime::now(),
            issues: vec![HealthIssue {
                severity: Severity::Error,
                message: "monomind is not installed".to_string(),
                resolution: Some("Run: npx monomind@latest init".to_string()),
            }],
        }
    }

    /// Check if overall health is good (no errors)
    pub fn is_healthy(&self) -> bool {
        self.installed
            && self.control_server_reachable
            && self.broker_registered
            && !self.issues.iter().any(|i| i.severity == Severity::Error)
    }
}

/// Individual health issue
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthIssue {
    pub severity: Severity,
    pub message: String,
    pub resolution: Option<String>,
}

/// Issue severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Result of upgrade operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpgradeResult {
    /// Whether upgrade succeeded
    pub success: bool,
    /// Previous version (before upgrade)
    pub old_version: Option<String>,
    /// New version (after upgrade)
    pub new_version: Option<String>,
    /// Full command output
    pub output: String,
}

/// Run monomind doctor health check
///
/// Executes `npx monomind@latest doctor --json` in the given project directory
/// and parses the JSON output to determine health status.
///
/// # Design: Fail Loud, Not Silent
///
/// This function is designed to prevent the failure modes experienced in
/// monoes/monomind#135 (dropped auth credentials) and #136 (dead foreign-server
/// pairing). All failures are surfaced explicitly in the HealthStatus.
///
/// # Arguments
///
/// * `project_dir` - Project root directory containing .monomind/
///
/// # Returns
///
/// * `Ok(HealthStatus)` - Health check results (may contain errors)
/// * `Err(_)` - Command execution failed (permission denied, etc.)
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::run_doctor_check;
/// use std::path::Path;
///
/// let health = run_doctor_check(Path::new("/project")).await?;
/// if !health.is_healthy() {
///     for issue in &health.issues {
///         println!("{:?}: {}", issue.severity, issue.message);
///     }
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub async fn run_doctor_check(project_dir: &Path) -> Result<HealthStatus> {
    tracing::debug!(
        path = %project_dir.display(),
        "Running monomind doctor health check"
    );

    // Execute npx monomind@latest doctor --json
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("doctor")
                .arg("--json")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Failed to spawn health check task")?
    .context("Failed to execute monomind doctor")?;

    // Check if command succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(
            exit_code = output.status.code(),
            stderr = %stderr,
            "monomind doctor command failed"
        );

        // If npx itself failed, monomind is likely not installed
        if stderr.contains("command not found") || stderr.contains("not recognized") {
            return Ok(HealthStatus::not_installed());
        }

        // Other failure - return error status
        return Ok(HealthStatus {
            installed: false,
            version: None,
            control_server_reachable: false,
            broker_registered: false,
            last_check: SystemTime::now(),
            issues: vec![HealthIssue {
                severity: Severity::Error,
                message: format!("monomind doctor failed: {}", stderr),
                resolution: Some("Run: npx monomind@latest doctor --fix".to_string()),
            }],
        });
    }

    // Parse JSON output
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("Failed to parse monomind doctor JSON output")?;

    parse_doctor_output(json)
}

/// Parse monomind doctor JSON output into HealthStatus
///
/// Expected JSON schema (from monomind CLI):
/// ```json
/// {
///   "version": "1.2.3",
///   "controlServer": {
///     "reachable": true,
///     "port": 3000
///   },
///   "broker": {
///     "registered": true,
///     "path": "/path/to/broker.json"
///   },
///   "issues": [
///     {
///       "severity": "warning",
///       "message": "Old CLI version detected",
///       "resolution": "Run: npx monomind@latest upgrade"
///     }
///   ]
/// }
/// ```
fn parse_doctor_output(json: serde_json::Value) -> Result<HealthStatus> {
    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);

    let control_server_reachable = json
        .get("controlServer")
        .and_then(|v| v.get("reachable"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let broker_registered = json
        .get("broker")
        .and_then(|v| v.get("registered"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let issues: Vec<HealthIssue> = json
        .get("issues")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|issue| {
                    let severity_str = issue.get("severity")?.as_str()?;
                    let severity = match severity_str {
                        "error" => Severity::Error,
                        "warning" => Severity::Warning,
                        "info" => Severity::Info,
                        _ => return None,
                    };

                    let message = issue.get("message")?.as_str()?.to_string();
                    let resolution = issue
                        .get("resolution")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    Some(HealthIssue {
                        severity,
                        message,
                        resolution,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    tracing::info!(
        version = ?version,
        control_server = control_server_reachable,
        broker = broker_registered,
        issues = issues.len(),
        "Health check complete"
    );

    Ok(HealthStatus {
        installed: version.is_some(),
        version,
        control_server_reachable,
        broker_registered,
        last_check: SystemTime::now(),
        issues,
    })
}

/// Trigger one-click upgrade to latest monomind version
///
/// Executes `npx monomind@latest upgrade` in the given project directory.
/// This is a potentially destructive operation and should be gated by user
/// confirmation in the UI.
///
/// # Arguments
///
/// * `project_dir` - Project root directory containing .monomind/
///
/// # Returns
///
/// * `Ok(UpgradeResult)` - Upgrade results (success or failure)
/// * `Err(_)` - Command execution failed
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::upgrade_monomind;
/// use std::path::Path;
///
/// let result = upgrade_monomind(Path::new("/project")).await?;
/// if result.success {
///     println!("Upgraded {} -> {}",
///         result.old_version.unwrap_or_default(),
///         result.new_version.unwrap_or_default()
///     );
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub async fn upgrade_monomind(project_dir: &Path) -> Result<UpgradeResult> {
    tracing::info!(
        path = %project_dir.display(),
        "Triggering monomind upgrade"
    );

    // Get current version before upgrade
    let old_version = get_current_version(project_dir).await.ok();

    // Execute npx monomind@latest upgrade
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("upgrade")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Failed to spawn upgrade task")?
    .context("Failed to execute monomind upgrade")?;

    let success = output.status.success();
    let output_text = String::from_utf8_lossy(&output.stdout).to_string();

    // Get new version after upgrade
    let new_version = if success {
        get_current_version(project_dir).await.ok()
    } else {
        None
    };

    tracing::info!(
        success = success,
        old_version = ?old_version,
        new_version = ?new_version,
        "Upgrade complete"
    );

    Ok(UpgradeResult {
        success,
        old_version,
        new_version,
        output: output_text,
    })
}

/// Get current monomind CLI version
async fn get_current_version(project_dir: &Path) -> Result<String> {
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("--version")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Failed to spawn version check task")?
    .context("Failed to get monomind version")?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        Ok(version)
    } else {
        anyhow::bail!("Failed to get version")
    }
}

/// Daily health check scheduler
///
/// Runs health checks on a configurable interval (default 24 hours).
/// Designed to run in the background and emit health status via callback.
pub struct HealthScheduler {
    interval: Duration,
}

impl HealthScheduler {
    /// Create new scheduler with default 24-hour interval
    pub fn new() -> Self {
        Self::with_interval(Duration::from_secs(86400))
    }

    /// Create new scheduler with custom interval
    pub fn with_interval(interval: Duration) -> Self {
        Self { interval }
    }

    /// Start the scheduler loop
    ///
    /// Calls the provided callback with health status on each check.
    /// Runs indefinitely until the task is cancelled.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - Project root to check
    /// * `callback` - Async callback to receive health status
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoterminal_monomind_bridge::HealthScheduler;
    /// use std::path::Path;
    ///
    /// let scheduler = HealthScheduler::new();
    /// scheduler.start(
    ///     Path::new("/project"),
    ///     |health| async move {
    ///         println!("Health: {:?}", health);
    ///     }
    /// ).await?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub async fn start<F, Fut>(self, project_dir: &Path, callback: F) -> Result<()>
    where
        F: Fn(HealthStatus) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut ticker = interval(self.interval);
        let project_dir = project_dir.to_path_buf();

        tracing::info!(
            interval_secs = self.interval.as_secs(),
            "Health check scheduler started"
        );

        loop {
            ticker.tick().await;

            tracing::debug!("Running scheduled health check");

            match run_doctor_check(&project_dir).await {
                Ok(health) => {
                    callback(health).await;
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "Scheduled health check failed"
                    );

                    // Call callback with error status
                    callback(HealthStatus {
                        installed: false,
                        version: None,
                        control_server_reachable: false,
                        broker_registered: false,
                        last_check: SystemTime::now(),
                        issues: vec![HealthIssue {
                            severity: Severity::Error,
                            message: format!("Health check error: {}", e),
                            resolution: None,
                        }],
                    })
                    .await;
                }
            }
        }
    }
}

impl Default for HealthScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_health_status_healthy() {
        let status = HealthStatus::healthy("1.2.3".to_string());

        assert!(status.installed);
        assert_eq!(status.version, Some("1.2.3".to_string()));
        assert!(status.control_server_reachable);
        assert!(status.broker_registered);
        assert!(status.is_healthy());
        assert!(status.issues.is_empty());
    }

    #[test]
    fn test_health_status_not_installed() {
        let status = HealthStatus::not_installed();

        assert!(!status.installed);
        assert_eq!(status.version, None);
        assert!(!status.control_server_reachable);
        assert!(!status.broker_registered);
        assert!(!status.is_healthy());
        assert_eq!(status.issues.len(), 1);
        assert_eq!(status.issues[0].severity, Severity::Error);
    }

    #[test]
    fn test_health_status_is_healthy_with_warnings() {
        let mut status = HealthStatus::healthy("1.2.3".to_string());
        status.issues.push(HealthIssue {
            severity: Severity::Warning,
            message: "Old version".to_string(),
            resolution: Some("Upgrade".to_string()),
        });

        // Should still be healthy (warnings don't block)
        assert!(status.is_healthy());
    }

    #[test]
    fn test_health_status_is_unhealthy_with_errors() {
        let mut status = HealthStatus::healthy("1.2.3".to_string());
        status.issues.push(HealthIssue {
            severity: Severity::Error,
            message: "Control server down".to_string(),
            resolution: None,
        });

        assert!(!status.is_healthy());
    }

    #[test]
    fn test_parse_doctor_output_healthy() {
        let json = json!({
            "version": "1.2.3",
            "controlServer": {
                "reachable": true,
                "port": 3000
            },
            "broker": {
                "registered": true,
                "path": "/path/to/broker.json"
            },
            "issues": []
        });

        let status = parse_doctor_output(json).unwrap();

        assert!(status.installed);
        assert_eq!(status.version, Some("1.2.3".to_string()));
        assert!(status.control_server_reachable);
        assert!(status.broker_registered);
        assert!(status.issues.is_empty());
    }

    #[test]
    fn test_parse_doctor_output_with_issues() {
        let json = json!({
            "version": "1.0.0",
            "controlServer": {
                "reachable": false
            },
            "broker": {
                "registered": true
            },
            "issues": [
                {
                    "severity": "error",
                    "message": "Control server unreachable",
                    "resolution": "Check if server is running"
                },
                {
                    "severity": "warning",
                    "message": "Old version detected",
                    "resolution": "Run: npx monomind@latest upgrade"
                }
            ]
        });

        let status = parse_doctor_output(json).unwrap();

        assert!(status.installed);
        assert!(!status.control_server_reachable);
        assert!(status.broker_registered);
        assert_eq!(status.issues.len(), 2);
        assert_eq!(status.issues[0].severity, Severity::Error);
        assert_eq!(status.issues[1].severity, Severity::Warning);
    }

    #[test]
    fn test_parse_doctor_output_missing_fields() {
        let json = json!({
            "version": "1.2.3"
        });

        let status = parse_doctor_output(json).unwrap();

        assert!(status.installed);
        assert_eq!(status.version, Some("1.2.3".to_string()));
        // Missing fields should default to false
        assert!(!status.control_server_reachable);
        assert!(!status.broker_registered);
    }

    #[test]
    fn test_upgrade_result_success() {
        let result = UpgradeResult {
            success: true,
            old_version: Some("1.0.0".to_string()),
            new_version: Some("1.2.3".to_string()),
            output: "Upgrade successful".to_string(),
        };

        assert!(result.success);
        assert_eq!(result.old_version, Some("1.0.0".to_string()));
        assert_eq!(result.new_version, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_health_scheduler_default_interval() {
        let scheduler = HealthScheduler::new();
        assert_eq!(scheduler.interval, Duration::from_secs(86400));
    }

    #[test]
    fn test_health_scheduler_custom_interval() {
        let scheduler = HealthScheduler::with_interval(Duration::from_secs(3600));
        assert_eq!(scheduler.interval, Duration::from_secs(3600));
    }

    #[test]
    fn test_severity_serialization() {
        let info = serde_json::to_string(&Severity::Info).unwrap();
        let warning = serde_json::to_string(&Severity::Warning).unwrap();
        let error = serde_json::to_string(&Severity::Error).unwrap();

        assert_eq!(info, r#""info""#);
        assert_eq!(warning, r#""warning""#);
        assert_eq!(error, r#""error""#);
    }

    #[test]
    fn test_health_issue_with_resolution() {
        let issue = HealthIssue {
            severity: Severity::Warning,
            message: "Test message".to_string(),
            resolution: Some("Test resolution".to_string()),
        };

        assert_eq!(issue.severity, Severity::Warning);
        assert!(issue.resolution.is_some());
    }

    #[test]
    fn test_health_issue_without_resolution() {
        let issue = HealthIssue {
            severity: Severity::Info,
            message: "Test message".to_string(),
            resolution: None,
        };

        assert_eq!(issue.severity, Severity::Info);
        assert!(issue.resolution.is_none());
    }
}
