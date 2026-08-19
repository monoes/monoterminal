# Monomind Integration API Specification

**Version:** 1.0  
**Date:** 2026-08-15  
**Status:** Implementation-Ready  
**Owner:** monomind-integration-engineer  
**Context:** task-15 (Monomind Integration) - Unblocking frontend-engineer

---

## Executive Summary

This document specifies the complete API contract for monomind integration in MONOTERMINAL per SRS §2.4. The integration is **already mostly complete** — protocol messages and backend implementation exist. What remains is:

1. **Adding 2 new message types** to the existing protocol (DetectionRequest/Response, MonitoringData)
2. **Wiring existing backend** (`crates/monomind-bridge`) to the WebSocket handler
3. **SessionManager hooks** to trigger detection on session events

**Timeline:** 4-6 hours implementation (protocol + handler + hooks)

---

## 1. Protocol Messages (Extend Existing)

### 1.1 Status: Already Defined ✅

The following messages **already exist** in `proto/monoterminal/v1/messages.proto`:

```protobuf
// Lines 11-14: Already in Envelope.oneof
message HealthCheckRequest {
  string project_dir = 1;  // Lines 120-122
}

message HealthCheckResponse {
  bool installed = 1;                  // Lines 124-131
  string version = 2;
  bool control_server_reachable = 3;
  bool broker_registered = 4;
  int64 last_check_timestamp = 5;
  repeated HealthIssue issues = 6;
}

message UpgradeRequest {
  string project_dir = 1;  // Lines 145-148
  bool confirmed = 2;      // User confirmation (required per SRS §2.4.3)
}

message UpgradeResponse {
  bool success = 1;         // Lines 150-155
  string old_version = 2;
  string new_version = 3;
  string output = 4;  // Full command output
}
```

**Backend Implementation:** ✅ Complete in `crates/monomind-bridge/src/health.rs`
- `run_doctor_check()` → produces HealthCheckResponse
- `upgrade_monomind()` → produces UpgradeResponse

### 1.2 Missing: 2 New Message Types ❌

Add to `proto/monoterminal/v1/messages.proto`:

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  oneof message {
    // ... existing messages ...
    
    // ADD THESE TWO:
    DetectionRequest detection_request = 15;
    DetectionResponse detection_response = 16;
    MonitoringData monitoring_data = 17;  // Server-to-client streaming only
  }
}

// ============================================================================
// Monomind Detection (SRS §2.4.1)
// ============================================================================

message DetectionRequest {
  string project_dir = 1;  // Directory to check (typically PTY cwd)
}

message DetectionResponse {
  bool found = 1;                      // Whether .monomind/ exists
  string monomind_root = 2;            // Root directory containing .monomind/ (if found)
  bool suggest_install = 3;            // Whether to show install suggestion
  bool dismiss_file_exists = 4;        // Whether user has dismissed the suggestion
  string banner_text = 5;              // MOTD-style banner to display (if suggest_install)
}

// ============================================================================
// Monitoring Data Stream (SRS §2.4.2)
// ============================================================================

message MonitoringData {
  // Org Status
  string org_name = 1;
  int32 active_agents = 2;
  int32 running_tasks = 3;
  
  // Knowledge Graph Stats
  int64 kg_nodes = 4;
  int64 kg_relationships = 5;
  int64 kg_last_updated = 6;  // Unix timestamp (seconds)
  
  // Run History (last 5 runs)
  repeated RunSummary recent_runs = 7;
}

