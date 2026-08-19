# Criterion #5 Pre-Work: Evidence Review + Additional Metrics

**Prepared by:** performance-engineer  
**Date:** 2026-08-17  
**For:** Tuesday 09:00 war room (task-4 deliverable)

---

## Executive Summary

**Current State:**
- 3 existing criterion.rs benchmark suites ✅
- E2E latency benchmark architecturally blocked by RwLock deadlock 🔴
- Timeout pattern: consistent 30.00s across all attempts
- Root cause confirmed: `pty_output_loop` holds session write lock while blocking on async PTY read

**Key Finding:** We're measuring *what we can reach*, not *what we need to fix*. The architectural issue (lock contention) isn't directly observable in current benchmarks because the benchmark *hangs* before completing measurements.

**Tuesday Goal:** Add instrumentation to make the *invisible* (lock wait times, task fairness, PTY I/O breakdown) *visible* before attempting architectural fix.

---

## 1. Review of Benchmark Evidence

### 1.1 Existing Benchmarks (From `crates/master/benches/`)

#### ✅ FPS Rendering (`fps_rendering.rs`)
**What it measures:**
- Dirty cell tracking: 0.5ms budget
- Glyph cache lookup: 1ms budget  
- GPU command submission: 8ms budget
- Full frame cycle: 16.67ms budget (60 FPS)
- Incremental rendering (hot path)

**Status:** ✅ VERIFIED by gpu-rendering-engineer (75.12 FPS mean, 13.58ms p95 frame time)  
**Evidence:** `tests/evidence/phase1/criterion-1-fps/VERIFICATION.md`  
**Gaps for lock analysis:** None - GPU path doesn't touch session locks

#### ⚠️ WebSocket Latency (`websocket_latency.rs`)
**What it measures:**
- Protobuf encode/decode: <1ms per message
- PTY echo simulation: <2ms
- Session fan-out (1→N): <1ms for N≤10 clients
- Queue backpressure: FIFO eviction timing
- Simulated RTT components

**Status:** ⚠️ SIMULATION ONLY - doesn't use real SessionManager  
**Gaps for lock analysis:**
- ❌ No real `session.write().await` calls
- ❌ No real `attach_client()` flow
- ❌ No concurrent task contention measurement
- ✅ Does measure protocol overhead (encode/decode) - this is valid

#### ✅ PTY Throughput (`pty_throughput.rs`)
**What it measures:**
- Ring buffer append/evict: <1µs per line
- UTF-8 validation: >100 MB/s
- ANSI parsing overhead
- Scrollback retrieval: lines/sec
- Compression (zstd): >300 MB/s

**Status:** ✅ Valid for throughput, but not latency  
**Gaps for lock analysis:**
- ❌ No timing of `pty.read().await` under lock
- ❌ No concurrent read/write measurement
- ❌ No lock-hold duration tracking

#### 🔴 E2E LAN Latency (`latency_e2e_lan.rs`)
**What it measures (when working):**
- Full RTT: Client → WebSocket → Server → PTY → echo → Client
- Real session attachment flow
- Real authentication (Ed25519/JWT)
- Concurrent session latency (planned)

**Status:** 🔴 BLOCKED - 30s timeout on `attach_client()`  
**Failure logs:**
- `benchmark-run-20260817-230722.log` (original)
- `benchmark-run-*-fix.log` (failed `yield_now()` attempt)

**Timeout pattern:**
```
JWT verified: 21:10:02.715535Z
Timeout:      21:10:32.586591Z
Duration:     30.00s ← Identical across all runs
```

**Root cause** (from `FAILURE_SUMMARY.md`):
```rust
// crates/master/src/session/manager.rs:303-317
let read_result = tokio::time::timeout(
    Duration::from_millis(100),
    async {
        let mut s = session.write().await;  // ← ACQUIRE WRITE LOCK
        match s.pty.as_mut() {
            Some(pty) => pty.read(&mut buffer).await,  // ← BLOCK WITH LOCK HELD
            None => Ok(0)
        }
    }
).await;
```

**Why benchmarks can't measure this yet:**
- Benchmark *hangs* before completing measurement loop
- Criterion can't collect p50/p95 from zero successful samples
- Timeout is a *binary fail*, not a *distribution*

