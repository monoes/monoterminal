# Session/PTY Locking Architecture Analysis
**Prepared for:** Tuesday 2026-08-18 09:00 War Room  
**Author:** rust-backend-lead  
**Purpose:** Root cause analysis + design options for Criterion #5 deadlock

---

## Current Architecture: Lock Acquisition Paths

### Path 1: `pty_output_loop` (Background Task)

```
┌─────────────────────────────────────────────────────────────┐
│ pty_output_loop (spawned at session creation)              │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ Loop iteration starts         │
          └───────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ session.read().await          │
          │ (check if terminated)         │
          └───────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ timeout(100ms, async {        │
          │   session.write().await  ◄────┼─── WRITE LOCK ACQUIRED
          │   pty.read(&mut buf).await    │     HELD DURING I/O ❌
          │ })                            │
          └───────────────────────────────┘
                          │
                  ┌───────┴────────┐
                  │                │
            Data received      Timeout (no data)
                  │                │
                  ▼                ▼
          ┌─────────────┐  ┌──────────────┐
          │ Flush data  │  │ Continue     │
          │ (if needed) │  │ waiting      │
          └─────────────┘  └──────────────┘
                  │                │
                  └────────┬───────┘
                           │
                           ▼
                  Loop continues (goto top)
```

**Critical issue:** Write lock held for up to 100ms per iteration while blocked on `pty.read()`.

### Path 2: `attach_client()` (User Request Handler)

```
┌─────────────────────────────────────────────────────────────┐
│ WebSocket handler receives AttachRequest                   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ Verify JWT authentication     │
          └───────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ sessions.read().await         │
          │ (get session from HashMap)    │
          └───────────────────────────────┘
                          │
                          ▼
          ┌───────────────────────────────┐
          │ session.write().await  ◄──────┼─── BLOCKS HERE ❌
          │ (attach client)               │     Waiting for pty_output_loop
          │                               │     to release write lock
          └───────────────────────────────┘
                          │
                    (never reached)
                          ▼
          ┌───────────────────────────────┐
          │ Return scrollback snapshot    │
          └───────────────────────────────┘
```

**Deadlock scenario:** `attach_client()` waits indefinitely for write lock held by `pty_output_loop`.

---

## Deadlock Race Condition Timeline

```
Time    pty_output_loop                      attach_client()
─────────────────────────────────────────────────────────────
t=0     Session created, loop spawned        
t=1     Acquire write lock                   
t=2     pty.read() → shell prompt received   
t=3     Release lock, loop continues         
t=4     Acquire write lock                   
t=5     pty.read() → BLOCKS (no data)        AttachRequest arrives
t=6     (blocked, lock held)                 JWT verified ✓
t=7     (blocked, lock held)                 sessions.read() ✓
t=8     (blocked, lock held)                 session.write() → BLOCKS ❌
t=9     (blocked, lock held)                 (waiting...)
...
t=100   100ms timeout fires                  (still waiting...)
t=101   Release lock                         (scheduler delay)
t=102   Loop continues                       
t=103   Acquire write lock AGAIN ◄───────────┼─── Race condition!
                                             │   attach_client() loses
t=104   pty.read() → BLOCKS                  (still blocked)
...
t=30s   (still blocked)                      30s timeout → PANIC
```

**Why yield_now() failed:** tokio::RwLock has no FIFO fairness. Even with yields, `pty_output_loop` wins race and re-acquires lock before `attach_client()` can compete.

---

## Design Options for Tuesday Discussion

### Option A: Extract PTY I/O from Session Lock ⭐ **RECOMMENDED**

**Concept:** PTY ownership separate from session lock scope

```rust
// Current (broken):
struct Session {
    pty: Box<dyn PtyBackend>,  // ← Inside RwLock
    scrollback: Scrollback,
    clients: HashMap<ClientId, Sender>,
}

// Proposed:
struct Session {
    // NO PTY HERE
    scrollback: Scrollback,
    clients: HashMap<ClientId, Sender>,
}

struct SessionContainer {
    session: Arc<RwLock<Session>>,      // ← Separate locks
    pty: Arc<Mutex<Box<dyn PtyBackend>>>,  // ← Independent
}
```

**pty_output_loop changes:**
```rust
async fn pty_output_loop(
    session: Arc<RwLock<Session>>,
    pty: Arc<Mutex<Box<dyn PtyBackend>>>,  // ← New parameter
) {
    loop {
        // Read PTY WITHOUT session lock
        let data = {
            let mut pty_guard = pty.lock().await;
            timeout(100ms, pty_guard.read(&mut buf)).await
        };  // ← PTY lock released

        // Only acquire session lock to update state
        if let Ok(Ok(n)) = data {
            let mut s = session.write().await;
            s.scrollback.push(data);
            s.broadcast_to_clients(data);
        }  // ← Session lock released
    }
}
```

