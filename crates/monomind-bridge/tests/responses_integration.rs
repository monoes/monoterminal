// Integration tests for protocol response conversions
//
// Tests the conversion from internal types to wire protocol types:
// - HealthStatus -> HealthCheckResponse
// - DashboardData -> DashboardResponse
// - Timestamp conversions
// - Edge case handling

use monoterminal_monomind_bridge::{
    to_dashboard_response, to_health_check_response, AgentInfo, DashboardData, HealthIssue,
    HealthStatus, MemoryStats, OrgStatus, Severity,
};
use std::time::SystemTime;

#[test]
fn test_to_health_check_response_healthy() {
    let health = HealthStatus::healthy("1.2.3".to_string());
    let response = to_health_check_response(health);

    assert!(response.installed);
    assert_eq!(response.version, "1.2.3");
    assert!(response.control_server_reachable);
    assert!(response.broker_registered);
    assert!(response.last_check_timestamp > 0);
    assert_eq!(response.issues.len(), 0);
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
    assert!(response.issues[0].message.contains("not installed"));
}

#[test]
fn test_to_health_check_response_with_issues() {
    let mut health = HealthStatus::healthy("1.2.3".to_string());

    health.issues.push(HealthIssue {
        severity: Severity::Info,
        message: "Info message".to_string(),
        resolution: None,
    });

    health.issues.push(HealthIssue {
        severity: Severity::Warning,
        message: "Warning message".to_string(),
        resolution: Some("Fix warning".to_string()),
    });

    health.issues.push(HealthIssue {
        severity: Severity::Error,
        message: "Error message".to_string(),
        resolution: Some("Fix error".to_string()),
    });

    let response = to_health_check_response(health);

    assert_eq!(response.issues.len(), 3);
    assert_eq!(response.issues[0].severity, 0); // INFO
    assert_eq!(response.issues[1].severity, 1); // WARNING
    assert_eq!(response.issues[2].severity, 2); // ERROR
}

#[test]
fn test_severity_mapping() {
    let info_issue = HealthIssue {
        severity: Severity::Info,
        message: "Info".to_string(),
        resolution: None,
    };

    let warning_issue = HealthIssue {
        severity: Severity::Warning,
        message: "Warning".to_string(),
        resolution: None,
    };

    let error_issue = HealthIssue {
        severity: Severity::Error,
        message: "Error".to_string(),
        resolution: None,
    };

    let health = HealthStatus {
        installed: true,
        version: Some("1.0.0".to_string()),
        control_server_reachable: true,
        broker_registered: true,
        last_check: SystemTime::now(),
        issues: vec![info_issue, warning_issue, error_issue],
    };

    let response = to_health_check_response(health);

    assert_eq!(response.issues[0].severity, 0); // INFO = 0
    assert_eq!(response.issues[1].severity, 1); // WARNING = 1
    assert_eq!(response.issues[2].severity, 2); // ERROR = 2
}

#[test]
fn test_health_issue_resolution_handling() {
    let with_resolution = HealthIssue {
        severity: Severity::Warning,
        message: "Issue with fix".to_string(),
        resolution: Some("Run this command".to_string()),
    };

    let without_resolution = HealthIssue {
        severity: Severity::Info,
        message: "Issue without fix".to_string(),
        resolution: None,
    };

    let health = HealthStatus {
        installed: true,
        version: Some("1.0.0".to_string()),
        control_server_reachable: true,
        broker_registered: true,
        last_check: SystemTime::now(),
        issues: vec![with_resolution, without_resolution],
    };

    let response = to_health_check_response(health);

    // With resolution
    assert_eq!(response.issues[0].resolution, "Run this command");

    // Without resolution (should be empty string)
    assert_eq!(response.issues[1].resolution, "");
}

#[test]
fn test_to_dashboard_response_empty() {
    let data = DashboardData::empty();
    let response = to_dashboard_response(data);

    assert_eq!(response.org_name, "No org");
    assert_eq!(response.org_status, "stopped");
    assert_eq!(response.agents.len(), 0);
    assert_eq!(response.tasks.len(), 0); // Tasks not yet implemented
    assert!(response.kg_stats.is_some());
    assert!(response.timestamp > 0);
}

