// Protocol Response Conversions for Monomind Integration
// Converts internal monomind-bridge types to protocol buffer response types
//
// This module owns the conversion from internal types (HealthStatus, DashboardData)
// to the wire protocol types (HealthCheckResponse, DashboardResponse).

use crate::{
    AgentInfo as BridgeAgentInfo, DashboardData, HealthIssue, HealthStatus, MemoryStats,
    Severity,
};
#[cfg(test)]
use crate::OrgStatus;
use std::time::SystemTime;

// Re-export protocol types for convenience
// NOTE: This requires the protocol crate to be rebuilt after the protobuf changes
// Run: cargo build -p protocol

/// Convert HealthStatus to HealthCheckResponse
///
/// Maps internal health check results to the wire protocol format.
/// All fields are mapped 1:1, with SystemTime converted to Unix timestamp.
///
/// # Example
///
/// ```no_run
/// use monoterminal_monomind_bridge::{run_doctor_check, to_health_check_response};
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// let health = run_doctor_check(Path::new("/project")).await?;
/// let response = to_health_check_response(health);
/// # Ok(())
/// # }
/// ```
pub fn to_health_check_response(health: HealthStatus) -> HealthCheckResponseProto {
    let last_check_timestamp = health
        .last_check
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    HealthCheckResponseProto {
        installed: health.installed,
        version: health.version.unwrap_or_default(),
        control_server_reachable: health.control_server_reachable,
        broker_registered: health.broker_registered,
        last_check_timestamp,
        issues: health.issues.into_iter().map(to_health_issue).collect(),
    }
}

/// Convert HealthIssue to protocol HealthIssue
fn to_health_issue(issue: HealthIssue) -> HealthIssueProto {
    HealthIssueProto {
        severity: match issue.severity {
            Severity::Info => 0,    // INFO
            Severity::Warning => 1, // WARNING
            Severity::Error => 2,   // ERROR
        },
        message: issue.message,
        resolution: issue.resolution.unwrap_or_default(),
    }
}

/// Convert DashboardData to DashboardResponse
///
/// Maps internal dashboard data to the wire protocol format.
/// All fields are converted to their protocol equivalents.
///
/// # Example
///
/// ```no_run
/// use monoterminal_monomind_bridge::{get_dashboard_data, to_dashboard_response};
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// let data = get_dashboard_data(Path::new("/project")).await?;
/// let response = to_dashboard_response(data);
/// # Ok(())
/// # }
/// ```
pub fn to_dashboard_response(data: DashboardData) -> DashboardResponseProto {
    let timestamp = data
        .timestamp
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    DashboardResponseProto {
        org_name: data.org_status.name.unwrap_or_else(|| "No org".to_string()),
        org_status: if data.org_status.running {
            "running".to_string()
        } else {
            "stopped".to_string()
        },
        agents: data.agents.into_iter().map(to_agent_info).collect(),
        tasks: vec![], // TODO: Task tracking not yet implemented in Phase 1
        kg_stats: Some(to_kg_stats(data.memory_stats)),
        timestamp,
    }
}

/// Convert AgentInfo to protocol AgentInfo
fn to_agent_info(agent: BridgeAgentInfo) -> AgentInfoProto {
    AgentInfoProto {
        id: agent.id.clone(),
        name: agent.id, // For now, ID serves as name
        role: agent.agent_type,
        status: agent.status,
        tasks_completed: agent.tasks_completed,
        uptime_secs: agent.uptime_secs,
    }
}

/// Convert MemoryStats to KnowledgeGraphStats
fn to_kg_stats(stats: MemoryStats) -> KnowledgeGraphStatsProto {
    KnowledgeGraphStatsProto {
        nodes: stats.kg_nodes,
        relationships: stats.kg_edges,
        total_entries: stats.total_entries,
        db_size_bytes: stats.db_size_bytes,
        last_updated: 0, // TODO: Track KG last updated timestamp
    }
}

// Placeholder protocol types until protocol crate is rebuilt
// These will be replaced by actual generated types from protocol crate

