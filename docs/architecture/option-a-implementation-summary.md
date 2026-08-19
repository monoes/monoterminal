# Option A Implementation Summary
**Tuesday 2026-08-18 Phase 2 Implementation**  
**Time:** 09:30-11:00  
**Status:** COMPLETE

---

## What Was Implemented

### Core Pattern: Lock Separation

**Before (Broken):**
```rust
// Session owned PTY inside RwLock
struct Session {
    pty: Option<Box<dyn PtyBackend>>,  // ← Inside RwLock
    scrollback: Scrollback,
    clients: Vec<...>,
}

// pty_output_loop held RwLock write during I/O
async fn pty_output_loop(session: Arc<RwLock<Session>>) {
    let mut s = session.write().await;  // ← Write lock acquired
    pty.read(&mut buf).await;  // ← Blocks WITH LOCK HELD ❌
}

// attach_client blocked on same RwLock
pub async fn attach_client(...) {
    let mut s = session.write().await;  // ← DEADLOCKS waiting for pty_output_loop ❌
}
```

**After (Fixed - Option A):**
```rust
// SessionContainer: Separate Arc's for session and PTY
#[derive(Clone)]
pub struct SessionContainer {
    session: Arc<RwLock<Session>>,                        // ← Metadata lock
    pty: Arc<Mutex<Option<Box<dyn PtyBackend>>>>,  // ← I/O lock (SEPARATE)
}

// Session: PTY field removed
pub struct Session {
    // NO PTY HERE
    scrollback: Scrollback,
    clients: Vec<...>,
    shell_pid: u32,  // Cached from PTY
}

// pty_output_loop: Locks PTY independently
async fn pty_output_loop(
    session: Arc<RwLock<Session>>,
    pty: Arc<Mutex<Option<Box<dyn PtyBackend>>>>,
) {
    // Lock PTY Mutex for I/O (independent of session)
    let mut pty_guard = pty.lock().await;
    pty_backend.read(&mut buf).await;  // ← I/O WITHOUT session lock ✅
    // PTY lock released

    // Brief session lock for update only
    let mut s = session.write().await;
    s.scrollback.push(data);
    s.broadcast_to_clients(data);
    // Session lock released
}

// attach_client: Locks session only (no PTY contention)
pub async fn attach_client(...) {
    let mut s = container.session.write().await;  // ← NO BLOCKING on PTY I/O ✅
    s.attach_client(client_id, output_tx);
    Ok(s.snapshot())
}
```

---

## Files Modified

### 1. `crates/master/src/session/session.rs`

**Added:**
- `SessionContainer` struct (lines 67-79)
- `SessionContainer::new()` (lines 193-223)
- `SessionContainer::terminate_pty()` (lines 225-254)

**Modified:**
- Session struct: Removed `pty` field (line 88 deleted)
- Session::new(): Changed signature (no longer takes PTY parameter)

### 2. `crates/master/src/session/mod.rs`

**Modified:**
- Export `SessionContainer` (line 9)

### 3. `crates/master/src/session/manager.rs`

**Modified:**
- `SessionManager.sessions`: Type changed from `HashMap<SessionId, Arc<RwLock<Session>>>` to `HashMap<SessionId, SessionContainer>` (line 21)

**Methods updated:**
1. **create_session()** (lines 84-154):
   - Creates `SessionContainer` instead of `Session`
   - Spawns `pty_output_loop` with both session Arc and pty Arc

2. **pty_output_loop()** (lines 293-390) - **THE CORE FIX:**
   - Signature: Added `pty: Arc<Mutex<Option<Box<dyn PtyBackend>>>>` parameter
   - PTY read: Locks PTY Mutex independently (line 316-327)
   - Session update: Brief lock only for scrollback/broadcast (lines 354-357)

3. **attach_client()** (lines 161-183):
   - Uses `container.session.write()` instead of direct session
   - No PTY lock acquisition → No deadlock

4. **send_input()** (lines 207-230):
   - Locks PTY Mutex for write operation
   - Brief session lock for touch() only

5. **resize_session()** (lines 234-270):
   - Locks PTY Mutex for resize operation
   - Brief session lock for dimension update

6. **kill_session()** (lines 274-291):
   - Calls `container.terminate_pty()` instead of `session.terminate_pty()`

7. **detach_client()** (lines 188-204):
   - Uses `container.session.write()` instead of direct session

---

## Key Design Decisions

### 1. Mutex vs RwLock for PTY

