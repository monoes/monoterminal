// Integration tests for embedded dashboard (§2.4.2)
//
// Tests the complete dashboard data flow including:
// - Dashboard data aggregation
// - Org status queries
// - Agent list queries
// - Run history queries
// - Memory stats queries
// - Error handling and fallback behavior

use monoterminal_monomind_bridge::{
    get_dashboard_data, AgentInfo, DashboardData, MemoryStats, OrgStatus, RunInfo,
};
use std::time::SystemTime;
use tempfile::TempDir;

#[tokio::test]
async fn test_dashboard_data_empty() {
    let empty = DashboardData::empty();

    assert!(!empty.org_status.running);
    assert_eq!(empty.org_status.name, None);
    assert_eq!(empty.agents.len(), 0);
    assert_eq!(empty.runs.len(), 0);
    assert_eq!(empty.memory_stats.total_entries, 0);
    assert_eq!(empty.memory_stats.kg_nodes, 0);
    assert_eq!(empty.memory_stats.kg_edges, 0);
}

#[tokio::test]
async fn test_org_status_not_configured() {
    let status = OrgStatus::not_configured();

    assert!(!status.running);
    assert_eq!(status.name, None);
    assert_eq!(status.run_id, None);
    assert_eq!(status.active_agents, 0);
    assert_eq!(status.pending_tasks, 0);
    assert_eq!(status.status_message, "No org configured");
}

#[tokio::test]
async fn test_org_status_running() {
    let status = OrgStatus::running("test-org".to_string(), "run-abc123".to_string(), 5, 10);

    assert!(status.running);
    assert_eq!(status.name, Some("test-org".to_string()));
    assert_eq!(status.run_id, Some("run-abc123".to_string()));
    assert_eq!(status.active_agents, 5);
    assert_eq!(status.pending_tasks, 10);
    assert_eq!(status.status_message, "Org running");
}

#[test]
fn test_agent_info_construction() {
    let agent = AgentInfo {
        id: "agent-001".to_string(),
        agent_type: "coder".to_string(),
        status: "running".to_string(),
        tasks_completed: 42,
        uptime_secs: 3600,
    };

    assert_eq!(agent.id, "agent-001");
    assert_eq!(agent.agent_type, "coder");
    assert_eq!(agent.status, "running");
    assert_eq!(agent.tasks_completed, 42);
    assert_eq!(agent.uptime_secs, 3600);
}

#[test]
fn test_run_info_construction() {
    let run = RunInfo {
        id: "run-123".to_string(),
        org_name: "test-org".to_string(),
        started_at: "2026-08-15T10:00:00Z".to_string(),
        ended_at: Some("2026-08-15T11:00:00Z".to_string()),
        outcome: "success".to_string(),
        tokens: 100000,
    };

    assert_eq!(run.id, "run-123");
    assert_eq!(run.org_name, "test-org");
    assert_eq!(run.started_at, "2026-08-15T10:00:00Z");
    assert_eq!(run.ended_at, Some("2026-08-15T11:00:00Z".to_string()));
    assert_eq!(run.outcome, "success");
    assert_eq!(run.tokens, 100000);
}

#[test]
fn test_memory_stats_empty() {
    let stats = MemoryStats::empty();

    assert_eq!(stats.total_entries, 0);
    assert_eq!(stats.kg_nodes, 0);
    assert_eq!(stats.kg_edges, 0);
    assert_eq!(stats.db_size_bytes, 0);
}

#[test]
fn test_memory_stats_with_data() {
    let stats = MemoryStats {
        total_entries: 1000,
        kg_nodes: 500,
        kg_edges: 1500,
        db_size_bytes: 1024000,
    };

    assert_eq!(stats.total_entries, 1000);
    assert_eq!(stats.kg_nodes, 500);
    assert_eq!(stats.kg_edges, 1500);
    assert_eq!(stats.db_size_bytes, 1024000);
}

#[tokio::test]
async fn test_get_dashboard_data_nonexistent_directory() {
    let temp = TempDir::new().unwrap();
    let nonexistent = temp.path().join("nonexistent");

    // Should return DashboardData (potentially empty), not panic
    let result = get_dashboard_data(&nonexistent).await;

    assert!(result.is_ok());
    if let Ok(data) = result {
        // Data should be valid even if commands failed
        assert!(!data.org_status.running || !data.org_status.status_message.is_empty());
    }
}

#[tokio::test]
async fn test_get_dashboard_data_empty_project() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().to_path_buf();

    // Run dashboard query on empty directory
    let result = get_dashboard_data(&project).await;

    assert!(result.is_ok());
    if let Ok(data) = result {
        // Should return empty/default data (no monomind installed)
        assert!(!data.org_status.running);
        assert_eq!(data.agents.len(), 0);
        assert_eq!(data.runs.len(), 0);
    }
}

#[test]
fn test_agent_info_serialization() {
    use serde_json;

    let agent = AgentInfo {
        id: "agent-001".to_string(),
        agent_type: "coder".to_string(),
        status: "running".to_string(),
        tasks_completed: 10,
        uptime_secs: 3600,
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&agent).unwrap();
    let deserialized: AgentInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(agent, deserialized);
}

#[test]
fn test_run_info_serialization() {
    use serde_json;

    let run = RunInfo {
        id: "run-123".to_string(),
        org_name: "test-org".to_string(),
        started_at: "2026-08-15T10:00:00Z".to_string(),
        ended_at: Some("2026-08-15T11:00:00Z".to_string()),
        outcome: "success".to_string(),
        tokens: 100000,
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&run).unwrap();
    let deserialized: RunInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(run, deserialized);
}

