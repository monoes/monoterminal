# Criterion #5 RwLock Deadlock - Emergency Fix Attempt FAILED

**Date:** 2026-08-17 Monday evening (23:15-23:35)
**Attempted by:** rust-backend-lead
**Outcome:** FAILED - identical 30s timeout persists
**Status:** ABORTED per 60-min window limit

## Root Cause (Confirmed)

**Location:** `crates/master/src/session/manager.rs:306-309`
**Issue:** `pty_output_loop` holds session write lock while blocking on `pty.read().await`

```rust
// Line 303-317: The problematic pattern
let read_result = tokio::time::timeout(
    Duration::from_millis(100),
    async {
        let mut s = session.write().await;  // ← ACQUIRE WRITE LOCK
        match s.pty.as_mut() {
            Some(pty) => pty.read(&mut buffer).await,  // ← BLOCK WITH LOCK HELD
            None => Ok(0)
        }
    }  // ← Lock dropped here, but immediately re-acquired next iteration
).await;
```

**Failure scenario:**
1. PTY session created, `pty_output_loop` spawned
2. Loop acquires write lock, reads initial shell prompt
3. Loop continues, acquires write lock, blocks waiting for more PTY output
4. `attach_client()` tries to acquire write lock → DEADLOCKS (30s timeout)

## Fix Attempted: `tokio::task::yield_now()`

**Hypothesis:** tokio RwLock lacks FIFO fairness, pty_output_loop monopolizes lock

**Implementation:**
- Added `yield_now().await` after timeout (line 377)
- Added `yield_now().await` at end of each iteration (line 382)

**Expected outcome:** Yields would give `attach_client()` opportunity to acquire lock

**Actual outcome:** FAILED - identical 30s timeout

### Evidence

**Original failure:**
- Log: `benchmark-run-20260817-230722.log`
- JWT verified: 21:10:02.715535Z
- Timeout: 21:10:32.586591Z
- Duration: 30.00s

**Fix attempt failure:**
- Log: `benchmark-run-*-fix.log` (timestamp varies)
- JWT verified: 21:22:39.045086Z
- Timeout: 21:23:08.906480Z
- Duration: 30.00s (identical)

## Why Yield Failed

tokio::RwLock does not guarantee FIFO fairness for write lock requests. Even with explicit yields:

1. `pty_output_loop` acquires lock
2. Timeout fires, lock released
3. `yield_now()` called
4. Scheduler resumes `pty_output_loop` (or it wins the race)
5. Lock re-acquired by `pty_output_loop` before `attach_client()` competes
6. Cycle repeats → `attach_client()` starves indefinitely

## Correct Fix Required

**Architectural refactor** - PTY I/O cannot hold session write lock

### Option A: Lock-free PTY reads
- Extract PTY into separate Arc<Mutex<>> per session
- `pty_output_loop` reads without session lock
- Only acquire session lock for scrollback/broadcast updates

### Option B: Message-passing pattern
- PTY owned by dedicated task
- Session receives PTY output via channel
- No shared lock between PTY I/O and session state

### Option C: Lock minimization
- Acquire lock, clone necessary data, release lock
- Perform PTY I/O outside lock scope
- Re-acquire lock to commit results

**Estimated complexity:** 2-3 hours (exceeds 60-min emergency window)
**Regression risk:** HIGH (core session/PTY boundary)

## Abort Decision

**Criteria met:**
- ✅ Fix complexity exceeds authorized window
- ✅ High regression risk for late-night architectural change
- ✅ qa-lead's original recommendation (Tuesday fresh eyes) validated

**Deferred to:** Tuesday 2026-08-18 09:00 war room

## Tuesday Prep

**Rust-backend-lead (me) pre-work:**
1. Architectural diagram: lock acquisition paths
2. Draft 2-3 design options with trade-offs
3. Safety analysis for each option

**QA-lead pre-work:**
1. Verification test spec for chosen design
2. Review both benchmark failure logs
3. Additional diagnostic instrumentation if needed

**War room agenda:** See qa-lead's structured 90-min session plan

## Gate Impact

**Current:** 4/7 criteria verified
**Blocking:** Criterion #5 (session attachment latency)
**Phase 1 gate:** DOES NOT PASS (below ADR-006 threshold)

## Lessons Learned

✅ Abort clause worked as designed
✅ 60-min window prevented protracted late-night debugging
✅ Quick recognition of failure better than persisting with wrong approach
✅ qa-lead's process discipline prevented shipping non-fix

---

*Evidence preserved for Tuesday 09:00 architectural review*
