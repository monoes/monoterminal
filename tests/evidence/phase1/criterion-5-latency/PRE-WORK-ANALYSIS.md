# Pre-Work Analysis: Criterion #5 Latency Hang
**Prepared by:** qa-lead  
**Date:** 2026-08-17 23:30  
**For:** Tuesday 09:00 War Room

---

## Executive Summary

**Hang Pattern:** Architectural deadlock occurring after JWT verification, before AttachResponse transmission. Both original failure and yield_now() fix attempt exhibit **identical behavior**, confirming surface-level task scheduling fixes are insufficient.

**Root Cause Hypothesis:** Session write lock held during synchronous I/O operation (PTY attach path), blocking AttachResponse send. Likely RwLock contention between handler task and background session I/O task.

**Evidence Quality:** High - Defensive timeout captured diagnostic state, both runs show deterministic hang at same code location.

---

## Failure Log Analysis

### Original Failure (benchmark-run-20260817-230722.log)

**Timeline:**
```
21:10:02.715 - AttachRequest received (session: abc25220-e904-42e9-b17c-e9fa2ff6dcc5)
21:10:02.715 - JWT verified for AttachRequest
21:10:32.586 - TIMEOUT (30.01s elapsed)
```

**Key Observations:**
1. ✅ Server startup successful (127.0.0.1:18080)
2. ✅ WebSocket handshake completed
3. ✅ PTY session created (pid=26232)
4. ✅ JWT verification passed
5. ❌ **NO AttachResponse sent** (no log line, client timeout)
6. ❌ **NO subsequent handler activity** (handler task appears blocked)

**Last Known State:** Line 1646 - "JWT verified for AttachRequest"

---

### Failed Fix Attempt (benchmark-run-20260817-231956-fix.log)

**Approach:** Added `tokio::task::yield_now()` after JWT verification to prevent RwLock starvation.

**Timeline:**
```
21:22:39.044 - AttachRequest received (session: f1cd32e1-9fd4-413f-a71a-487f55f58819)
21:22:39.045 - JWT verified for AttachRequest
21:23:08.906 - TIMEOUT (30.00s elapsed)
```

**Result:** **IDENTICAL HANG PATTERN**

**Conclusion:** yield_now() ineffective → This is NOT a task starvation issue → Architectural deadlock confirmed.

---

## Root Cause Hypothesis

### Suspected Deadlock Scenario

**Hypothesis:** Session write lock held during synchronous PTY attach I/O, blocking AttachResponse send.

**Suspected Code Path:**
```
AttachRequest handler:
  1. Verify JWT ✅ (completes)
  2. Acquire session write lock 🔒
  3. Perform PTY attach I/O (synchronous read?) ⏳ HANGS HERE
  4. Send AttachResponse ❌ (never reached)
```

**Why yield_now() failed:**
- yield_now() only helps if lock holder is starved by scheduler
- If lock holder is blocked on I/O (PTY read waiting for data), yielding doesn't help
- Need to move I/O **outside** lock critical section, not just yield

### Evidence Supporting Hypothesis

1. **Hang location**: After JWT (CPU-bound) ✅, before AttachResponse (network I/O) ❌
2. **Deterministic**: 100% reproducible, same location every time
3. **No timeout variance**: Both hangs ~30.0s (defensive timeout triggers, not natural resolution)
4. **Handler silence**: No error logs, no panic → suggests await on lock/I/O

---

## Architectural Questions for Tuesday War Room

### Critical Path Analysis

1. **Why does PTY read require session write lock?**
   - Is this a data race concern (PTY state mutation)?
   - Is this an implementation accident (over-locking)?
   - Could we use session read lock + interior mutability for PTY state?

2. **What is the lock acquisition order?**
   - AttachRequest handler: `session_manager.read()` → `session.write()`?
   - Background PTY task: `session.write()` → I/O?
   - **Risk:** Lock inversion deadlock if handler holds session_manager lock while waiting for session lock

3. **Is PTY attach I/O synchronous?**
   - Does `attach()` call `pty.read()` or `pty.write()` under lock?
   - Are we using `tokio::fs` async I/O or std::io blocking I/O?

---

## Design Options for Tuesday Discussion

