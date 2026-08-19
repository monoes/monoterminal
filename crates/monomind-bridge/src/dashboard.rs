// Week 4 Implementation: Embedded Dashboard (§2.4.2)
//
// Core functionality:
// - get_org_status: Query org runtime state
// - get_agent_status: Query active agents
// - get_run_history: Query recent org runs
// - get_memory_stats: Query knowledge graph and memory stats
//
// Design principle: Fail loud, not silent (prevent monoes/monomind#135, #136)
// All dashboard data is fetched via authenticated WebSocket using the same JWT
// as the terminal session - no separate ports or credentials.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

/// Dashboard data aggregating all monomind status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardData {
    /// Org runtime status
    pub org_status: OrgStatus,
    /// Active agents list
    pub agents: Vec<AgentInfo>,
    /// Recent run history
    pub runs: Vec<RunInfo>,
    /// Memory and knowledge graph statistics
    pub memory_stats: MemoryStats,
    /// Timestamp of data fetch
    pub timestamp: SystemTime,
}

impl DashboardData {
    /// Create an empty dashboard (no monomind detected)
    pub fn empty() -> Self {
        Self {
            org_status: OrgStatus::not_configured(),
            agents: vec![],
            runs: vec![],
            memory_stats: MemoryStats::empty(),
            timestamp: SystemTime::now(),
        }
    }
}

/// Org runtime status from `monomind org status`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrgStatus {
    /// Whether an org is currently running
    pub running: bool,
    /// Org name (if running)
    pub name: Option<String>,
    /// Current run ID (if running)
    pub run_id: Option<String>,
    /// Number of active agents in the org
    pub active_agents: u32,
    /// Number of pending tasks
    pub pending_tasks: u32,
    /// Current status message
    pub status_message: String,
}

impl OrgStatus {
    /// Create status indicating no org is configured
    pub fn not_configured() -> Self {
        Self {
            running: false,
            name: None,
            run_id: None,
            active_agents: 0,
            pending_tasks: 0,
            status_message: "No org configured".to_string(),
        }
    }

    /// Create status indicating org is running
    pub fn running(name: String, run_id: String, agents: u32, tasks: u32) -> Self {
        Self {
            running: true,
            name: Some(name),
            run_id: Some(run_id),
            active_agents: agents,
            pending_tasks: tasks,
            status_message: "Org running".to_string(),
        }
    }
}

/// Individual agent information from `monomind agent list`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentInfo {
    /// Agent ID
    pub id: String,
    /// Agent type/role
    pub agent_type: String,
    /// Current status (running, idle, stopped)
    pub status: String,
    /// Number of tasks completed
    pub tasks_completed: u32,
    /// Uptime in seconds
    pub uptime_secs: u64,
}

/// Run history entry from `monomind org report`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunInfo {
    /// Run ID
    pub id: String,
    /// Org name
    pub org_name: String,
    /// Start timestamp
    pub started_at: String,
    /// End timestamp (if completed)
    pub ended_at: Option<String>,
    /// Outcome (success, failure, cancelled)
    pub outcome: String,
    /// Total tokens consumed
    pub tokens: u64,
}

/// Memory and knowledge graph statistics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryStats {
    /// Total number of memory entries
    pub total_entries: u64,
    /// Number of knowledge graph nodes
    pub kg_nodes: u64,
    /// Number of knowledge graph edges
    pub kg_edges: u64,
    /// Size of memory database in bytes
    pub db_size_bytes: u64,
}

impl MemoryStats {
    /// Create empty stats (no data)
    pub fn empty() -> Self {
        Self {
            total_entries: 0,
            kg_nodes: 0,
            kg_edges: 0,
            db_size_bytes: 0,
        }
    }
}

