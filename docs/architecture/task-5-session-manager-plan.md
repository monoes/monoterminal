# Task-5: Session Manager Runtime - Implementation Plan

**Owner:** rust-backend-lead  
**Status:** READY - Awaiting Dependencies (task-4)  
**SRS References:** §2.1.3 (Session Management), §2.1.5 (Monomind Integration)

---

## Current State Analysis

### Existing Implementation
Located in `crates/master/src/session/manager.rs`

**What's Already There:**
- ✅ CREATE operation (`create_session()` lines 44-103)
- ✅ ATTACH operation (`attach_client()` lines 105-126)
- ✅ DETACH operation (`detach_client()` lines 128-145)
- ✅ Input handling (`send_input()` lines 147-165)
- ✅ Resize handling (`resize_session()` lines 167-194)
- ✅ Basic KILL operation (`kill_session()` lines 196-221)
- ✅ Session snapshot for late-joiner sync (`snapshot()` in session.rs)

**What's Missing:**
- ❌ Client broadcast channel setup (integration point from task-4)
- ❌ Monomind detection on session create (SRS §2.1.5)
- ❌ Graceful shutdown with SIGHUP → wait → SIGKILL escalation (line 218 TODO)
- ❌ Multi-client output fan-out (depends on task-4 completion)

---

## Implementation Requirements

### 1. Client List Management & Late-Joiner Sync

**Current State:** ✅ Mostly Complete

**Session Snapshot Flow (Already Implemented):**
1. Client calls `attach_client(session_id, client_id)`
2. Session adds client to `clients: Vec<ClientId>`
3. Returns `SessionSnapshot` with:
   - 10k lines scrollback (~1MB per SRS §2.1.4)
   - Session metadata (id, dimensions, working_dir, shell_type)

**Integration with task-4:**
- Attach creates `broadcast::Receiver` for new client
- Client handler (task-3) consumes receiver for live output
- Snapshot provides historical output for sync

**No Changes Needed** - Already correct per SRS §2.1.4.

### 2. Monomind Detection Integration (SRS §2.1.5)

**Detection Strategy:**
Walk upward from session `working_dir` to find `.monomind/` directory.

**Implementation:**

```rust
// In create_session(), after session creation:
let monomind_detected = detect_monomind(&working_dir);
if monomind_detected {
    tracing::info!("Monomind detected for session {}", id);
    // Call monomind-bridge SESSION_START hook
    if let Err(e) = monomind_bridge::notify_session_start(id, &working_dir).await {
        tracing::warn!("Monomind SESSION_START hook failed: {}", e);
        // Non-fatal - continue session creation
    }
}

fn detect_monomind(cwd: &Path) -> bool {
    let mut current = cwd;
    loop {
        if current.join(".monomind").is_dir() {
            return true;
        }
        match current.parent() {
            Some(p) => current = p,
            None => return false,
        }
    }
}
```

**Monomind-Bridge Integration:**
- Add `monomind-bridge` crate dependency
- Call `monomind_bridge::notify_session_start(session_id, working_dir)` async
- Non-blocking, non-fatal (log warning on error)
- Set `session.monomind_detected = true` flag

**SRS Reference:** §2.1.5 - Detection walks upward from cwd

### 3. Graceful Shutdown Enhancement

**Current Implementation (lines 196-221):**
```rust
// Sets state = Terminated
// Sleeps 100ms for cleanup
// Relies on Drop for PTY cleanup
// TODO comment for proper escalation
```

**SRS Requirement (§2.1.3):**
1. Receive termination signal
2. Send SIGHUP to session process group
3. Wait up to 10s for clean exit
4. Force kill with SIGKILL if still alive
5. Flush resources (SQLite WAL in Phase 2, file descriptors)

**Windows-Specific Implementation:**

```rust
pub async fn kill_session(&self, session_id: SessionId) -> Result<()> {
    let mut sessions = self.sessions.write().await;
    let session_arc = sessions
        .remove(&session_id)
        .ok_or(SessionError::NotFound(session_id))?;

    tracing::info!("Session {} terminating (graceful shutdown)", session_id);

    let shell_pid = {
        let mut session = session_arc.write().await;
        session.state = SessionState::Terminated;
        session.shell_pid
    };

    // Windows: Use taskkill /PID for graceful termination
    // Equivalent to SIGTERM on Unix
    let graceful = tokio::process::Command::new("taskkill")
        .args(&["/PID", &shell_pid.to_string(), "/T"]) // /T = kill tree
        .output()
        .await;

    if let Ok(output) = graceful {
        if output.status.success() {
            tracing::info!("Session {} gracefully terminated", session_id);
        }
    }

    // Wait up to 10s for process exit
    let mut checks = 0;
    while checks < 100 {
        let alive = tokio::process::Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", shell_pid)])
            .output()
            .await
            .ok()
            .map(|o| o.stdout.len() > 0)
            .unwrap_or(false);

        if !alive {
            tracing::info!("Session {} process exited cleanly", session_id);
            return Ok(());
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        checks += 1;
    }

    // Force kill after 10s timeout
    tracing::warn!("Session {} force killing after timeout", session_id);
    let _ = tokio::process::Command::new("taskkill")
        .args(&["/PID", &shell_pid.to_string(), "/F", "/T"]) // /F = force
        .output()
        .await;

    Ok(())
}
```