#[test]
fn test_org_status_serialization() {
    use serde_json;

    let status = OrgStatus::running("test-org".to_string(), "run-123".to_string(), 5, 10);

    // Serialize and deserialize
    let json = serde_json::to_string(&status).unwrap();
    let deserialized: OrgStatus = serde_json::from_str(&json).unwrap();

    assert_eq!(status, deserialized);
}

#[test]
fn test_memory_stats_serialization() {
    use serde_json;

    let stats = MemoryStats {
        total_entries: 1000,
        kg_nodes: 500,
        kg_edges: 1500,
        db_size_bytes: 1024000,
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: MemoryStats = serde_json::from_str(&json).unwrap();

    assert_eq!(stats, deserialized);
}

#[test]
fn test_dashboard_data_serialization() {
    use serde_json;

    let data = DashboardData {
        org_status: OrgStatus::running("test-org".to_string(), "run-123".to_string(), 3, 5),
        agents: vec![AgentInfo {
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

    // Serialize and deserialize
    let json = serde_json::to_string(&data).unwrap();
    let deserialized: DashboardData = serde_json::from_str(&json).unwrap();

    assert_eq!(data.org_status.name, deserialized.org_status.name);
    assert_eq!(data.agents.len(), deserialized.agents.len());
    assert_eq!(data.runs.len(), deserialized.runs.len());
}

#[tokio::test]
async fn test_dashboard_data_timestamp() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().to_path_buf();

    let result = get_dashboard_data(&project).await;

    assert!(result.is_ok());
    if let Ok(data) = result {
        // Verify timestamp is recent (within last second)
        let now = SystemTime::now();
        let duration_since = now.duration_since(data.timestamp).unwrap();
        assert!(duration_since < std::time::Duration::from_secs(1));
    }
}

#[test]
fn test_run_info_without_end_time() {
    let running_run = RunInfo {
        id: "run-456".to_string(),
        org_name: "test-org".to_string(),
        started_at: "2026-08-15T10:00:00Z".to_string(),
        ended_at: None, // Still running
        outcome: "running".to_string(),
        tokens: 50000,
    };

    assert!(running_run.ended_at.is_none());
    assert_eq!(running_run.outcome, "running");
}

#[tokio::test]
async fn test_concurrent_dashboard_queries() {
    use std::sync::Arc;

    let temp = TempDir::new().unwrap();
    let project = Arc::new(temp.path().to_path_buf());

    // Spawn multiple concurrent dashboard queries
    let mut handles = vec![];

    for _ in 0..5 {
        let project = Arc::clone(&project);
        let handle = tokio::spawn(async move {
            let _result = get_dashboard_data(&project).await;
            // Result should be Ok, we just verify it doesn't panic
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[test]
fn test_org_status_clone() {
    let original = OrgStatus::running("test-org".to_string(), "run-123".to_string(), 5, 10);
    let cloned = original.clone();

    assert_eq!(original.running, cloned.running);
    assert_eq!(original.name, cloned.name);
    assert_eq!(original.run_id, cloned.run_id);
    assert_eq!(original.active_agents, cloned.active_agents);
    assert_eq!(original.pending_tasks, cloned.pending_tasks);
}

#[test]
fn test_agent_info_clone() {
    let original = AgentInfo {
        id: "agent-1".to_string(),
        agent_type: "coder".to_string(),
        status: "running".to_string(),
        tasks_completed: 10,
        uptime_secs: 3600,
    };
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_memory_stats_clone() {
    let original = MemoryStats {
        total_entries: 1000,
        kg_nodes: 500,
        kg_edges: 1500,
        db_size_bytes: 1024000,
    };
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_dashboard_data_with_multiple_agents() {
    let data = DashboardData {
        org_status: OrgStatus::running("multi-agent-org".to_string(), "run-789".to_string(), 3, 5),
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
            AgentInfo {
                id: "agent-3".to_string(),
                agent_type: "tester".to_string(),
                status: "running".to_string(),
                tasks_completed: 15,
                uptime_secs: 7200,
            },
        ],
        runs: vec![],
        memory_stats: MemoryStats::empty(),
        timestamp: SystemTime::now(),
    };

    assert_eq!(data.agents.len(), 3);
    assert_eq!(data.org_status.active_agents, 3);
}

#[test]
fn test_dashboard_data_with_run_history() {
    let data = DashboardData {
        org_status: OrgStatus::not_configured(),
        agents: vec![],
        runs: vec![
            RunInfo {
                id: "run-1".to_string(),
                org_name: "test-org".to_string(),
                started_at: "2026-08-15T08:00:00Z".to_string(),
                ended_at: Some("2026-08-15T09:00:00Z".to_string()),
                outcome: "success".to_string(),
                tokens: 50000,
            },
            RunInfo {
                id: "run-2".to_string(),
                org_name: "test-org".to_string(),
                started_at: "2026-08-15T09:30:00Z".to_string(),
                ended_at: Some("2026-08-15T10:00:00Z".to_string()),
                outcome: "success".to_string(),
                tokens: 75000,
            },
            RunInfo {
                id: "run-3".to_string(),
                org_name: "test-org".to_string(),
                started_at: "2026-08-15T10:30:00Z".to_string(),
                ended_at: None,
                outcome: "running".to_string(),
                tokens: 25000,
            },
        ],
        memory_stats: MemoryStats::empty(),
        timestamp: SystemTime::now(),
    };

    assert_eq!(data.runs.len(), 3);

    // Check completed runs
    assert!(data.runs[0].ended_at.is_some());
    assert!(data.runs[1].ended_at.is_some());

    // Check running run
    assert!(data.runs[2].ended_at.is_none());
    assert_eq!(data.runs[2].outcome, "running");
}