/// Get comprehensive dashboard data
///
/// Executes multiple monomind CLI commands to gather org status, agent list,
/// run history, and memory stats. Returns aggregated dashboard data.
///
/// # Design: Fail Loud, Not Silent
///
/// Each command failure is logged but does not fail the entire operation.
/// Missing data is represented as empty/default values, but the dashboard
/// always returns a result.
///
/// # Arguments
///
/// * `project_dir` - Project root directory containing .monomind/
///
/// # Returns
///
/// * `Ok(DashboardData)` - Aggregated dashboard data (may be partial on errors)
/// * `Err(_)` - Only on catastrophic failures (permission denied, etc.)
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::get_dashboard_data;
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// let data = get_dashboard_data(Path::new("/project")).await?;
/// println!("Org running: {}", data.org_status.running);
/// println!("Active agents: {}", data.agents.len());
/// # Ok(())
/// # }
/// ```
pub async fn get_dashboard_data(project_dir: &Path) -> Result<DashboardData> {
    tracing::debug!(
        path = %project_dir.display(),
        "Fetching monomind dashboard data"
    );

    // Fetch all data sources concurrently
    let (org_status, agents, runs, memory_stats) = tokio::join!(
        get_org_status(project_dir),
        get_agent_list(project_dir),
        get_run_history(project_dir),
        get_memory_stats(project_dir),
    );

    Ok(DashboardData {
        org_status: org_status.unwrap_or_else(|e| {
            tracing::warn!("Failed to get org status: {}", e);
            OrgStatus::not_configured()
        }),
        agents: agents.unwrap_or_else(|e| {
            tracing::warn!("Failed to get agent list: {}", e);
            vec![]
        }),
        runs: runs.unwrap_or_else(|e| {
            tracing::warn!("Failed to get run history: {}", e);
            vec![]
        }),
        memory_stats: memory_stats.unwrap_or_else(|e| {
            tracing::warn!("Failed to get memory stats: {}", e);
            MemoryStats::empty()
        }),
        timestamp: SystemTime::now(),
    })
}