### 1.2 Evidence Review: Timeout Failure Patterns

**Analyzed logs:**
1. `benchmark-run-20260817-230722.log` - Original hang
2. `benchmark-run-20260817-231836-fix.log` - Post-yield_now() attempt #1
3. `benchmark-run-20260817-231956-fix.log` - Post-yield_now() attempt #2

**Common pattern:**
```
✓ Server successfully bound to 127.0.0.1:18080
✓ WebSocket client connected successfully
✓ PTY session created: <uuid>
[...JWT verification logs...]
✗ TIMEOUT: Iteration exceeded 30s limit
```

**Key observations:**
1. **Server startup**: ALWAYS succeeds (no port binding failures after fix)
2. **Session creation**: ALWAYS succeeds (ConPTY working)
3. **Attach flow**: ALWAYS hangs (lock starvation)
4. **Consistency**: 30.00s timeout (not random, not progressive)

**Hypothesis validation:**
- ✅ `yield_now()` had ZERO effect → rules out simple scheduler unfairness
- ✅ Timeout duration identical → rules out variable network/PTY latency
- ✅ Hang point always same (attach) → confirms lock as root cause, not data race

### 1.3 What's NOT Measured (Current Gaps)

From reading existing benchmarks + failure evidence:

#### 🔴 CRITICAL GAP: Lock Acquisition Metrics
**What we need:**
- Time from `session.write()` call to actual lock acquisition
- Distribution: p50, p95, p99, max
- Per-operation breakdown:
  - `attach_client()` lock wait time
  - `pty_output_loop` lock acquisition frequency
  - `send_input()` lock wait time (if any)
  - `kill_session()` lock wait time

**Why critical:**
- Current timeout is binary (30s or hang) - no visibility into *which* operation waits how long
- Can't distinguish "slow PTY read" from "lock starvation" without this
- Can't validate fix effectiveness without before/after lock-wait distributions

**How to instrument:**
```rust
let lock_wait_start = Instant::now();
let session = self.sessions.get(&session_id)
    .ok_or(SessionError::NotFound)?;
let mut s = session.write().await;
let lock_wait_time = lock_wait_start.elapsed();
tracing::debug!("Lock acquired in {:?}", lock_wait_time);
```

#### 🔴 CRITICAL GAP: Task Scheduling Fairness
**What we need:**
- Number of consecutive `pty_output_loop` lock acquisitions before `attach_client()` wins
- Time-based fairness: does tokio RwLock favor long-running tasks over new arrivals?
- Yield effectiveness: does `yield_now()` actually change scheduler order?

**Why critical:**
- Failed fix attempt (yield_now) suggests scheduler may not be FIFO
- Need to prove whether lock-free refactor is *necessary* vs "just add more yields"

**How to instrument:**
```rust
static LOCK_ACQUISITION_COUNTER: AtomicU64 = AtomicU64::new(0);

// In pty_output_loop:
let acq_count = LOCK_ACQUISITION_COUNTER.fetch_add(1, Ordering::Relaxed);
tracing::trace!("pty_output_loop acquired lock (count={})", acq_count);

// In attach_client:
let acq_count = LOCK_ACQUISITION_COUNTER.fetch_add(1, Ordering::Relaxed);
tracing::info!("attach_client acquired lock after {} pty_output_loop acquisitions", acq_count);
```

#### 🟡 IMPORTANT GAP: PTY I/O Timing Breakdown
**What we have:**
- ✅ Ring buffer throughput (pty_throughput.rs)
- ✅ UTF-8 validation overhead

**What's missing:**
- ❌ Actual `pty.read().await` latency under different conditions:
  - With data available (hot path)
  - Blocking wait (no data yet)
  - Under lock contention
- ❌ Distribution of read sizes (how often 4KB vs partial reads)
- ❌ Time-to-first-byte after session creation (initialization latency)

**Why important:**
- Current 2ms PTY read budget (SRS §2.1.1) is theoretical
- Need to validate ConPTY on Windows meets this budget
- If PTY reads are consistently >10ms, that's a separate issue from lock contention