#[derive(Debug, Clone)]
pub struct HealthCheckResponseProto {
    pub installed: bool,
    pub version: String,
    pub control_server_reachable: bool,
    pub broker_registered: bool,
    pub last_check_timestamp: i64,
    pub issues: Vec<HealthIssueProto>,
}

#[derive(Debug, Clone)]
pub struct HealthIssueProto {
    pub severity: i32,
    pub message: String,
    pub resolution: String,
}

#[derive(Debug, Clone)]
pub struct DashboardResponseProto {
    pub org_name: String,
    pub org_status: String,
    pub agents: Vec<AgentInfoProto>,
    pub tasks: Vec<TaskInfoProto>,
    pub kg_stats: Option<KnowledgeGraphStatsProto>,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct AgentInfoProto {
    pub id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub tasks_completed: u32,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone)]
pub struct TaskInfoProto {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeGraphStatsProto {
    pub nodes: u64,
    pub relationships: u64,
    pub total_entries: u64,
    pub db_size_bytes: u64,
    pub last_updated: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_to_health_check_response() {
        let health = HealthStatus {
            installed: true,
            version: Some("1.2.3".to_string()),
            control_server_reachable: true,
            broker_registered: true,
            last_check: SystemTime::now(),
            issues: vec![HealthIssue {
                severity: Severity::Warning,
                message: "Test warning".to_string(),
                resolution: Some("Fix it".to_string()),
            }],
        };

        let response = to_health_check_response(health);

        assert!(response.installed);
        assert_eq!(response.version, "1.2.3");
        assert!(response.control_server_reachable);
        assert!(response.broker_registered);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].severity, 1); // WARNING
    }

    #[test]
    fn test_to_health_check_response_not_installed() {
        let health = HealthStatus::not_installed();
        let response = to_health_check_response(health);

        assert!(!response.installed);
        assert_eq!(response.version, "");
        assert!(!response.control_server_reachable);
        assert!(!response.broker_registered);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].severity, 2); // ERROR
    }

    #[test]
    fn test_to_dashboard_response() {
        let data = DashboardData {
            org_status: OrgStatus::running("test-org".to_string(), "run-123".to_string(), 3, 5),
            agents: vec![BridgeAgentInfo {
                id: "agent-1".to_string(),
                agent_type: "coder".to_string(),
                status: "running".to_string(),
                tasks_completed: 10,
                uptime_secs: 3600,
            }],
            runs: vec![],
            memory_stats: MemoryStats {
                total_entries: 1000,
                kg_nodes: 500,
                kg_edges: 1500,
                db_size_bytes: 1024000,
            },
            timestamp: SystemTime::now(),
        };

        let response = to_dashboard_response(data);

        assert_eq!(response.org_name, "test-org");
        assert_eq!(response.org_status, "running");
        assert_eq!(response.agents.len(), 1);
        assert_eq!(response.agents[0].role, "coder");
        assert!(response.kg_stats.is_some());

        let kg_stats = response.kg_stats.unwrap();
        assert_eq!(kg_stats.nodes, 500);
        assert_eq!(kg_stats.relationships, 1500);
    }

    #[test]
    fn test_to_dashboard_response_empty() {
        let data = DashboardData::empty();
        let response = to_dashboard_response(data);

        assert_eq!(response.org_name, "No org");
        assert_eq!(response.org_status, "stopped");
        assert_eq!(response.agents.len(), 0);
        assert!(response.kg_stats.is_some());
    }

    #[test]
    fn test_severity_mapping() {
        let info = to_health_issue(HealthIssue {
            severity: Severity::Info,
            message: "test".to_string(),
            resolution: None,
        });
        assert_eq!(info.severity, 0);

        let warning = to_health_issue(HealthIssue {
            severity: Severity::Warning,
            message: "test".to_string(),
            resolution: None,
        });
        assert_eq!(warning.severity, 1);

        let error = to_health_issue(HealthIssue {
            severity: Severity::Error,
            message: "test".to_string(),
            resolution: None,
        });
        assert_eq!(error.severity, 2);
    }
}