### Option A: Lock-Free PTY I/O (Message Passing)

**Pattern:**
```rust
// Handler does NOT hold session lock during I/O
let (tx, rx) = oneshot::channel();
session.pty_command_tx.send(AttachCommand { response_tx: tx }).await?;
let attach_result = rx.await?; // I/O happens in background task, no lock
// Now build AttachResponse with result
```

**Trade-offs:**
- ✅ No deadlock risk (no lock held during I/O)
- ✅ Clean separation of concerns
- ❌ More complex (new message passing layer)
- ❌ Latency overhead (extra hop through channel)

### Option B: Read/Write Lock Split

**Pattern:**
```rust
// Separate PTY state (needs write lock) from session metadata (read lock)
let session = session_manager.get_session(id).await?; // Read lock
let pty_handle = session.pty.clone(); // Arc<Mutex<Pty>>
drop(session); // Release session lock
let output = pty_handle.lock().await.read().await?; // I/O outside session lock
```

**Trade-offs:**
- ✅ Minimal changes to existing architecture
- ✅ Fast path (no extra hops)
- ❌ Still has lock (Arc<Mutex>), just narrower scope
- ❌ Potential for deadlock if we're not careful with lock order

### Option C: Lock-Minimized (Clone Data, Release, I/O)

**Pattern:**
```rust
let pty_fd = {
    let session = session_manager.get_session(id).await?;
    session.pty_fd.clone() // Cheap clone (Arc or raw fd)
}; // Lock released here
let output = read_pty(pty_fd).await?; // I/O outside lock
```

**Trade-offs:**
- ✅ Simple refactor (minimal code changes)
- ✅ No new architecture (just move I/O)
- ❌ Assumes PTY can be safely accessed outside lock (need to verify)
- ❌ May still have races if PTY state is complex

---

## Verification Test Spec (Post-Fix)

### Unit Test: Attach Flow Under Load

**Purpose:** Verify AttachRequest → AttachResponse path completes under concurrent load.

**Test Design:**
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_attach_no_deadlock() {
    let server = setup_test_server().await;
    let session_id = create_test_session(&server).await;
    
    // Spawn 10 concurrent attach requests
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let sid = session_id.clone();
            tokio::spawn(async move {
                let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
                let start = Instant::now();
                client.attach(&sid, JWT_TOKEN).await?; // Must complete
                let elapsed = start.elapsed();
                assert!(elapsed < Duration::from_secs(1), "Attach took {}ms (>1000ms)", elapsed.as_millis());
                Ok::<_, Error>(elapsed)
            })
        })
        .collect();
    
    // All must complete within 5s total
    let results = timeout(Duration::from_secs(5), join_all(handles)).await
        .expect("Concurrent attach deadlock detected");
    
    // Verify all succeeded
    for r in results {
        r.expect("Task panicked").expect("Attach failed");
    }
}
```

**Success Criteria:**
- ✅ All 10 attach requests complete
- ✅ No timeout (< 5s total)
- ✅ Individual attach latency < 1s each

### Integration Test: Attach Regression Guard

**Purpose:** Prevent reintroduction of I/O-under-lock pattern.

**Test Design:**
```rust
#[tokio::test]
async fn test_attach_no_blocking_io_under_lock() {
    // This test uses tokio::time::pause() to detect blocking I/O
    tokio::time::pause();
    
    let server = setup_test_server().await;
    let session_id = create_test_session(&server).await;
    let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
    
    // If attach does blocking I/O under lock, this will hang
    // (paused time doesn't advance unless we explicitly advance it)
    let attach_future = client.attach(&session_id, JWT_TOKEN);
    
    // Advance time by 100ms (should be enough for CPU-bound work)
    tokio::time::advance(Duration::from_millis(100)).await;
    
    // Attach should complete (no blocking I/O to stall it)
    let result = timeout(Duration::from_millis(10), attach_future).await;
    assert!(result.is_ok(), "Attach appears to do blocking I/O under lock");
}
```

**Success Criteria:**
- ✅ Test completes without timeout
- ✅ Attach completes in simulated time (no real I/O blocking)

---

## Acceptance Criteria: Criterion #5 Re-Verification

### Benchmark Must Pass

**Command:**
```bash
cargo bench --bench latency_e2e_lan
```

**Success Criteria:**
1. ✅ **No timeouts** (all iterations complete within 30s defensive timeout)
2. ✅ **p95 latency < 10ms** (SRS §4.4.1 requirement)
3. ✅ **Mean latency < 5ms** (reasonable for LAN loopback)
4. ✅ **No outlier spikes > 50ms** (indicates no sporadic blocking)
5. ✅ **Clean logs** (no ERROR/WARN in attach path)

### Evidence Requirements

**Logs:**
- `tests/evidence/phase1/criterion-5-latency/benchmark-run-<timestamp>-PASS.log`
- Must show:
  - AttachRequest received
  - JWT verified
  - **NEW:** AttachResponse sent (this line was missing in failures)
  - Benchmark completion with timing stats

**Metrics:**
```
Expected output:
e2e_lan_latency/real_master_rtt_loopback
                        time:   [X.XXX ms X.XXX ms X.XXX ms]
                        change: [-XX.X% -XX.X% -XX.X%] (p = 0.00 < 0.05)
                        Performance has improved.

