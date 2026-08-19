# Monomind Bridge Architecture Design

**Version:** 1.0  
**Date:** August 14, 2026  
**Status:** Design Review  
**Owner:** monomind-integration-engineer  
**SRS Reference:** §2.4, §2.1.5

---

## 1. Executive Summary

This document specifies the architecture for the Monomind Bridge, a first-class integration between MONOTERMINAL and monomind. The bridge enables per-session detection, embedded dashboard, health monitoring, and lifecycle hooks—all designed to avoid the failure modes experienced during this project's own build (monoes/monomind#135, #136).

**Key Design Principles:**

1. **Embedded, not separate**: Dashboard lives in the master daemon, authenticated via session JWT
2. **Fail loud, not silent**: Every integration point surfaces errors in the UI
3. **Optional feature**: Build succeeds without monomind installed
4. **Zero circular dependencies**: Bridge is a clean interface layer
5. **Performance budgets**: All hooks meet strict latency requirements

---

## 2. Architecture Overview

### 2.1 Component Structure

```
┌─────────────────────────────────────────────────────────────┐
│                    Master Daemon                            │
│  ┌───────────────────────────────────────────────────────┐  │
│  │           Session Manager                             │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │      PTY Process (ConPTY on Windows)           │  │  │
│  │  │                                                 │  │  │
│  │  │      cwd: /path/to/project                     │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │              │                                         │  │
│  │              ▼                                         │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │    Monomind Bridge (crate)                     │  │  │
│  │  │                                                 │  │  │
│  │  │  • Detection: walk_to_monomind()              │  │  │
│  │  │  • Health: check_health()                     │  │  │
│  │  │  • Hooks: trigger_hook()                      │  │  │
│  │  │  • Dashboard API: get_org_status()            │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                        │                                    │
│                        ▼                                    │
│  ┌───────────────────────────────────────────────────────┐  │
│  │          WebSocket/HTTP Server                        │  │
│  │                                                       │  │
│  │  • /api/monomind/status (JWT-authenticated)         │  │
│  │  • /api/monomind/health                             │  │
│  │  • /api/monomind/upgrade                            │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                        │
                        ▼ (WebSocket + JWT)
┌─────────────────────────────────────────────────────────────┐
│                  Web Client (React PWA)                      │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Terminal View (xterm.js)                 │  │
│  │                                                       │  │
│  │  [Banner: "Install monomind to unlock org features"] │  │
│  └───────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │         Embedded Monomind Dashboard                   │  │
│  │                                                       │  │
│  │  • Org Status         • Agent List                   │  │
│  │  • Run History        • Memory Stats                 │  │
│  │  • Health Check       • One-Click Upgrade            │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

#### Session Creation Flow

```
1. User opens session → Session Manager creates PTY
2. Session Manager queries PTY cwd → /path/to/project
3. Bridge.detect_monomind(cwd) → walks upward for .monomind/
4. If not found:
   a. Bridge returns InstallSuggestion event
   b. Session emits banner to PTY output
   c. WebSocket sends toast to web client
   d. User can dismiss (persisted to .monomind-suggest-dismissed)
5. If found:
   a. Bridge.trigger_hook(SESSION_START) → spawns monomind CLI
   b. Returns org state to session
   c. Dashboard API becomes available
```

#### CWD Change Flow

```
1. Shell executes `cd /new/project`
2. PTY emits OUTPUT_STREAM with cd command
3. Bridge detects directory change (OSC sequence or polling)
4. Bridge.detect_monomind(/new/project)
5. If .monomind/ status changed → repeat Session Creation Flow
```

#### Dashboard Query Flow

```
1. Web client requests /api/monomind/status
2. Master validates session JWT
3. Bridge.get_org_status(session_id) → calls monomind CLI
4. Returns: { org_id, agents: [...], runs: [...], memory_stats: {...} }
5. Web client renders embedded dashboard
```

---

## 3. Module Design: `crates/monomind-bridge`

### 3.1 Core API

```rust
// src/lib.rs

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use anyhow::Result;