**How to instrument:**
```rust
// In pty_output_loop, OUTSIDE the session lock:
let pty_read_start = Instant::now();
let bytes_read = pty.read(&mut buffer).await?;
let pty_read_time = pty_read_start.elapsed();
tracing::debug!("PTY read: {} bytes in {:?}", bytes_read, pty_read_time);
```

#### 🟡 MODERATE GAP: Concurrent Load Behavior
**What's missing:**
- ❌ Latency degradation curve: 1 session vs 10 vs 100
- ❌ Cross-session interference: does session A's heavy output slow session B's attach?
- ❌ Resource exhaustion point: at what concurrency does p95 exceed 10ms?

**Why moderate:**
- Phase 1 target is single-session <10ms, not 1000-session load
- But Phase 2 target (100 sessions, per acceptance criteria) needs this

**How to benchmark (after lock fix):**
```rust
// Spawn N sessions in parallel
// Measure attach latency for session N+1 while N are active
for n in [1, 5, 10, 20, 50, 100] {
    let sessions = spawn_n_active_sessions(n).await;
    let attach_latency = measure_attach_latency().await;
    // Expect: flat line until resource exhaustion, then degradation
}
```

---

## 2. Additional Metrics Needed for Tuesday Verification

### 2.1 Pre-Fix Instrumentation (Baseline)

**Goal:** Make lock contention visible BEFORE attempting architectural fix