**Pros:**
- ✅ Minimal lock hold time (only during state updates, not I/O)
- ✅ Separate contention domains (PTY I/O vs. session metadata)
- ✅ attach_client() never blocks on PTY I/O

**Cons:**
- ⚠️ API change: SessionManager must track both Arc's
- ⚠️ Moderate complexity: two separate synchronization primitives
- ⚠️ `Mutex` vs `RwLock` for PTY (single writer, so Mutex is fine)

**Safety concerns:**
- PTY lifecycle: ensure PTY isn't dropped while reads in flight
- Task coordination: both tasks must handle termination cleanly

**Estimated effort:** 2-3 hours (test included)

---

### Option B: Message-Passing Pattern

**Concept:** Dedicated PTY task owns the backend, sends output via channel

```rust
// PTY owner task
async fn pty_owner_task(
    mut pty: Box<dyn PtyBackend>,
    output_tx: mpsc::Sender<Vec<u8>>,
    mut input_rx: mpsc::Receiver<Vec<u8>>,
) {
    loop {
        tokio::select! {
            // Read from PTY, send to session
            result = pty.read(&mut buf) => {
                if let Ok(data) = result {
                    output_tx.send(data).await;
                }
            }
            // Write input to PTY
            Some(input) = input_rx.recv() => {
                pty.write(&input).await;
            }
        }
    }
}

// Session no longer owns PTY
struct Session {
    pty_input_tx: mpsc::Sender<Vec<u8>>,  // ← Send input to PTY task
    scrollback: Scrollback,
    clients: HashMap<ClientId, Sender>,
}

// pty_output_loop receives via channel
async fn pty_output_loop(
    session: Arc<RwLock<Session>>,
    mut pty_output_rx: mpsc::Receiver<Vec<u8>>,  // ← From PTY task
) {
    while let Some(data) = pty_output_rx.recv().await {
        let mut s = session.write().await;
        s.scrollback.push(data.clone());
        s.broadcast_to_clients(data);
    }
}
```

**Pros:**
- ✅ Complete decoupling (PTY task has exclusive ownership)
- ✅ No shared locks between I/O and session
- ✅ Easier to reason about: single owner per resource

**Cons:**
- ⚠️ More complex: 3 communicating tasks (PTY, output loop, input handler)
- ⚠️ Channel overhead (data copying unless Arc<Bytes> used)
- ⚠️ Larger refactor (PTY interface changes, task lifecycle coordination)

**Safety concerns:**
- Backpressure: what if output channel fills up? (bounded channel + drop policy)
- Termination: coordinated shutdown of 3 tasks

**Estimated effort:** 4-6 hours (larger refactor)

---

### Option C: Lock Minimization (Clone-Process-Update)

**Concept:** Hold lock only long enough to clone data, release, process, re-acquire to commit

```rust
async fn pty_output_loop(session: Arc<RwLock<Session>>) {
    loop {
        // Acquire lock, clone PTY handle reference
        let pty_handle: /* some Arc or cloneable ref */ = {
            let s = session.read().await;
            s.pty_handle.clone()  // ← Shallow clone
        };  // ← Lock released

        // Read from PTY WITHOUT session lock
        let result = timeout(100ms, pty_handle.read(&mut buf)).await;

        // Re-acquire lock to commit result
        if let Ok(Ok(data)) = result {
            let mut s = session.write().await;
            s.scrollback.push(data.clone());
            s.broadcast_to_clients(data);
        }
    }
}
```

**Challenge:** ConPtyBackend is NOT cloneable (owns Windows HANDLEs)

**Workaround:** Wrap PTY internals in Arc<> to enable cloning

```rust
struct ConPtyBackend {
    inner: Arc<ConPtyInner>,  // ← Shareable
}

struct ConPtyInner {
    output_reader: Mutex<BufReader<AsyncPipeReader>>,  // ← Interior mutability
    ...
}
```

**Pros:**
- ✅ Minimal API change (Session still owns PTY)
- ✅ Lock hold time reduced

**Cons:**
- ⚠️ Requires interior mutability (Arc<Mutex<>> inside PTY)
- ⚠️ Complicates PTY implementation
- ⚠️ Two lock acquisitions per iteration (read to clone, write to commit)

**Safety concerns:**
- Mutation through shared reference (requires careful Mutex use)
- Still potential for contention on PTY's internal Mutex

**Estimated effort:** 3-4 hours (PTY refactor + testing)

---

## Trade-Off Matrix