/// Main bridge interface
pub struct MonomindBridge {
    config: BridgeConfig,
    detection_cache: Arc<RwLock<HashMap<PathBuf, DetectionResult>>>,
    health_check_scheduler: Arc<HealthScheduler>,
}

#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub health_check_interval_secs: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            health_check_interval_secs: 86400, // 24 hours
        }
    }
}

impl MonomindBridge {
    /// Create new bridge instance
    pub fn new(config: BridgeConfig) -> Result<Self> {
        Ok(Self {
            config,
            detection_cache: Arc::new(RwLock::new(HashMap::new())),
            health_check_scheduler: Arc::new(HealthScheduler::new()),
        })
    }
    
    /// Detect monomind in a directory tree
    pub async fn detect_monomind(&self, cwd: &Path) -> DetectionResult {
        // Check cache first
        if let Some(cached) = self.detection_cache.read().await.get(cwd) {
            return cached.clone();
        }
        
        let result = detection::walk_to_monomind(cwd)
            .map(|opt| DetectionResult {
                found: opt.is_some(),
                monomind_root: opt.clone(),
                suggest_install: opt.is_none() && detection::should_suggest_install(cwd),
                dismiss_file_exists: cwd.join(".monomind-suggest-dismissed").exists(),
            })
            .unwrap_or_else(|_| DetectionResult {
                found: false,
                monomind_root: None,
                suggest_install: false,
                dismiss_file_exists: false,
            });
        
        // Cache result
        self.detection_cache.write().await.insert(cwd.to_path_buf(), result.clone());
        result
    }
    
    /// Check health status
    pub async fn check_health(&self, project_dir: &Path) -> Result<HealthStatus> {
        health::run_doctor_check(project_dir).await
    }
    
    /// Trigger a lifecycle hook
    pub async fn trigger_hook(&self, hook: Hook) -> Result<HookResult> {
        hooks::trigger_hook(hook).await
    }
    
    /// Get org/agent status for dashboard
    pub async fn get_org_status(&self, project_dir: &Path) -> Result<OrgStatus> {
        dashboard::get_org_status(project_dir).await
    }
    
    /// Trigger one-click upgrade
    pub async fn upgrade(&self, project_dir: &Path) -> Result<UpgradeResult> {
        health::upgrade_monomind(project_dir).await
    }
}

/// Detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub found: bool,
    pub monomind_root: Option<PathBuf>,
    pub suggest_install: bool,
    pub dismiss_file_exists: bool,
}

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub control_server_reachable: bool,
    pub broker_registered: bool,
    pub last_check: SystemTime,
    pub issues: Vec<HealthIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub severity: Severity,
    pub message: String,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Org status for embedded dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgStatus {
    pub org_id: String,
    pub agents: Vec<AgentInfo>,
    pub recent_runs: Vec<RunInfo>,
    pub memory_stats: MemoryStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub role: String,
    pub status: AgentStatus,
    pub last_active: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    pub run_id: String,
    pub agent: String,
    pub started_at: SystemTime,
    pub status: RunStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: u64,
    pub knowledge_graph_nodes: u64,
    pub recent_sessions: u64,
}

/// Lifecycle hooks
#[derive(Debug, Clone)]
pub enum Hook {
    SessionStart { session_id: String, cwd: PathBuf },
    PreCommand { session_id: String, command: String },
    OutputStream { session_id: String, chunk: Vec<u8> },
    PostCommand { session_id: String, exit_code: i32 },
    SessionEnd { session_id: String },
}