**Note:** ConPTY cleanup happens via `ClosePseudoConsole()` in Drop impl (task-1 responsibility).

### 4. CREATE/ATTACH/DETACH/KILL Operations

**State Transitions (SRS §2.1.3):**
```
  CREATE ──> RUNNING ──┬──> DETACHED ──> REATTACHED ──┐
                       │                              │
                       └──> TERMINATED ───────────────┘
```

**Phase 1 Simplification:**
- No DETACHED state (deferred to Phase 2)
- Sessions terminate when last client disconnects OR explicit KILL
- Simplified: `CREATE → RUNNING → TERMINATED`

**Current Implementation Status:**

| Operation | Status | Lines | Notes |
|-----------|--------|-------|-------|
| CREATE | ✅ Complete | 44-103 | Spawns PTY, creates session, starts output loop |
| ATTACH | ✅ Complete | 105-126 | Adds client, returns snapshot |
| DETACH | ✅ Complete | 128-145 | Removes client from list |
| KILL | ⚠️ Needs Enhancement | 196-221 | Add graceful shutdown per above |

**Integration Points:**
- CREATE → Monomind detection (add)
- ATTACH → Broadcast receiver creation (task-4 integration)
- KILL → Graceful shutdown (enhance)

---

## Architecture Integration

### Session Lifecycle
```
Client → WebSocket Connect (task-3)
           ↓
       Authentication (auth module)
           ↓
   ┌──────┴──────┐
   ATTACH    CREATE (task-5)
     ↓          ↓
   Existing  New PTY (task-1)
   Session      ↓
     ↓      Monomind Detection (task-5)
     ↓          ↓
   Snapshot   Session Start
     ↓          ↓
   ┌──────┴──────┐
   │  PTY Output Loop (task-4)
   │  Fan-out to clients
   └──────┬──────┘
          ↓
    Client Disconnect → DETACH
          ↓
    Last Client Gone → KILL (optional auto-terminate)
```

---

## Code Changes Checklist

### File: `crates/master/src/session/manager.rs`

**Monomind Integration:**
- [ ] Add `detect_monomind(cwd: &Path) -> bool` function
- [ ] Call monomind detection in `create_session()` after PTY spawn
- [ ] Add `monomind_bridge` dependency and async hook call
- [ ] Set `session.monomind_detected` flag

**Graceful Shutdown:**
- [ ] Replace `kill_session()` implementation with Windows-specific version
- [ ] Use `taskkill /PID /T` for graceful termination (SIGTERM equivalent)
- [ ] Implement 10s wait loop checking process exit
- [ ] Force kill with `taskkill /F` after timeout
- [ ] Add tracing for shutdown phases

**Broadcast Integration (from task-4):**
- [ ] Update `attach_client()` to create broadcast receiver
- [ ] Pass receiver to client handler (task-3 integration point)

### File: `crates/master/Cargo.toml`

- [ ] Add dependency: `monomind-bridge = { path = "../monomind-bridge" }`

---

## Testing Strategy (Deferred to task-15/task-16)

**Unit Tests:**
- Monomind detection (with/without `.monomind/` dir)
- Session state transitions
- Client attach/detach list management

**Integration Tests:**
- Full session lifecycle: CREATE → ATTACH → I/O → DETACH → KILL
- Multi-client attach to same session
- Late-joiner scrollback sync (snapshot correctness)
- Graceful shutdown timeout behavior

**E2E Tests (task-14):**
- Web client → create session → attach → send input → receive output
- Monomind dashboard displays detected session

---

## Performance Targets (SRS §5.1.1)

| Metric | Target | Verification |
|--------|--------|--------------|
| **Session Create Latency** | <100ms | Time from request to first I/O ready |
| **Attach Latency** | <50ms | Snapshot serialization + client add |
| **Memory per Session** | 7MB | 4KB buffer + 1MB scrollback + metadata |
| **Max Concurrent Sessions** | 1000 (Phase 2+) | Phase 1: Single session optimized |

---

## Dependencies

**Requires from task-4 (PTY Async I/O):**
- Working output fan-out to N clients
- Broadcast channel infrastructure
- Backpressure handling

**Provides to:**
- **task-6** (GPU rendering): Session metadata for UI display
- **task-8** (Monomind dashboard): Session state via monomind-bridge hooks
- **task-14** (E2E tests): Complete session lifecycle

---

## Open Questions for Dependencies

**For monomind-integration-engineer:**
- Is `monomind_bridge::notify_session_start()` async or blocking?
- What's the expected latency (<100ms budget)?
- Error handling - should session creation fail if hook errors?

**For rust-engineer-protocol (task-3):**
- When should client_id be generated (before or during attach)?
- How to pass broadcast receiver to WebSocket handler?

---

## References

- SRS §2.1.3: Session Management (lifecycle, daemon mode)
- SRS §2.1.4: Networking Layer (late-joiner sync)
- SRS §2.1.5: Monomind Integration (detection, hooks)
- Architecture: `docs/architecture/phase1-overview.md` §2

---

**Status:** READY to implement when task-4 completes.  
**Estimated Duration:** 1 day (per task graph)  
**Next Task:** Unblocks task-6 (GPU rendering) and task-8 (Monomind dashboard API)