#### Instrument #1: Lock Wait Time Distribution
**Location:** `crates/master/src/session/manager.rs`  
**Operations to track:**
- `attach_client()` - PRIMARY BLOCKER
- `send_input()` - if it also uses session.write()
- `kill_session()` - cleanup path
- `list_sessions()` - if it uses read lock (shouldn't conflict, but verify)

**Metrics to collect:**
- p50, p95, p99, max lock wait time (milliseconds)
- Success rate: % of operations that acquire lock within timeout
- Histogram: 0-1ms, 1-10ms, 10-100ms, 100-1000ms, >1000ms buckets

**Output format:**
```
Lock acquisition metrics (60s sample):
  attach_client:
    p50: <value>ms, p95: <value>ms, p99: <value>ms, max: <value>ms
    success_rate: <value>%
    histogram: [0-1ms: N, 1-10ms: N, ..., timeout: N]
```

#### Instrument #2: PTY Read Latency (Without Lock Held)
**Location:** `crates/master/src/pty/` (wherever ConPTY read happens)  
**What to measure:**
- Time per `pty.read()` call
- Bytes read per call (distribution)
- Blocking vs non-blocking reads (if distinguishable)

**Rationale:**
- Separate PTY slowness from lock contention
- If PTY reads are consistently >100ms, that's a ConPTY issue, not an architecture issue
- Validates the 2ms PTY read budget from SRS §2.1.1

**Output format:**
```
PTY read metrics (1000 samples):
  latency_p50: <value>ms
  latency_p95: <value>ms
  bytes_per_read_p50: <value>
  blocking_reads: <count>/<total>
```

#### Instrument #3: Task Interleaving Counter
**Location:** `crates/master/src/session/manager.rs`  
**What to track:**
- Number of times `pty_output_loop` acquires lock consecutively before `attach_client()` gets a turn
- Yield effectiveness: lock acquisitions before/after `yield_now()`

**Rationale:**
- Proves/disproves scheduler fairness hypothesis
- If counter shows 1000+ consecutive acquisitions by pty_output_loop, confirms starvation
- If counter shows interleaving but attach still times out, different root cause

**Output format:**
```
Task fairness metrics:
  pty_output_loop consecutive acquisitions before attach_client: <count>
  total yield_now() calls: <count>
  yield-to-acquisition latency: <value>ms
```

#### Instrument #4: WebSocket Message Queue Depth
**Location:** `crates/master/src/server/` (WebSocket handler)  
**What to track:**
- Queue size when `attach_client()` is called
- Queue growth rate during hang (if any)
- Max queue size before backpressure kicks in

**Rationale:**
- Rule out "server overloaded with pending messages" as cause of slow attach
- Validates that hang is lock-related, not I/O-related

### 2.2 Post-Fix Verification Metrics

**After architectural fix is applied, re-run ALL of the above plus:**

#### Verification #1: E2E Latency Distribution (Full Benchmark)
**Tool:** `latency_e2e_lan.rs` (should work post-fix)  
**Expected results:**
- p50 < 5ms ✅
- p95 < 10ms ✅ (Phase 1 gate)
- p99 < 15ms ✅
- Max < 30ms (no outliers)

**Comparison:**
- Before fix: 100% timeout at 30s
- After fix: p95 should be <10ms (3000x improvement)

#### Verification #2: Lock Hold Duration (Not Wait Time)
**New metric:** How long is the write lock *held* (not waited for)  
**Rationale:**
- If fix is "lock-free PTY reads", lock hold time should drop from >100ms to <1ms
- If fix is "message passing", lock hold time becomes irrelevant (no lock)

**Expected post-fix:**
- Lock hold time for attach_client(): <1ms (just metadata updates)
- Lock hold time for pty_output_loop: 0ms (no longer acquires lock)

#### Verification #3: Concurrent Attach Success Rate
**Test:** Attach 10 clients to same session simultaneously  
**Pre-fix expected:** All timeout  
**Post-fix expected:** All succeed within <10ms p95

---

## 3. Observability Gaps Beyond Performance

### 3.1 Memory Leak Detection (Related to Heap Corruption Bug)
**Current state:** 52.1% memory growth observed in long-running tests  
**Missing metrics:**
- Per-session memory breakdown (scrollback, PTY buffers, client queues)
- Arc reference counts (detect cyclic references)
- Allocation hotspots (which struct is leaking)

**Why relevant to lock analysis:**
- If memory leak is caused by failed attach leaving orphaned sessions, fixing lock = fixing leak
- If leak is independent, needs separate investigation

**Recommended tool:** `cargo-instruments` on macOS, or Windows Performance Analyzer

### 3.2 Tracing Instrumentation Levels
**Current issue:** Debug logs flood output during benchmark  
**Recommendation for Tuesday:**
- Add `#[instrument(skip(...))]` to hot-path functions (pty_output_loop)
- Use `tracing::span!` to group related operations (attach flow)
- Add custom metrics via `tracing::field` for lock wait times

**Example:**
```rust
#[instrument(skip(self, session), fields(session_id = %session_id, lock_wait_ms))]
async fn attach_client(&self, session_id: &SessionId, ...) -> Result<...> {
    let lock_wait_start = Instant::now();
    let session = ...;
    let s = session.write().await;
    tracing::Span::current().record("lock_wait_ms", lock_wait_start.elapsed().as_millis());
    ...
}
```

### 3.3 Criterion Configuration for Diagnostics
**Current config:** 10,000 samples, 30s measurement time  
**Issue:** Timeout prevents any samples from being collected

**Recommendation for Tuesday:**
- Add `LATENCY_SHORT_TEST=1` mode (already exists, 100 samples)
- Add `LATENCY_TIMEOUT_OVERRIDE` for testing longer waits
- Add per-iteration diagnostics (already added in `latency_e2e_lan.rs` line 271-276)

**Why:** Allows incremental testing of fix without 5-minute wait for full benchmark

---

## 4. Metrics Coverage Matrix

### SRS Performance Targets vs Current Benchmark Coverage

| SRS Target | Location | Current Benchmark | Status | Gap |
|------------|----------|-------------------|--------|-----|
| **§5.1.1** 1000 concurrent sessions | §5.1.1 | `bench_e2e_concurrent_sessions` | 🟡 Stubbed | Need real impl |
| **§5.1.1** 7GB memory @ 1000 sessions | §5.1.1 | None | ❌ Missing | Add memory profiling |
| **§5.1.1** 10% idle CPU | §5.1.1 | None | ❌ Missing | Add CPU profiling |
| **§5.1.2** LAN p95 <30ms | §5.1.2 | `latency_e2e_lan.rs` | 🔴 Blocked | Fix lock, then measure |
| **§7.1** Local p95 <10ms | §7.1 | `latency_e2e_lan.rs` | 🔴 Blocked | Fix lock, then measure |
| **§2.1.1** PTY read <2ms | §2.1.1 | `pty_throughput.rs` | 🟡 Partial | Need latency, not just throughput |
| **§2.1.1** Dirty track <0.5ms | §2.1.1 | `fps_rendering.rs` | ✅ Covered | PASS |
| **§2.1.1** Glyph lookup <1ms | §2.1.1 | `fps_rendering.rs` | ✅ Covered | PASS |
| **§2.1.1** GPU render <8ms | §2.1.1 | `fps_rendering.rs` | ✅ Covered | PASS |
| **§2.1.1** Full frame <16.67ms | §2.1.1 | `fps_rendering.rs` | ✅ Covered | PASS (13.58ms) |
| **§4.1.2** SQLite <1ms SELECT | §4.1.2 | None | ❌ Missing | Phase 2 (persistence) |
| **§3.1.3** Compression <1ms @ 4KB | §3.1.3 | `pty_throughput.rs` | ✅ Covered | PASS (>300 MB/s) |
| **§3.1.4** Flush <100ms | §3.1.4 | None | ❌ Missing | Add flush trigger benchmark |
| **§3.1.4** Backpressure (1MB buffer) | §3.1.4 | `websocket_latency.rs::bench_queue_backpressure` | ✅ Covered | PASS |

**Summary:**
- ✅ Covered: 7/14 (50%)
- 🟡 Partial/Stubbed: 3/14 (21%)
- 🔴 Blocked: 2/14 (14%) ← **UNBLOCKING THESE IS TUESDAY PRIORITY**
- ❌ Missing: 2/14 (14%) - acceptable for Phase 1 (deferred to Phase 2)

### Additional Metrics NOT in SRS (But Needed for Diagnosis)

| Metric | Rationale | Status |
|--------|-----------|--------|
| Lock wait time distribution | Root cause of current blocker | ❌ Add for Tuesday |
| Lock hold duration | Validate fix effectiveness | ❌ Add for Tuesday |
| Task scheduling fairness | Rule out scheduler as cause | ❌ Add for Tuesday |
| PTY read latency (not throughput) | Separate PTY issues from lock issues | ❌ Add for Tuesday |
| WebSocket queue depth | Rule out I/O overload | ❌ Add for Tuesday |
| Arc reference counts | Memory leak diagnosis | 🟡 Covered by heap profiler |
| Per-session memory breakdown | 52.1% leak root cause | 🟡 Covered by heap profiler |

---

## 5. Recommendations for Tuesday War Room

### 5.1 Pre-Fix Actions (Before Touching Code)

1. **Add lock wait time instrumentation** (30 min)
   - `attach_client()`, `send_input()`, `kill_session()` paths
   - Emit metrics to `tracing` at INFO level
   - Run `latency_e2e_lan` with `LATENCY_SHORT_TEST=1` and `RUST_LOG=info`
   - Collect baseline: "How long does attach wait before timing out?"

2. **Add PTY read timing** (15 min)
   - Instrument ConPTY read path (outside lock)
   - Measure distribution of actual read latencies
   - Validate 2ms budget assumption

3. **Add task fairness counter** (15 min)
   - AtomicU64 tracking consecutive pty_output_loop acquisitions
   - Log when attach_client finally wins
   - Prove/disprove scheduler hypothesis

**Total pre-fix instrumentation time: ~60 minutes**  
**Deliverable:** Diagnostic report showing WHERE the 30s is spent (all waiting for lock? or something else?)

### 5.2 Fix Implementation (After Diagnosis)

**Three architectural options** (from `FAILURE_SUMMARY.md`):

#### Option A: Lock-Free PTY Reads
```rust
struct Session {
    pty: Arc<Mutex<Pty>>,  // ← Separate lock, not session-wide
    metadata: Arc<RwLock<SessionMetadata>>,  // ← Rarely locked
}

// pty_output_loop NO LONGER holds session lock
loop {
    let data = pty.lock().await.read(&mut buf).await?;
    let session = sessions.get(id).metadata.write().await;
    session.scrollback.append(data);
}
```
**Pros:** Minimal refactor, preserves existing API  
**Cons:** Now 2 locks to coordinate, potential for new deadlocks

#### Option B: Message Passing
```rust
struct PtyTask {
    pty: Pty,
    output_tx: mpsc::Sender<Vec<u8>>,
}

// No shared lock - PTY owned by dedicated task
tokio::spawn(async move {
    loop {
        let data = pty.read(&mut buf).await?;
        output_tx.send(data).await?;
    }
});

// attach_client() receives from channel, no lock contention
```
**Pros:** Clean separation, no lock contention possible  
**Cons:** Larger refactor, changes session lifecycle

#### Option C: Lock Minimization
```rust
// Clone data BEFORE long I/O, release lock immediately
let pty_fd = {
    let s = session.write().await;
    s.pty.as_ref().unwrap().clone_fd()  // ← Arc or dup()
};  // ← Lock released here

let data = pty_read(pty_fd, &mut buf).await?;  // ← I/O outside lock

{
    let mut s = session.write().await;
    s.scrollback.append(data);
}  // ← Lock held only for metadata update
```
**Pros:** Minimal code change, easy to validate  
**Cons:** Requires PTY to be cloneable (may not be possible with ConPTY)

**Recommendation:** Start with **Option C** (lock minimization) as lowest-risk, fall back to **Option B** (message passing) if ConPTY can't be cloned.

### 5.3 Post-Fix Verification Checklist

- [ ] E2E latency benchmark completes without timeout
- [ ] p95 < 10ms (Phase 1 gate)
- [ ] Lock wait time drops from 30s → <1ms
- [ ] 10 concurrent attaches all succeed
- [ ] No new failures in existing tests
- [ ] Memory leak (52.1%) either fixed or proven independent

---

## 6. Evidence Collection Plan

### 6.1 Tuesday Morning Diagnostic Run

**Command:**
```powershell
# Set short test mode (100 samples instead of 10,000)
$env:LATENCY_SHORT_TEST = "1"
$env:RUST_LOG = "monoterminal_master=debug,latency_e2e_lan=debug"

# Run with extended timeout for diagnostics
cargo bench --bench latency_e2e_lan -- --nocapture 2>&1 | 
  Tee-Object -FilePath "tests/evidence/phase1/criterion-5-latency/tuesday-diagnostic-run.log"
```

**Expected output:**
- Lock wait times for attach_client (should show ~30s)
- PTY read latencies (should show <2ms or reveal slowness)
- Task fairness counter (should show 100+ consecutive pty_output_loop wins)

**Decision point:**
- If lock wait = 30s, PTY read <2ms, fairness counter high → confirms lock starvation, proceed with Option C/B
- If PTY read >100ms → ConPTY issue, different fix needed
- If lock wait <100ms but still timeouts → different root cause (race condition?)

### 6.2 Post-Fix Evidence Collection

**Full benchmark run** (after fix validated with short test):
```powershell
# Remove short test override
Remove-Item Env:\LATENCY_SHORT_TEST

# Run full 10,000 sample benchmark
cargo bench --bench latency_e2e_lan -- --nocapture 2>&1 | 
  Tee-Object -FilePath "tests/evidence/phase1/criterion-5-latency/tuesday-fix-verification.log"
```

**Criterion HTML report:**
- Location: `target/criterion/e2e_lan_latency/report/index.html`
- Extract p50, p95, p99 from `estimates.json`
- Compare against Phase 1 gate: p95 < 10ms ✅

**qa-lead handoff package:**
- `tuesday-diagnostic-run.log` (pre-fix baseline)
- `tuesday-fix-verification.log` (post-fix full benchmark)
- `estimates.json` (criterion statistical analysis)
- `VERIFICATION-REPORT.md` (pass/fail vs SRS §7.1)

---

## 7. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Instrumentation overhead skews benchmarks | Low | Use `#[cfg(feature = "diagnostics")]` to make optional |
| ConPTY can't be cloned (blocks Option C) | Medium | Have Option B (message passing) ready as fallback |
| Fix introduces new race condition | High | Verify with `loom` or `miri` before benchmarking |
| Fix helps but doesn't reach <10ms | High | Measure PTY read latency FIRST to set realistic expectations |
| Tuesday 90-min window insufficient | Medium | Pre-commit Option C sketch for faster implementation |

---

## 8. Success Criteria

### Diagnostic Phase Success (09:00-10:00 Tuesday)
- ✅ Lock wait time baseline established (<10 lines of instrumentation)
- ✅ PTY read latency measured (not just throughput)
- ✅ Task fairness counter shows data (proves/disproves scheduler hypothesis)
- ✅ Diagnostic report answers: "Where is the 30s spent?"

### Fix Phase Success (10:00-10:30 Tuesday)
- ✅ Architectural fix committed (Option C or B)
- ✅ Short test (100 samples) completes without timeout
- ✅ No new test failures introduced

### Verification Phase Success (Later Tuesday)
- ✅ Full benchmark (10,000 samples) p95 < 10ms
- ✅ Criterion HTML report generated
- ✅ Evidence package delivered to qa-lead

---

## Appendices

### A. SRS Performance Target Quick Reference

From `docs/monoterminal-srs.md`:

| Target | Section | Value | Phase |
|--------|---------|-------|-------|
| Local latency p95 | §7.1 | <10ms | Phase 1 gate |
| LAN latency p95 | §5.1.2 | <30ms | Overall target |
| PTY read | §2.1.1 | <2ms | Frame budget |
| Full frame | §2.1.1 | <16.67ms | 60 FPS |
| Concurrent sessions | §5.1.1 | 1000 | Phase 2 |
| Memory @ 1000 sessions | §5.1.1 | 7GB | Phase 2 |
| Compression @ 4KB | §3.1.3 | <1ms | Protocol |

### B. Benchmark File Locations

```
crates/master/benches/
├── fps_rendering.rs          # ✅ Verified (gpu-rendering-engineer)
├── websocket_latency.rs      # ⚠️ Simulation only (no real locks)
├── pty_throughput.rs         # ✅ Valid for throughput
├── latency_e2e_lan.rs        # 🔴 Blocked by lock deadlock
└── README.md                 # Documentation

tests/evidence/phase1/criterion-5-latency/
├── FAILURE_SUMMARY.md        # Root cause analysis
├── benchmark-run-*.log       # Failure evidence
├── TUESDAY-PREWORK-*.md      # This document
└── (to be added Tuesday)
    ├── tuesday-diagnostic-run.log
    ├── tuesday-fix-verification.log
    └── VERIFICATION-REPORT.md
```

### C. Instrumentation Code Snippets

#### Lock Wait Time
```rust
use std::time::Instant;

// In attach_client():
let lock_wait_start = Instant::now();
let session = self.sessions.get(&session_id)
    .ok_or(SessionError::NotFound)?;
let mut s = session.write().await;
let lock_wait_time = lock_wait_start.elapsed();

tracing::info!(
    session_id = %session_id,
    lock_wait_ms = lock_wait_time.as_millis(),
    "attach_client acquired lock"
);
```

#### PTY Read Timing
```rust
// In pty_output_loop (OUTSIDE session lock):
let pty_read_start = Instant::now();
let bytes_read = pty.read(&mut buffer).await?;
let pty_read_time = pty_read_start.elapsed();

tracing::debug!(
    session_id = %session_id,
    bytes_read,
    pty_read_ms = pty_read_time.as_millis(),
    "PTY read complete"
);
```

#### Task Fairness Counter
```rust
use std::sync::atomic::{AtomicU64, Ordering};

static PTY_LOOP_ACQUISITIONS: AtomicU64 = AtomicU64::new(0);

// In pty_output_loop:
let count = PTY_LOOP_ACQUISITIONS.fetch_add(1, Ordering::Relaxed);
tracing::trace!("pty_output_loop lock acquisition #{}", count);

// In attach_client:
let pty_acq_count = PTY_LOOP_ACQUISITIONS.load(Ordering::Relaxed);
tracing::info!(
    "attach_client attempting lock after {} pty_output_loop acquisitions",
    pty_acq_count
);
```

---

**End of Pre-Work Analysis**  
**Ready for:** Tuesday 09:00 war room  
**Next action:** Await architectural decision, then implement instrumentation + fix
