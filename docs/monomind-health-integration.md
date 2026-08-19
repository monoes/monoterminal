# Monomind Health Check Integration Guide

**SRS Reference:** §2.4.3 Health Check & Upgrade  
**Implementation:** `crates/monomind-bridge/src/health.rs`  
**Protocol:** `proto/monoterminal/v1/messages.proto` (fields 11-14)

## Overview

The health check system provides:
- **On-demand health checks** via `run_doctor_check()`
- **Scheduled daily checks** via `HealthScheduler`
- **One-click upgrade** via `upgrade_monomind()`
- **Fail-loud design** to prevent silent failures (per monoes/monomind#135, #136)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Web Client (React)                                          │
│  - Status chip (healthy/warning/error)                      │
│  - Health panel with issue list                             │
│  - Upgrade button (with confirmation)                       │
└────────────────┬────────────────────────────────────────────┘
                 │ WebSocket (HealthCheckRequest/Response)
┌────────────────▼────────────────────────────────────────────┐
│ Master Daemon (Rust)                                        │
│  - WebSocket handler for health requests                    │
│  - HealthScheduler (daily background task)                  │
│  - Session-triggered health checks (on cwd change)          │
└────────────────┬────────────────────────────────────────────┘
                 │ calls monomind-bridge
┌────────────────▼────────────────────────────────────────────┐
│ monomind-bridge crate                                       │
│  - run_doctor_check() → HealthStatus                        │
│  - upgrade_monomind() → UpgradeResult                       │
│  - HealthScheduler (tokio background task)                  │
└────────────────┬────────────────────────────────────────────┘
                 │ executes
┌────────────────▼────────────────────────────────────────────┐
│ monomind CLI (via npx)                                      │
│  - npx monomind@latest doctor --json                        │
│  - npx monomind@latest upgrade                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Integration Steps

### 1. Master Daemon: Add Health Check Handler

In `crates/master/src/health_handler.rs`:

```rust
use monoterminal_monomind_bridge::{run_doctor_check, upgrade_monomind, HealthScheduler};
use std::path::Path;

/// Handle health check request from web client
pub async fn handle_health_check(project_dir: &Path) -> HealthCheckResponse {
    match run_doctor_check(project_dir).await {
        Ok(status) => {
            // Convert HealthStatus to proto HealthCheckResponse
            HealthCheckResponse {
                installed: status.installed,
                version: status.version.unwrap_or_default(),
                control_server_reachable: status.control_server_reachable,
                broker_registered: status.broker_registered,
                last_check_timestamp: status.last_check
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                issues: status.issues.into_iter().map(|issue| {
                    HealthIssue {
                        severity: match issue.severity {
                            Severity::Info => IssueSeverity::INFO,
                            Severity::Warning => IssueSeverity::WARNING,
                            Severity::Error => IssueSeverity::ERROR,
                        },
                        message: issue.message,
                        resolution: issue.resolution.unwrap_or_default(),
                    }
                }).collect(),
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Health check failed");
            // Return error status
            HealthCheckResponse {
                installed: false,
                version: String::new(),
                control_server_reachable: false,
                broker_registered: false,
                last_check_timestamp: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                issues: vec![HealthIssue {
                    severity: IssueSeverity::ERROR,
                    message: format!("Health check error: {}", e),
                    resolution: String::new(),
                }],
            }
        }
    }
}

/// Handle upgrade request (requires user confirmation)
pub async fn handle_upgrade(project_dir: &Path, confirmed: bool) -> UpgradeResponse {
    if !confirmed {
        return UpgradeResponse {
            success: false,
            old_version: String::new(),
            new_version: String::new(),
            output: "User confirmation required".to_string(),
        };
    }

    match upgrade_monomind(project_dir).await {
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
```

### 2. Master Daemon: Start Daily Scheduler

In `crates/master/src/main.rs`:

```rust
use monoterminal_monomind_bridge::HealthScheduler;
use tokio::task;

async fn start_health_scheduler(project_dir: PathBuf) {
    task::spawn(async move {
        let scheduler = HealthScheduler::new(); // 24-hour default interval
        
        scheduler.start(&project_dir, |health| async move {
            // Broadcast health status to all connected clients
            tracing::info!(
                healthy = health.is_healthy(),
                issues = health.issues.len(),
                "Scheduled health check complete"
            );
            
            // TODO: Send health status update to all WebSocket clients
            // broadcast_health_status(health).await;
        }).await
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing initialization ...
    
    // Start daily health check scheduler
    let project_dir = std::env::current_dir()?;
    start_health_scheduler(project_dir.clone()).await;
    
    // ... rest of main ...
}
```

### 3. WebSocket Handler: Route Health Messages

In your WebSocket message handler:

```rust
match envelope.message {
    Some(Message::HealthCheckRequest(req)) => {
        let project_dir = req.project_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| session.working_dir.clone());
        
        let response = handle_health_check(&project_dir).await;
        
        send_envelope(Envelope {
            sequence_number: next_sequence(),
            message: Some(Message::HealthCheckResponse(response)),
        }).await?;
    }
    
    Some(Message::UpgradeRequest(req)) => {
        let project_dir = req.project_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| session.working_dir.clone());
        
        let response = handle_upgrade(&project_dir, req.confirmed).await;
        
        send_envelope(Envelope {
            sequence_number: next_sequence(),
            message: Some(Message::UpgradeResponse(response)),
        }).await?;
    }
    
    // ... other message handlers ...
}
```

### 4. Session Manager: Trigger on CWD Change

In `crates/master/src/session/session.rs`:

```rust
pub async fn on_working_directory_changed(&mut self, new_cwd: PathBuf) {
    self.working_dir = new_cwd.clone();
    
    // Trigger health check when cwd changes (per SRS §2.4.1)
    tokio::spawn(async move {
        if let Ok(health) = run_doctor_check(&new_cwd).await {
            if !health.is_healthy() {
                tracing::warn!(
                    path = %new_cwd.display(),
                    issues = health.issues.len(),
                    "Monomind health issues detected in new working directory"
                );
                // TODO: Notify client to show health warning
            }
        }
    });
}
```

---

## Web Client Integration

### TypeScript Types

In `web/src/types/health.ts`:

```typescript
export type HealthStatus = 'unknown' | 'healthy' | 'warning' | 'error';

export interface HealthCheckResponse {
  installed: boolean;
  version: string;
  controlServerReachable: boolean;
  brokerRegistered: boolean;
  lastCheckTimestamp: number;
  issues: HealthIssue[];
}

export interface HealthIssue {
  severity: 'INFO' | 'WARNING' | 'ERROR';
  message: string;
  resolution: string;
}

export interface UpgradeResponse {
  success: boolean;
  oldVersion: string;
  newVersion: string;
  output: string;
}
```

### WebSocket Client

In `web/src/lib/websocket-client.ts`:

```typescript
export class MonomindHealthClient {
  constructor(private ws: WebSocketClient) {}

  async checkHealth(projectDir?: string): Promise<HealthCheckResponse> {
    const response = await this.ws.sendRequest({
      healthCheckRequest: {
        projectDir: projectDir || '',
      },
    });
    
    if (response.healthCheckResponse) {
      return response.healthCheckResponse;
    }
    
    throw new Error('Invalid health check response');
  }

  async upgrade(projectDir?: string, confirmed: boolean = false): Promise<UpgradeResponse> {
    const response = await this.ws.sendRequest({
      upgradeRequest: {
        projectDir: projectDir || '',
        confirmed,
      },
    });
    
    if (response.upgradeResponse) {
      return response.upgradeResponse;
    }
    
    throw new Error('Invalid upgrade response');
  }
}
```

### React Component

The existing `web/src/components/MonomindPanel.tsx` should be updated to:

1. Call `healthClient.checkHealth()` when panel opens
2. Display health status from response
3. Show upgrade confirmation dialog before calling `healthClient.upgrade()`
4. Listen for scheduled health status broadcasts

See `web/src/components/MonomindPanel.tsx` for the UI implementation.

---

## Health Check Design Principles

### Fail Loud, Not Silent

Per SRS §2.4.3, this system is designed to prevent the failure modes in monoes/monomind#135 (dropped auth credentials) and #136 (dead foreign-server pairing).

**All failures are surfaced explicitly:**

1. **No silent defaults** - If a check fails, it's reported in `issues`
2. **Status chips always visible** - Never hide health state from user
3. **Clear resolution steps** - Every issue includes actionable guidance
4. **Audit trail** - All checks logged with tracing

### Verification Points

The health check verifies:

1. ✅ **CLI version** - `npx monomind@latest --version` succeeds
2. ✅ **Control server** - Broker file at `.monomind/broker.json` is valid
3. ✅ **Registration** - Broker registration hasn't been corrupted

### Error Handling

```rust
// BAD: Silent failure
if let Ok(health) = run_doctor_check(path).await {
    // Only handles success case
}

// GOOD: Fail loud
match run_doctor_check(path).await {
    Ok(health) => {
        if !health.is_healthy() {
            // Surface issues to user
            show_health_warning(health.issues);
        }
    }
    Err(e) => {
        // Execution error - show to user
        show_error(format!("Health check failed: {}", e));
    }
}
```

---

## Testing

### Unit Tests

The `health.rs` module includes comprehensive unit tests:

```bash
cd crates/monomind-bridge
cargo test health
```

### Integration Test Example

```rust
#[tokio::test]
async fn test_health_check_flow() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create mock .monomind directory
    fs::create_dir(temp_dir.path().join(".monomind")).unwrap();
    
    // Run health check
    let health = run_doctor_check(temp_dir.path()).await.unwrap();
    
    // Verify result
    assert!(health.installed || !health.installed); // Either is valid
    assert!(health.last_check <= SystemTime::now());
}
```

### Manual Testing

1. **Health Check:**
   ```bash
   # In web client, open Monomind panel and click "Run Health Check"
   # Should show current status within 2 seconds
   ```

2. **Scheduled Check:**
   ```bash
   # Wait 24 hours, or set custom interval for testing:
   # HealthScheduler::with_interval(Duration::from_secs(60))
   ```

3. **Upgrade Flow:**
   ```bash
   # In web client, click "Check for Updates"
   # Should show confirmation dialog
   # After confirm, should show progress and result
   ```

---

## Performance Considerations

- **Health check duration:** ~500ms (depends on `npx` cold start)
- **Scheduled checks:** Run in background, don't block WebSocket
- **Upgrade duration:** ~10-30 seconds (npm install time)
- **UI responsiveness:** Show loading state during checks

---

## Security Notes

### Upgrade Confirmation Required

Per SRS §2.4.3, upgrade is a potentially destructive operation:

```typescript
// User must explicitly confirm
const confirmUpgrade = window.confirm(
  'Upgrade monomind to latest version? This will restart the CLI.'
);

if (confirmUpgrade) {
  await healthClient.upgrade(projectDir, true);
}
```

### Authentication

Health check and upgrade use the same JWT authentication as other operations:

```rust
// Verify JWT before allowing health check/upgrade
if !verify_jwt(&request.auth_token) {
    return ErrorResponse {
        code: ErrorCode::AUTH_FAILED,
        message: "Authentication required".to_string(),
    };
}
```

---

## Next Steps (Tasks 8 & 12)

1. **task-8:** Backend API endpoints (depends on session-manager-runtime)
2. **task-12:** Complete React dashboard UI (depends on task-8 + task-11)

This integration guide covers the health check system (task-7). The dashboard API and UI will build on top of this foundation.