#[test]
fn test_to_dashboard_response_running_org() {
    let data = DashboardData {
        org_status: OrgStatus::running("test-org".to_string(), "run-123".to_string(), 3, 5),
        agents: vec![
            AgentInfo {
                id: "agent-1".to_string(),
                agent_type: "coder".to_string(),
                status: "running".to_string(),
                tasks_completed: 10,
                uptime_secs: 3600,
            },
            AgentInfo {
                id: "agent-2".to_string(),
                agent_type: "reviewer".to_string(),
                status: "idle".to_string(),
                tasks_completed: 5,
                uptime_secs: 1800,
            },
        ],
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
    assert_eq!(response.agents.len(), 2);

    // Verify agent conversion
    assert_eq!(response.agents[0].id, "agent-1");
    assert_eq!(response.agents[0].role, "coder");
    assert_eq!(response.agents[0].status, "running");
    assert_eq!(response.agents[0].tasks_completed, 10);
    assert_eq!(response.agents[0].uptime_secs, 3600);

    // Verify KG stats
    let kg_stats = response.kg_stats.unwrap();
    assert_eq!(kg_stats.nodes, 500);
    assert_eq!(kg_stats.relationships, 1500);
    assert_eq!(kg_stats.total_entries, 1000);
    assert_eq!(kg_stats.db_size_bytes, 1024000);
}

#[test]
fn test_to_dashboard_response_stopped_org() {
    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![],
        memory_stats: MemoryStats::empty(),
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    assert_eq!(response.org_name, "No org");
    assert_eq!(response.org_status, "stopped");
}

#[test]
fn test_agent_info_conversion() {
    let agent = AgentInfo {
        id: "agent-123".to_string(),
        agent_type: "test-agent".to_string(),
        status: "active".to_string(),
        tasks_completed: 42,
        uptime_secs: 7200,
    };

    let data = DashboardData {
        org_status: OrgStatus::running("org".to_string(), "run".to_string(), 1, 0),
        agents: vec![agent],
        runs: vec![],
        memory_stats: MemoryStats::empty(),
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    assert_eq!(response.agents.len(), 1);
    assert_eq!(response.agents[0].id, "agent-123");
    assert_eq!(response.agents[0].name, "agent-123"); // ID serves as name
    assert_eq!(response.agents[0].role, "test-agent");
    assert_eq!(response.agents[0].status, "active");
    assert_eq!(response.agents[0].tasks_completed, 42);
    assert_eq!(response.agents[0].uptime_secs, 7200);
}

#[test]
fn test_memory_stats_conversion() {
    let stats = MemoryStats {
        total_entries: 5000,
        kg_nodes: 2500,
        kg_edges: 7500,
        db_size_bytes: 5120000,
    };

    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![],
        memory_stats: stats,
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    let kg_stats = response.kg_stats.unwrap();
    assert_eq!(kg_stats.nodes, 2500);
    assert_eq!(kg_stats.relationships, 7500);
    assert_eq!(kg_stats.total_entries, 5000);
    assert_eq!(kg_stats.db_size_bytes, 5120000);
}

#[test]
fn test_timestamp_conversion() {
    let now = SystemTime::now();

    let health = HealthStatus {
        installed: true,
        version: Some("1.0.0".to_string()),
        control_server_reachable: true,
        broker_registered: true,
        last_check: now,
        issues: vec![],
    };

    let response = to_health_check_response(health);

    // Verify timestamp is non-zero and reasonable
    assert!(response.last_check_timestamp > 0);

    // Should be within the last minute
    let now_timestamp = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    assert!((now_timestamp - response.last_check_timestamp).abs() < 60);
}

#[test]
fn test_dashboard_timestamp_conversion() {
    let now = SystemTime::now();

    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![],
        memory_stats: MemoryStats::empty(),
        timestamp: now,
    };

    let response = to_dashboard_response(data);

    // Verify timestamp is non-zero
    assert!(response.timestamp > 0);

    // Should be within the last minute
    let now_timestamp = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    assert!((now_timestamp - response.timestamp).abs() < 60);
}

#[test]
fn test_empty_version_handling() {
    let health = HealthStatus {
        installed: false,
        version: None,
        control_server_reachable: false,
        broker_registered: false,
        last_check: SystemTime::now(),
        issues: vec![],
    };

    let response = to_health_check_response(health);

    // None version should convert to empty string
    assert_eq!(response.version, "");
}

#[test]
fn test_multiple_agents_conversion() {
    let agents = vec![
        AgentInfo {
            id: "agent-1".to_string(),
            agent_type: "type-a".to_string(),
            status: "running".to_string(),
            tasks_completed: 10,
            uptime_secs: 100,
        },
        AgentInfo {
            id: "agent-2".to_string(),
            agent_type: "type-b".to_string(),
            status: "idle".to_string(),
            tasks_completed: 20,
            uptime_secs: 200,
        },
        AgentInfo {
            id: "agent-3".to_string(),
            agent_type: "type-c".to_string(),
            status: "stopped".to_string(),
            tasks_completed: 30,
            uptime_secs: 300,
        },
    ];

    let data = DashboardData {
        org_status: OrgStatus::running("org".to_string(), "run".to_string(), 3, 0),
        agents,
        runs: vec![],
        memory_stats: MemoryStats::empty(),
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    assert_eq!(response.agents.len(), 3);
    for (i, agent) in response.agents.iter().enumerate() {
        assert_eq!(agent.id, format!("agent-{}", i + 1));
        assert_eq!(agent.tasks_completed, (i as u32 + 1) * 10);
    }
}

#[test]
fn test_zero_values_handling() {
    let stats = MemoryStats {
        total_entries: 0,
        kg_nodes: 0,
        kg_edges: 0,
        db_size_bytes: 0,
    };

    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![],
        memory_stats: stats,
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    let kg_stats = response.kg_stats.unwrap();
    assert_eq!(kg_stats.nodes, 0);
    assert_eq!(kg_stats.relationships, 0);
    assert_eq!(kg_stats.total_entries, 0);
    assert_eq!(kg_stats.db_size_bytes, 0);
}

#[test]
fn test_large_values_handling() {
    let stats = MemoryStats {
        total_entries: u64::MAX,
        kg_nodes: u64::MAX / 2,
        kg_edges: u64::MAX / 3,
        db_size_bytes: u64::MAX / 4,
    };

    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![],
        memory_stats: stats,
        timestamp: SystemTime::now(),
    };

    let response = to_dashboard_response(data);

    let kg_stats = response.kg_stats.unwrap();
    assert_eq!(kg_stats.total_entries, u64::MAX);
    assert_eq!(kg_stats.nodes, u64::MAX / 2);
    assert_eq!(kg_stats.relationships, u64::MAX / 3);
    assert_eq!(kg_stats.db_size_bytes, u64::MAX / 4);
}