message RunSummary {
  string run_id = 1;
  string goal = 2;
  int64 started_at = 3;   // Unix timestamp
  int64 completed_at = 4; // 0 if still running
  string status = 5;      // "running", "completed", "failed"
}
```

**Backend Implementation:**
- ✅ Detection: `crates/monomind-bridge/src/detection.rs` → `detect_monomind()`
- ❌ Monitoring: **NEW** - needs `crates/monomind-bridge/src/monitoring.rs`

---

## 2. Backend Implementation Plan

### 2.1 Already Complete ✅

**Files:**
- `crates/monomind-bridge/src/detection.rs` - Per-session detection (114 lines, 8 tests passing)
- `crates/monomind-bridge/src/health.rs` - Health check & upgrade (674 lines, 13 tests passing)
- `crates/monomind-bridge/src/lib.rs` - Public API exports

**API Surface:**
```rust
pub fn detect_monomind(path: &Path) -> DetectionResult;
pub async fn run_doctor_check(project_dir: &Path) -> Result<HealthStatus>;
pub async fn upgrade_monomind(project_dir: &Path) -> Result<UpgradeResult>;
```

### 2.2 Missing: Monitoring Module ❌

**File:** `crates/monomind-bridge/src/monitoring.rs` (NEW - ~200 lines)

```rust
/// Get current monitoring data from monomind CLI
///
/// Executes multiple commands to gather org/agent/KG status:
/// - npx monomind@latest status --json  → org name, active agents
/// - npx monomind@latest tasks --json   → running tasks
/// - npx monomind@latest memory kg-stats --json → KG stats
/// - npx monomind@latest runs --json --limit 5  → recent runs
pub async fn get_monitoring_data(project_dir: &Path) -> Result<MonitoringData>;

/// Start monitoring data stream (periodic updates)
///
/// Polls monomind CLI every N seconds and calls callback with updated data.
/// Designed to run in background task.
pub async fn start_monitoring_stream<F, Fut>(
    project_dir: &Path,
    interval: Duration,
    callback: F,
) -> Result<()>
where
    F: Fn(MonitoringData) -> Fut,
    Fut: std::future::Future<Output = ()>;
```

**Implementation Notes:**
- Poll interval: 10 seconds (configurable)
- Each poll executes 4 CLI commands in parallel
- Parse JSON output into MonitoringData struct
- Fail loud: any command failure surfaces in the stream (don't hide errors)

**Estimated LOC:** ~200 lines (+ ~100 lines tests)

---

## 3. WebSocket Handler Integration

### 3.1 Current State

**File:** `crates/master/src/server/handler.rs`

**Lines 192-206:** DashboardRequest handler has TODO (wrong message type, needs update)
**Missing:** HealthCheckRequest, UpgradeRequest, DetectionRequest handlers

### 3.2 Required Changes

**Step 1:** Add new message handlers in `process_message()`:

```rust
async fn process_message(
    envelope: Envelope,
    session_manager: &SessionManager,
    peer_addr: SocketAddr,
) -> Result<Option<Envelope>> {
    match envelope.message {
        // ... existing handlers ...
        
        // ADD THESE:
        
        Some(envelope::Message::HealthCheckRequest(req)) => {
            use monoterminal_monomind_bridge::run_doctor_check;
            
            let project_dir = PathBuf::from(req.project_dir);
            let health = run_doctor_check(&project_dir).await
                .map_err(|e| ServerError::Internal(format!("Health check failed: {}", e)))?;
            
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::HealthCheckResponse(
                    monoterminal_protocol::HealthCheckResponse {
                        installed: health.installed,
                        version: health.version.unwrap_or_default(),
                        control_server_reachable: health.control_server_reachable,
                        broker_registered: health.broker_registered,
                        last_check_timestamp: health.last_check
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                        issues: health.issues.into_iter().map(|issue| {
                            monoterminal_protocol::HealthIssue {
                                severity: match issue.severity {
                                    Severity::Info => IssueSeverity::Info,
                                    Severity::Warning => IssueSeverity::Warning,
                                    Severity::Error => IssueSeverity::Error,
                                } as i32,
                                message: issue.message,
                                resolution: issue.resolution.unwrap_or_default(),
                            }
                        }).collect(),
                    }
                )),
            };
            
            Ok(Some(response))
        }
        
        Some(envelope::Message::UpgradeRequest(req)) => {
            use monoterminal_monomind_bridge::upgrade_monomind;
            
            if !req.confirmed {
                return Err(ServerError::InvalidMessage(
                    "Upgrade requires confirmed=true".to_string()
                ));
            }
            
            let project_dir = PathBuf::from(req.project_dir);
            let result = upgrade_monomind(&project_dir).await
                .map_err(|e| ServerError::Internal(format!("Upgrade failed: {}", e)))?;
            
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::UpgradeResponse(
                    monoterminal_protocol::UpgradeResponse {
                        success: result.success,
                        old_version: result.old_version.unwrap_or_default(),
                        new_version: result.new_version.unwrap_or_default(),
                        output: result.output,
                    }
                )),
            };
            
            Ok(Some(response))
        }
        
        Some(envelope::Message::DetectionRequest(req)) => {
            use monoterminal_monomind_bridge::{detect_monomind, INSTALL_SUGGESTION_BANNER};
            
            let project_dir = PathBuf::from(req.project_dir);
            let detection = detect_monomind(&project_dir);
            
            let response = Envelope {
                sequence_number: envelope.sequence_number,
                message: Some(envelope::Message::DetectionResponse(
                    monoterminal_protocol::DetectionResponse {
                        found: detection.found,
                        monomind_root: detection.monomind_root
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                        suggest_install: detection.suggest_install,
                        dismiss_file_exists: detection.dismiss_file_exists,
                        banner_text: if detection.suggest_install {
                            INSTALL_SUGGESTION_BANNER.to_string()
                        } else {
                            String::new()
                        },
                    }
                )),
            };
            
            Ok(Some(response))
        }
        
        // ... rest of existing handlers ...
    }
}
```

**Step 2:** Add MonitoringData streaming support (background task)

```rust
// In handler.rs or new file: crates/master/src/server/monitoring_stream.rs