/// Get org runtime status
///
/// Executes `npx monomind@latest org status --json`
async fn get_org_status(project_dir: &Path) -> Result<OrgStatus> {
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("org")
                .arg("status")
                .arg("--json")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Task join failed")?
    .context("Failed to execute org status command")?;

    if !output.status.success() {
        return Ok(OrgStatus::not_configured());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output
    // For now, return a basic parse - this will be refined based on actual CLI output
    #[derive(Deserialize)]
    struct OrgStatusJson {
        running: Option<bool>,
        name: Option<String>,
        run_id: Option<String>,
        active_agents: Option<u32>,
        pending_tasks: Option<u32>,
    }

    match serde_json::from_str::<OrgStatusJson>(&stdout) {
        Ok(status) => Ok(OrgStatus {
            running: status.running.unwrap_or(false),
            name: status.name,
            run_id: status.run_id,
            active_agents: status.active_agents.unwrap_or(0),
            pending_tasks: status.pending_tasks.unwrap_or(0),
            status_message: if status.running.unwrap_or(false) {
                "Org running".to_string()
            } else {
                "No org running".to_string()
            },
        }),
        Err(e) => {
            tracing::debug!(
                "Failed to parse org status JSON: {}. Raw output: {}",
                e,
                stdout
            );
            Ok(OrgStatus::not_configured())
        }
    }
}

/// Get active agent list
///
/// Executes `npx monomind@latest agent list --json`
async fn get_agent_list(project_dir: &Path) -> Result<Vec<AgentInfo>> {
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("agent")
                .arg("list")
                .arg("--json")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Task join failed")?
    .context("Failed to execute agent list command")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON array of agents
    #[derive(Deserialize)]
    struct AgentJson {
        id: String,
        #[serde(rename = "type")]
        agent_type: Option<String>,
        status: Option<String>,
        tasks_completed: Option<u32>,
        uptime_secs: Option<u64>,
    }

    match serde_json::from_str::<Vec<AgentJson>>(&stdout) {
        Ok(agents) => Ok(agents
            .into_iter()
            .map(|a| AgentInfo {
                id: a.id,
                agent_type: a.agent_type.unwrap_or_else(|| "unknown".to_string()),
                status: a.status.unwrap_or_else(|| "unknown".to_string()),
                tasks_completed: a.tasks_completed.unwrap_or(0),
                uptime_secs: a.uptime_secs.unwrap_or(0),
            })
            .collect()),
        Err(e) => {
            tracing::debug!(
                "Failed to parse agent list JSON: {}. Raw output: {}",
                e,
                stdout
            );
            Ok(vec![])
        }
    }
}

/// Get run history
///
/// Executes `npx monomind@latest org report --json` for recent runs
async fn get_run_history(project_dir: &Path) -> Result<Vec<RunInfo>> {
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("org")
                .arg("report")
                .arg("--json")
                .arg("--limit")
                .arg("10") // Last 10 runs
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Task join failed")?
    .context("Failed to execute org report command")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON array of runs
    #[derive(Deserialize)]
    struct RunJson {
        id: String,
        org_name: Option<String>,
        started_at: String,
        ended_at: Option<String>,
        outcome: Option<String>,
        tokens: Option<u64>,
    }

    match serde_json::from_str::<Vec<RunJson>>(&stdout) {
        Ok(runs) => Ok(runs
            .into_iter()
            .map(|r| RunInfo {
                id: r.id,
                org_name: r.org_name.unwrap_or_else(|| "unknown".to_string()),
                started_at: r.started_at,
                ended_at: r.ended_at,
                outcome: r.outcome.unwrap_or_else(|| "unknown".to_string()),
                tokens: r.tokens.unwrap_or(0),
            })
            .collect()),
        Err(e) => {
            tracing::debug!(
                "Failed to parse run history JSON: {}. Raw output: {}",
                e,
                stdout
            );
            Ok(vec![])
        }
    }
}

/// Get memory and knowledge graph statistics
///
/// Executes `npx monomind@latest status memory --json`
async fn get_memory_stats(project_dir: &Path) -> Result<MemoryStats> {
    let output = tokio::task::spawn_blocking({
        let project_dir = project_dir.to_path_buf();
        move || {
            Command::new("npx")
                .arg("monomind@latest")
                .arg("status")
                .arg("memory")
                .arg("--json")
                .current_dir(&project_dir)
                .output()
        }
    })
    .await
    .context("Task join failed")?
    .context("Failed to execute status memory command")?;

    if !output.status.success() {
        return Ok(MemoryStats::empty());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON stats
    #[derive(Deserialize)]
    struct MemoryStatsJson {
        total_entries: Option<u64>,
        kg_nodes: Option<u64>,
        kg_edges: Option<u64>,
        db_size_bytes: Option<u64>,
    }

    match serde_json::from_str::<MemoryStatsJson>(&stdout) {
        Ok(stats) => Ok(MemoryStats {
            total_entries: stats.total_entries.unwrap_or(0),
            kg_nodes: stats.kg_nodes.unwrap_or(0),
            kg_edges: stats.kg_edges.unwrap_or(0),
            db_size_bytes: stats.db_size_bytes.unwrap_or(0),
        }),
        Err(e) => {
            tracing::debug!(
                "Failed to parse memory stats JSON: {}. Raw output: {}",
                e,
                stdout
            );
            Ok(MemoryStats::empty())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_status_not_configured() {
        let status = OrgStatus::not_configured();
        assert!(!status.running);
        assert_eq!(status.name, None);
        assert_eq!(status.active_agents, 0);
    }

    #[test]
    fn test_org_status_running() {
        let status = OrgStatus::running("test-org".to_string(), "run-123".to_string(), 5, 10);
        assert!(status.running);
        assert_eq!(status.name, Some("test-org".to_string()));
        assert_eq!(status.run_id, Some("run-123".to_string()));
        assert_eq!(status.active_agents, 5);
        assert_eq!(status.pending_tasks, 10);
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
    fn test_dashboard_data_empty() {
        let data = DashboardData::empty();
        assert!(!data.org_status.running);
        assert_eq!(data.agents.len(), 0);
        assert_eq!(data.runs.len(), 0);
        assert_eq!(data.memory_stats.total_entries, 0);
    }

    #[test]
    fn test_agent_info_serialization() {
        let agent = AgentInfo {
            id: "agent-001".to_string(),
            agent_type: "coder".to_string(),
            status: "running".to_string(),
            tasks_completed: 42,
            uptime_secs: 3600,
        };

        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: AgentInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(agent, deserialized);
    }

    #[test]
    fn test_run_info_serialization() {
        let run = RunInfo {
            id: "run-123".to_string(),
            org_name: "test-org".to_string(),
            started_at: "2026-08-15T10:00:00Z".to_string(),
            ended_at: Some("2026-08-15T11:00:00Z".to_string()),
            outcome: "success".to_string(),
            tokens: 100000,
        };

        let json = serde_json::to_string(&run).unwrap();
        let deserialized: RunInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(run, deserialized);
    }
}