#[derive(Debug, Clone)]
pub struct HookResult {
    pub action: HookAction,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum HookAction {
    Allow,
    Deny { reason: String },
    Overlay { content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub success: bool,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub output: String,
}

// Module declarations
mod detection;
mod health;
mod hooks;
mod dashboard;

pub use detection::{walk_to_monomind, should_suggest_install, dismiss_suggestion};
pub use health::{HealthScheduler, run_doctor_check, upgrade_monomind};
pub use hooks::trigger_hook;
pub use dashboard::get_org_status;
```

### 3.2 Detection Module

See Section 3.2 in full design document (code for walk_to_monomind, should_suggest_install, dismiss_suggestion).

### 3.3 Health Check Module

Implements `run_doctor_check()`, `upgrade_monomind()`, and `HealthScheduler` for daily automated checks.

### 3.4 Hook Integration Module

Implements hook triggers for SESSION_START, PRE_COMMAND, OUTPUT_STREAM, POST_COMMAND, SESSION_END with performance budgets and secret redaction.

**Secret Redaction Patterns:**
- Credentials like API keys, passwords, tokens, and secrets are redacted before logging
- Pattern matching replaces sensitive values with placeholder text
- Applied to all hook command inputs

### 3.5 Dashboard API Module

Implements `get_org_status()` by querying monomind CLI commands (`agent list`, `session info`, `memory search --stats`).

---

## 4. Master Daemon Integration

### 4.1 Session Manager Integration

The master daemon's session manager integrates the bridge at key lifecycle points:

```rust
// Conceptual integration in master daemon's session.rs

use monoterminal_monomind_bridge::{MonomindBridge, Hook};

pub struct Session {
    id: String,
    pty: ConPty,
    cwd: PathBuf,
    monomind: Option<MonomindBridge>,
}

impl Session {
    pub async fn new(config: SessionConfig) -> Result<Self> {
        let pty = ConPty::spawn(&config)?;
        let cwd = pty.working_directory()?;
        
        // Initialize monomind bridge if feature enabled
        #[cfg(feature = "monomind")]
        let monomind = {
            let bridge = MonomindBridge::new(Default::default())?;
            let detection = bridge.detect_monomind(&cwd).await;
            
            if detection.suggest_install {
                pty.write_banner(INSTALL_SUGGESTION_BANNER)?;
            }
            
            if detection.found {
                bridge.trigger_hook(Hook::SessionStart {
                    session_id: id.clone(),
                    cwd: cwd.clone(),
                }).await?;
            }
            
            Some(bridge)
        };
        
        #[cfg(not(feature = "monomind"))]
        let monomind = None;
        
        Ok(Self { id, pty, cwd, monomind })
    }
}
```

### 4.2 HTTP API Endpoints

Exposes `/api/monomind/status/:session_id`, `/api/monomind/health/:session_id`, `/api/monomind/upgrade/:session_id`, `/api/monomind/dismiss/:session_id` endpoints, all JWT-authenticated.

---

## 5. Web Client Integration

### 5.1 Dashboard Component

React component (`MonomindDashboard.tsx`) that:
- Polls `/api/monomind/status` every 5 seconds
- Displays org info, agents, recent runs, memory stats
- Shows health status chip (green/red)
- Provides one-click upgrade button

### 5.2 Install Suggestion Banner

Toast component showing "Install monomind" message with dismiss button that calls `/api/monomind/dismiss`.

---

## 6. Feature Flag Configuration

```toml
# Workspace Cargo.toml
[features]
default = ["monomind"]
monomind = ["monoterminal-master/monomind", "monoterminal-monomind-bridge/enabled"]

# Build without monomind
cargo build --no-default-features
```

---

## 7. Performance Budgets

| Hook | Budget | Strategy |
|------|--------|----------|
| **SESSION_START** | N/A | Async, non-blocking |
| **PRE_COMMAND** | <100ms | Timeout enforced, fail-open on timeout |
| **OUTPUT_STREAM** | <5ms | Skip most chunks, only trigger on markers |
| **POST_COMMAND** | N/A | Async, fire-and-forget |
| **SESSION_END** | N/A | Blocking OK, cleanup phase |

---

## 8. Error Handling & Failure Modes

### 8.1 Design Principle: Fail Loud, Not Silent

**Historical Context:** This project's build hit two silent failures in monomind's standalone dashboard:
- Dropped auth credentials (monoes/monomind#135)
- Dead foreign-server pairing (monoes/monomind#136)

Both produced NO visible warning. This integration is designed to make such failures impossible.

### 8.2 Failure Scenarios & Handling

| Failure | Detection | User-Visible Feedback | Recovery |
|---------|-----------|----------------------|----------|
| **monomind not installed** | `npx monomind` fails | Banner + dashboard shows "Not Installed" | One-click install via dashboard |
| **Control server unreachable** | Health check fails | Red status chip in dashboard | "Run `npx monomind@latest doctor --fix`" |
| **Broker registration dead** | Health check fails | Red status chip + specific error message | "Check .monomind/broker.json permissions" |
| **Hook timeout (PRE_COMMAND >100ms)** | Timeout enforced | Command proceeds with warning overlay | Log to session, auto-disable hook |
| **Dashboard API auth failure** | JWT validation fails | "Session expired - reconnect" toast | Force client reconnect |
| **Upgrade fails** | Upgrade command exit code | Error modal with full output | Manual intervention required |

---

## 9. Testing Strategy

### 9.1 Unit Tests
- Detection: walk upward, dismiss logic
- Health check: doctor command parsing
- Hook triggers: timeout enforcement, secret redaction

### 9.2 Integration Tests
- Full bridge lifecycle
- API endpoint responses
- JWT authentication flow

### 9.3 E2E Tests
- Dashboard loads and displays data
- Install suggestion can be dismissed
- Upgrade flow shows confirmation

---

## 10. Security Considerations

### 10.1 Authentication
- All dashboard API endpoints require valid session JWT
- Session scoping prevents cross-session data leaks

### 10.2 Privacy & Redaction
- Secrets are redacted from all hook inputs before logging or transmission

### 10.3 Command Injection Prevention
- All CLI calls use `Command::new()` with separate arguments (never shell interpolation)

---

## 11. Rollout Plan

### Phase 1: Core Detection (Week 1)
- Implement directory walking and detection logic
- Session creation detection
- Install suggestion banner

### Phase 2: Health Check (Week 2)
- Doctor command integration
- Daily health scheduler
- Dashboard health status API

### Phase 3: Hooks (Week 3)
- Lifecycle hook triggers
- Performance budget enforcement
- Secret redaction

### Phase 4: Dashboard (Week 4)
- API endpoints
- Web client dashboard component
- Real-time updates

### Phase 5: Testing & Hardening (Week 5)
- Unit/integration/E2E tests
- Performance profiling
- Security audit

---

## 12. Open Questions

1. **CWD change detection**: OSC sequences or polling?
   - **Recommendation**: OSC 7 sequence, fallback to 5s polling

2. **OUTPUT_STREAM hook**: <5ms realistic?
   - **Recommendation**: Skip most chunks, only trigger on error patterns

3. **Health check frequency**: Daily + session creation?
   - **Recommendation**: Check on session creation (if >24h since last) + daily background

4. **Dashboard updates**: Polling or WebSocket?
   - **Recommendation**: Start with 5s polling

5. **Monomind CLI version**: Minimum supported?
   - **Recommendation**: Always use `monomind@latest` via npx

---

## 13. Acceptance Criteria

- [ ] New session in project without `.monomind/` shows banner within 1s
- [ ] Banner dismissal creates `.monomind-suggest-dismissed`
- [ ] Dashboard accessible at `/api/monomind/status/:session_id`
- [ ] Dashboard authenticated via session JWT
- [ ] Health check runs daily + on session creation
- [ ] One-click upgrade with confirmation
- [ ] All hooks respect performance budgets
- [ ] Secrets redacted from all hook inputs
- [ ] Build succeeds with `--no-default-features`

---

## 14. Metrics

| Metric | Target |
|--------|--------|
| **Detection latency** | <100ms |
| **Health check latency** | <2s |
| **Dashboard API latency** | <500ms (p95) |
| **PRE_COMMAND hook latency** | <100ms (p99, timeout enforced) |
| **OUTPUT_STREAM hook latency** | <5ms (p99) |
| **Upgrade success rate** | >95% |

---

## Appendix A: Historical Failures Reference

### monoes/monomind#135: Dropped Auth Credentials
**Mitigation:** Embedded dashboard uses session JWT; no separate credential file

### monoes/monomind#136: Dead Foreign-Server Pairing
**Mitigation:** Daily health check + visible status chip; health issues surface immediately

---

**End of Design Document**