use monoterminal_monomind_bridge::start_monitoring_stream;
use tokio::sync::mpsc;

/// Start monitoring data stream for a session
pub async fn start_session_monitoring(
    session_id: String,
    project_dir: PathBuf,
    tx: mpsc::Sender<Envelope>,
) -> Result<()> {
    start_monitoring_stream(
        &project_dir,
        Duration::from_secs(10),
        |data| {
            let envelope = Envelope {
                sequence_number: 0,  // Server-initiated, no request seq
                message: Some(envelope::Message::MonitoringData(
                    monoterminal_protocol::MonitoringData {
                        org_name: data.org_name,
                        active_agents: data.active_agents,
                        running_tasks: data.running_tasks,
                        kg_nodes: data.kg_nodes,
                        kg_relationships: data.kg_relationships,
                        kg_last_updated: data.kg_last_updated,
                        recent_runs: data.recent_runs.into_iter().map(|run| {
                            monoterminal_protocol::RunSummary {
                                run_id: run.run_id,
                                goal: run.goal,
                                started_at: run.started_at,
                                completed_at: run.completed_at,
                                status: run.status,
                            }
                        }).collect(),
                    }
                )),
            };
            
            async move {
                let _ = tx.send(envelope).await;
            }
        }
    ).await
}
```

---

## 4. Session Manager Hooks

### 4.1 Trigger Points (Per SRS §2.4.1)

**Detection must run:**
1. On session creation (new PTY spawned)
2. On `cd` command (working directory change detected)

### 4.2 Implementation

**File:** `crates/master/src/session/session.rs`

```rust
impl Session {
    /// Create new session
    pub async fn new(
        shell_type: String,
        working_dir: PathBuf,
        rows: u32,
        cols: u32,
    ) -> Result<Self> {
        // ... existing PTY creation ...
        
        // ADD: Trigger monomind detection on session creation
        tokio::spawn({
            let working_dir = working_dir.clone();
            async move {
                use monoterminal_monomind_bridge::detect_monomind;
                let detection = detect_monomind(&working_dir);
                
                if detection.suggest_install {
                    tracing::info!(
                        path = %working_dir.display(),
                        "Monomind not detected, suggestion will be shown to client"
                    );
                    // Banner will be sent via DetectionResponse when client requests
                }
            }
        });
        
        // ... rest of existing code ...
    }
    