| Criteria                  | Option A (Extract) | Option B (Message) | Option C (Clone) |
|---------------------------|--------------------|--------------------|------------------|
| **Lock contention**       | ✅ Eliminated      | ✅ Eliminated      | ⚠️ Reduced       |
| **Complexity**            | ⚠️ Moderate        | ❌ High            | ⚠️ Moderate      |
| **API stability**         | ⚠️ Breaks          | ❌ Major break     | ✅ Minimal       |
| **Performance overhead**  | ✅ Minimal         | ⚠️ Channel copy    | ✅ Minimal       |
| **Safety verification**   | ⚠️ Lifecycle       | ⚠️ Backpressure    | ⚠️ Mutation      |
| **Implementation time**   | ⚠️ 2-3h            | ❌ 4-6h            | ⚠️ 3-4h          |
| **Rollback risk**         | ✅ Low             | ❌ High            | ⚠️ Medium        |

**Recommendation:** **Option A (Extract PTY I/O)**

**Rationale:**
- Cleanest separation of concerns (I/O vs. metadata)
- Moderate complexity (manageable in 1 session)
- Clear rollback path (revert to single Arc if issues)
- Performance optimal (no channel overhead)

---

## Safety Analysis: Option A Deep Dive

### Lifecycle Guarantees Needed

1. **PTY must outlive all readers**
   - Solution: `SessionContainer` holds both Arc's, dropped together
   - Verification: Unit test that drops session → PTY cleanup confirmed

2. **No use-after-free on PTY during termination**
   - Current risk: `pty_output_loop` reads while `terminate_pty()` consumes it
   - Solution: `Arc<Mutex<Option<Box<dyn PtyBackend>>>>`
     - `terminate_pty()` sets to None
     - `pty_output_loop` checks for None, exits gracefully

3. **Task coordination on session cleanup**
   - Current: AbortOnDrop handles for both tasks
   - Unchanged: Session drop still aborts both tasks

### Concurrency Invariants

1. **PTY Mutex fairness**
   - tokio::Mutex is fair (FIFO for lock requests)
   - No starvation risk (unlike RwLock write-heavy scenario)

2. **Session RwLock contention**
   - Reduced: only brief holds for scrollback/broadcast
   - Multiple readers OK (clients can attach concurrently)

### Edge Cases to Test

1. **Attach during PTY read**
   - Expected: attach_client() proceeds immediately (separate locks)
   - Test: Benchmark should pass with <10ms p95

2. **Attach during session update (scrollback write)**
   - Expected: brief block (<1ms, just scrollback push)
   - Test: Concurrent attach requests under load

3. **PTY termination during read**
   - Expected: pty_output_loop sees None, exits cleanly
   - Test: Call terminate_pty() while loop is blocked in read

4. **Session drop during PTY read**
   - Expected: AbortOnDrop kills pty_output_loop, PTY Arc dropped
   - Test: Drop session, verify no leaked tasks or handles

---

## Verification Protocol Checklist

**For qa-lead to expand on Tuesday:**

### Unit Tests
- [ ] PTY extracted separately, sessions.get() returns SessionContainer
- [ ] pty_output_loop reads from Arc<Mutex<PTY>> independently
- [ ] attach_client() doesn't block on PTY lock
- [ ] terminate_pty() sets PTY to None, loop exits

### Integration Tests
- [ ] AttachRequest completes <10ms under load
- [ ] Concurrent attaches (10 clients) don't interfere
- [ ] PTY output during attach doesn't block response

### Benchmark
- [ ] Criterion #5 passes (p95 <10ms)
- [ ] No timeout failures after 10,000 iterations
- [ ] Load test: 100 concurrent sessions, 10 attaches/sec

### Safety
- [ ] ASAN: no use-after-free
- [ ] TSAN: no data races (if available on Windows)
- [ ] Manual review: all PTY access through Mutex

---

## Implementation Plan (Tuesday 09:00-12:00)

### Phase 1: Design confirmation (09:00-09:30)
- Review this document
- Confirm Option A or pivot to B/C with justification
- Assign: Me (implementation), qa-lead (test spec), performance-engineer (verification)

### Phase 2: Implementation (09:30-11:00)
- 09:30-10:00: Refactor SessionContainer, extract PTY
- 10:00-10:30: Update pty_output_loop, attach_client, terminate_pty
- 10:30-11:00: Unit tests (4 edge cases above)

### Phase 3: Verification (11:00-11:45)
- 11:00-11:15: Integration test
- 11:15-11:30: Benchmark re-run
- 11:30-11:45: Load test + safety checks

### Phase 4: Decision gate (11:45-12:00)
- ✅ Pass: Merge, update gate status to 5/7
- ⚠️ Issues: Rollback plan + reschedule for Wednesday
- ❌ Fail: Escalate to eng-director for Phase 1 scope adjustment

---

**Prepared by:** rust-backend-lead  
**For review by:** qa-lead, eng-director, performance-engineer  
**Session:** Tuesday 2026-08-18 09:00