Key metrics:
- Mean:  < 5ms
- p95:   < 10ms
- p99:   < 15ms
- Max:   < 50ms
```

---

## Pre-Work Deliverables Status

- ✅ **Benchmark log review** (this document, §2)
- ✅ **Root cause hypothesis** (this document, §3)
- ✅ **Architectural questions** (this document, §4)
- ✅ **Design options** (this document, §5)
- ✅ **Verification test spec** (this document, §6)
- ✅ **Acceptance criteria** (this document, §7)

---

## Recommended War Room Agenda (Tuesday 09:00)

**Phase 1: Root Cause Confirmation (30 min)**
- rust-backend-lead: Present lock acquisition path diagram
- Team: Answer architectural questions (§4)
- Verify hypothesis: Is PTY I/O happening under session write lock?

**Phase 2: Design Selection (30 min)**
- rust-backend-lead: Present 2-3 design options (may overlap with §5)
- Team: Trade-off discussion (complexity vs. safety vs. performance)
- Decision: Select one design for implementation

**Phase 3: Implementation (90 min)**
- rust-backend-lead: Implement chosen design
- qa-lead: Prepare verification tests (§6) in parallel
- Incremental validation: Unit test → integration test → benchmark

**Phase 4: Verification (30 min)**
- qa-lead: Run verification test suite (§6)
- qa-lead: Run criterion #5 benchmark (§7)
- Decision: ✅ Criterion #5 VERIFIED or ❌ Defer/iterate

**Total: 3 hours (09:00-12:00)**

---

## Risk Assessment

### High Confidence Areas
- ✅ Hang location identified (post-JWT, pre-AttachResponse)
- ✅ Defensive timeout working (captured diagnostic state)
- ✅ Evidence quality high (deterministic, reproducible)

### Low Confidence Areas
- ⚠️ Exact lock acquisition order (need code review)
- ⚠️ PTY I/O synchronicity (async vs. blocking)
- ⚠️ Safe refactor scope (how much code must change?)

### Mitigation
- rust-backend-lead pre-work: Architectural diagram + lock order trace
- Tuesday war room: Live code walkthrough to confirm hypothesis
- Incremental testing: Unit → integration → benchmark (catch regressions early)

---

## Appendix: Defensive Timeout Effectiveness

**Original Design (qa-lead recommendation):**
> "Add defensive 30s timeout to benchmark iterations to prevent infinite hang."

**Effectiveness Assessment:**
- ✅ **Prevented infinite hang** (both runs aborted at 30s)
- ✅ **Captured diagnostic state** (iteration count, elapsed time, server address)
- ✅ **Clear error message** (recommended actions visible)
- ✅ **Evidence preserved** (logs written before panic)

**Lessons Learned:**
- Defensive timeouts are critical for CI/CD (prevent pipeline stalls)
- Timeout duration should be >>expected latency (30s is 3000x the 10ms target)
- Diagnostic dumps should include last known state (we got logs up to JWT verification)

**Recommendation:** Keep defensive timeout in place post-fix as regression guard.