    /// Handle output from PTY (detect cwd changes)
    async fn process_output(&mut self, data: &[u8]) -> Result<()> {
        // ... existing output handling ...
        
        // ADD: Detect 'cd' command and trigger re-detection
        if self.detect_cwd_change(data) {
            let new_cwd = self.get_current_cwd()?;
            
            tokio::spawn({
                let new_cwd = new_cwd.clone();
                async move {
                    use monoterminal_monomind_bridge::detect_monomind;
                    let detection = detect_monomind(&new_cwd);
                    
                    if detection.suggest_install {
                        tracing::info!(
                            path = %new_cwd.display(),
                            "Working directory changed, monomind not detected in new location"
                        );
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// Detect if output contains a cd command (simple heuristic)
    fn detect_cwd_change(&self, data: &[u8]) -> bool {
        // Simple pattern: look for "cd " in output
        // More robust: track shell prompt changes or parse $PWD
        String::from_utf8_lossy(data).contains("cd ")
    }
    
    /// Get current working directory of PTY process
    fn get_current_cwd(&self) -> Result<PathBuf> {
        // Platform-specific:
        // Windows: use GetCurrentDirectory or process handle
        // Linux/macOS: read /proc/{pid}/cwd symlink
        
        #[cfg(target_os = "windows")]
        {
            // Read cwd from ConPTY process handle
            // (requires storing process handle in Session struct)
            todo!("Windows: Get PTY cwd via process handle")
        }
        
        #[cfg(unix)]
        {
            use std::fs;
            let cwd_link = format!("/proc/{}/cwd", self.pty.pid());
            fs::read_link(cwd_link)
                .context("Failed to read PTY cwd from /proc")
        }
    }
}
```

---

## 5. Frontend Integration (Already Complete)

**File:** `web/src/components/MonomindPanel.tsx`

**Lines 50-56:** Health check TODO → Call WebSocket with HealthCheckRequest
**Lines 84-98:** Upgrade TODO → Call WebSocket with UpgradeRequest

**No changes needed** once WebSocket handler is wired up. Frontend engineer just needs to:

1. Add WebSocket method calls (client already exists from task-9)
2. Subscribe to MonitoringData stream
3. Handle DetectionResponse (show banner/toast if suggest_install=true)

---

## 6. Testing Plan

### 6.1 Unit Tests ✅

**Already Passing:**
- `crates/monomind-bridge/src/detection.rs` - 13 tests ✅
- `crates/monomind-bridge/src/health.rs` - 13 tests ✅

**To Add:**
- `crates/monomind-bridge/src/monitoring.rs` - ~6 tests (NEW)

### 6.2 Integration Tests

**New file:** `crates/master/tests/monomind_integration_test.rs`

```rust
#[tokio::test]
async fn test_health_check_roundtrip() {
    // 1. Send HealthCheckRequest via WebSocket
    // 2. Verify HealthCheckResponse received
    // 3. Assert response matches expected structure
}

#[tokio::test]
async fn test_detection_on_session_create() {
    // 1. Create session in project WITHOUT .monomind/
    // 2. Send DetectionRequest
    // 3. Assert suggest_install=true, banner_text non-empty
}

#[tokio::test]
async fn test_upgrade_requires_confirmation() {
    // 1. Send UpgradeRequest with confirmed=false
    // 2. Assert ErrorResponse returned
}

#[tokio::test]
async fn test_monitoring_data_stream() {
    // 1. Subscribe to MonitoringData stream
    // 2. Wait for 2-3 updates
    // 3. Assert data structure matches schema
}
```

### 6.3 E2E Tests

**File:** `web/tests/e2e/monomind-panel.spec.ts` (already exists from task-7)

**To Add:**
- Test: Click "Run Health Check" → verify UI updates
- Test: Click "Upgrade" → confirm dialog → verify success message
- Test: Open panel in project without monomind → verify banner shown

---

## 7. Implementation Timeline

### Phase 1: Protocol & Backend (2-3 hours)

1. **Update protocol** (30 min):
   - Add DetectionRequest/Response/MonitoringData to messages.proto
   - Run `cargo build` to regenerate Rust bindings
   - Verify protocol crate compiles

2. **Create monitoring module** (90 min):
   - Implement `crates/monomind-bridge/src/monitoring.rs`
   - Write 6 unit tests
   - Export from lib.rs

3. **Wire handler** (60 min):
   - Add HealthCheck/Upgrade/Detection handlers to handler.rs
   - Add MonitoringData streaming background task
   - Test locally with WebSocket client

### Phase 2: Session Hooks (1-2 hours)

4. **Session detection hooks** (90 min):
   - Add detection call to Session::new()
   - Add cwd change detection to process_output()
   - Implement get_current_cwd() for Windows/Unix

### Phase 3: Testing & Polish (1 hour)

5. **Integration tests** (60 min):
   - Write 4 integration tests
   - Run full test suite
   - Fix any failing tests

**Total:** 4-6 hours

---

## 8. Deployment Checklist

- [ ] Protocol updated (messages.proto)
- [ ] monitoring.rs implemented and tested
- [ ] Handler wired (health/upgrade/detection)
- [ ] MonitoringData streaming implemented
- [ ] Session hooks added (creation + cwd change)
- [ ] Integration tests passing
- [ ] E2E tests updated
- [ ] Frontend connected (frontend-engineer)
- [ ] Manual smoke test: open panel, check health, trigger upgrade
- [ ] Documentation updated (this file)

---

## 9. Risk Mitigation (Per SRS §2.4.2)

**Historical Failure Modes (monoes/monomind#135, #136):**

1. **Dropped auth credentials** → MITIGATED: We use session JWT (already established), no separate token file
2. **Dead foreign-server pairing** → MITIGATED: Embedded in same WebSocket connection, fail loud on error
3. **Silent failures** → MITIGATED: Every monomind CLI call has explicit error handling, surfaces to UI

**Fail-Loud Principles:**
- Every CLI command failure logs error AND surfaces to client
- Health check failure shows red status chip (not hidden)
- Upgrade failure shows alert with full output (not silent)
- Detection failure logs warning but doesn't block session

---

## 10. Open Questions

1. **MonitoringData poll interval:** 10 seconds? Configurable?
   - **Answer:** Start with 10s, make configurable in Phase 2

2. **Cwd change detection:** Parse output or use platform APIs?
   - **Answer:** Use platform APIs (`/proc/{pid}/cwd` on Unix, process handle on Windows)

3. **Detection caching:** Should we cache detection results per project?
   - **Answer:** No caching in Phase 1 (detection is fast, <1ms)

4. **Monitoring stream lifecycle:** When to start/stop?
   - **Answer:** Start when MonomindPanel opens, stop when panel closes or session detaches

---

## Appendix A: File Locations

**Protocol:**
- `proto/monoterminal/v1/messages.proto` - Protocol definitions

**Backend:**
- `crates/monomind-bridge/src/lib.rs` - Public API
- `crates/monomind-bridge/src/detection.rs` - ✅ Complete
- `crates/monomind-bridge/src/health.rs` - ✅ Complete
- `crates/monomind-bridge/src/monitoring.rs` - ❌ NEW (200 LOC)

**Master Daemon:**
- `crates/master/src/server/handler.rs` - WebSocket message handlers
- `crates/master/src/session/session.rs` - Session lifecycle hooks

**Frontend:**
- `web/src/components/MonomindPanel.tsx` - ✅ UI complete (needs WebSocket wiring)
- `web/src/types/health.ts` - TypeScript types

**Tests:**
- `crates/monomind-bridge/src/detection.rs` - ✅ 13 tests passing
- `crates/monomind-bridge/src/health.rs` - ✅ 13 tests passing
- `crates/master/tests/monomind_integration_test.rs` - ❌ NEW

---

## Appendix B: Example Message Flow

### B.1 Health Check Flow

```
Client                   WebSocket Handler              monomind-bridge
  |                            |                              |
  |--HealthCheckRequest------->|                              |
  |  { project_dir: "/proj" }  |                              |
  |                            |--run_doctor_check()--------->|
  |                            |                              |--npx monomind doctor --json
  |                            |<--HealthStatus---------------|
  |                            |  { installed: true,          |
  |                            |    version: "1.2.3", ... }   |
  |<--HealthCheckResponse------|                              |
  |  { installed: true, ... }  |                              |
```

### B.2 Detection on Session Create

```
SessionManager           Session                  monomind-bridge
  |                        |                            |
  |--create_session()----->|                            |
  |                        |--detect_monomind()-------->|
  |                        |                            |--walk_to_monomind()
  |                        |<--DetectionResult----------|
  |                        |  { found: false,           |
  |                        |    suggest_install: true } |
  |                        |                            |
  (client will request DetectionResponse via WebSocket)
```

### B.3 Monitoring Data Stream

```
Client                   Handler                    monitoring_stream
  |                        |                              |
  |--subscribe----------- →|--start_monitoring_stream()-->|
  |                        |                              |
  |                        |                              |--poll every 10s
  |                        |                              |--npx monomind status
  |                        |                              |--npx monomind tasks
  |                        |                              |--npx monomind memory kg-stats
  |                        |<--MonitoringData-------------|
  |<--MonitoringData-------|  { org_name: "foo",         |
  |  { org_name: "foo",    |    active_agents: 3, ... }  |
  |    active_agents: 3 }  |                              |
  |                        |                              |
  (repeats every 10 seconds until stream closed)
```

---

**End of Specification**