**Decision:** Use `Mutex<Option<Box<dyn PtyBackend>>>`

**Rationale:**
- PTY has single writer (pty_output_loop)
- No concurrent reads needed
- Mutex is simpler and slightly faster for exclusive access
- RwLock's read-sharing benefit doesn't apply

### 2. Option<Box<dyn PtyBackend>> Pattern

**Decision:** Wrap PTY in `Option<>`

**Rationale:**
- Allows graceful termination (set to None)
- pty_output_loop checks for None and exits cleanly
- Prevents use-after-free during termination

### 3. SessionContainer vs Separate Manager Fields

**Decision:** Create `SessionContainer` struct

**Rationale:**
- Encapsulates session + PTY lifecycle together
- Clear ownership: container owns both Arc's
- Drop semantics: both Arc's dropped together (prevents leak)
- API clarity: one container per session

### 4. Task Spawning Order

**Decision:** Store container BEFORE spawning tasks (create_session line 108)

**Rationale:**
- Prevents deadlock during initialization
- Tasks can safely access stored container
- Matches Monday's existing fix pattern

---

## Safety Guarantees

### 1. PTY Lifecycle

**Guarantee:** PTY cannot outlive session or vice versa

**Mechanism:**
- `SessionContainer` owns both `session: Arc<RwLock<Session>>` and `pty: Arc<Mutex<Option<...>>>`
- Drop order: container dropped → both Arc's dropped
- Reference counting prevents premature drop

### 2. Termination Safety

**Guarantee:** pty_output_loop exits gracefully when PTY terminates

**Mechanism:**
- `terminate_pty()` sets `pty` to `None` (line 234)
- `pty_output_loop` checks `pty_guard.as_mut()` (line 317)
- `None` → returns `Ok(0)` → loop breaks (line 320)

### 3. Task Coordination

**Guarantee:** Background tasks aborted when session drops

**Mechanism:**
- Session still owns `_output_task: AbortOnDrop` (line 109)
- Session still owns `_monomind_task: AbortOnDrop` (line 113)
- Drop semantics unchanged from Monday baseline

### 4. No Use-After-Free

**Guarantee:** Cannot access PTY after termination

**Mechanism:**
- `Option<Box<dyn PtyBackend>>` pattern
- `as_mut()` returns `None` after termination
- Methods return `BrokenPipe` error when PTY is `None`

---

## Verification Checklist

**Build:**
- ✅ Compiles clean (0 errors, 24 warnings)
- ✅ No unsafe code added
- ✅ No breaking API changes to external interfaces

**Lock Independence:**
- ✅ pty_output_loop: Only locks PTY Mutex during I/O
- ✅ attach_client: Only locks Session RwLock
- ✅ send_input: Locks PTY Mutex, then session (brief)
- ✅ resize_session: Locks PTY Mutex, then session (brief)

**Safety:**
- ✅ SessionContainer lifecycle guarantees both Arc's
- ✅ Option<> pattern for graceful termination
- ✅ AbortOnDrop preserved for task cleanup

---

## Expected Test Results

### Test 8 (Criterion #5 - Critical Path)

**Before (Monday baseline):**
```
21:22:39.045086Z DEBUG JWT verified for AttachRequest
[30 seconds of silence]
21:23:08.906480Z ERROR TIMEOUT: Iteration exceeded 30s limit
```

**After (Option A):**
```
[timestamp] DEBUG JWT verified for AttachRequest
[timestamp] DEBUG Processing AttachRequest: session_id=...
[timestamp] INFO Client {id} attached to session {id}  ← THIS LINE NOW APPEARS
[timestamp] Criterion #5 p95: <10ms  ← PASSES
```

**Key indicator:** Log line "Client {id} attached to session {id}" proves attach_client completed (no deadlock).

---

## Rollback Plan

**If verification fails:**

1. **Revert commits:**
   ```bash
   git reset --hard <monday-baseline-commit>
   ```

2. **Evidence:**
   - All Option A changes in single commit
   - Clean revert to Monday baseline
   - No partial state

3. **Defer to Wednesday:**
   - Reassess design
   - Consider Option B or C
   - Additional verification testing

---

## Implementation Time

**Actual:** 09:30-10:15 (45 minutes)
**Estimated:** 2-3 hours (1.5h-2.5h buffer remaining)
**Status:** AHEAD OF SCHEDULE ✅

---

**Implemented by:** rust-backend-lead  
**Reviewed for handoff by:** qa-lead (11:00)  
**Final verification:** Phase 3 (11:00-11:45)
