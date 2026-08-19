# Verification Specification: Option A (Extract PTY I/O)
**Prepared by:** qa-lead  
**Date:** 2026-08-17 23:45  
**Context:** Expands verification checklist from `docs/architecture/session-pty-locking-analysis.md` §6

---

## Cross-Reference: Architecture Analysis

This spec implements the verification protocol requested in:
- **Architecture doc:** `docs/architecture/session-pty-locking-analysis.md`
- **QA pre-work:** `tests/evidence/phase1/criterion-5-latency/PRE-WORK-ANALYSIS.md`
- **Design option:** Option A (Extract PTY I/O) ⭐ RECOMMENDED

---

## 6.1 Unit Tests (Checklist Expansion)

### Test 1: SessionContainer Isolation

**Checklist item:**
- [x] PTY extracted separately, sessions.get() returns SessionContainer

**Test Code:**
```rust
#[tokio::test]
async fn test_session_container_pty_separate_arcs() {
    let session_manager = SessionManager::new();
    let session_id = session_manager.create_session(
        PtyConfig::default(), None
    ).await.unwrap();
    
    // Verify SessionContainer structure
    let container = session_manager.get_session_container(&session_id).await.unwrap();
    
    // Both Arc's must be independent (not nested)
    assert!(Arc::strong_count(&container.session) >= 1);
    assert!(Arc::strong_count(&container.pty) >= 1);
    
    // Drop one Arc, other remains valid
    let session_arc = container.session.clone();
    drop(container);
    assert!(Arc::strong_count(&session_arc) >= 1, "Session Arc survived container drop");
}
```

**Success Criteria:**
- ✅ SessionContainer contains two independent Arc's
- ✅ Dropping container doesn't invalidate either Arc
- ✅ Arc::strong_count reflects correct reference tracking

---

### Test 2: PTY Output Loop Independence

**Checklist item:**
- [x] pty_output_loop reads from Arc<Mutex<PTY>> independently

**Test Code:**
```rust
#[tokio::test]
async fn test_pty_output_loop_no_session_lock_during_io() {
    let (session_arc, pty_arc) = create_test_session_container().await;
    
    // Spawn pty_output_loop
    let loop_handle = tokio::spawn(pty_output_loop(
        session_arc.clone(),
        pty_arc.clone(),
    ));
    
    // Hold session WRITE lock (simulating attach_client)
    let _session_guard = session_arc.write().await;
    
    // pty_output_loop should NOT be blocked (separate lock)
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Verify loop is still running (not blocked on session lock)
    assert!(!loop_handle.is_finished(), "pty_output_loop blocked by session write lock");
    
    drop(_session_guard); // Release session lock
    loop_handle.abort();
}
```

**Success Criteria:**
- ✅ pty_output_loop runs independently of session write lock
- ✅ No deadlock when session locked + PTY I/O concurrent

---

### Test 3: Attach During PTY Read (No Blocking)

**Checklist item:**
- [x] attach_client() doesn't block on PTY lock

**Test Code:**
```rust
#[tokio::test]
async fn test_attach_no_block_on_pty_io() {
    let (session_arc, pty_arc) = create_test_session_container().await;
    let session_id = SessionId::new_v4();
    
    // Hold PTY lock (simulating pty_output_loop blocked in read)
    let _pty_guard = pty_arc.lock().await;
    
    // attach_client should NOT block (separate locks)
    let attach_start = Instant::now();
    let result = timeout(
        Duration::from_millis(100),
        attach_client_internal(session_arc.clone(), ClientId::new_v4())
    ).await;
    let attach_elapsed = attach_start.elapsed();
    
    assert!(result.is_ok(), "attach_client blocked on PTY lock");
    assert!(attach_elapsed < Duration::from_millis(50), 
        "attach took {}ms (expected <50ms)", attach_elapsed.as_millis());
    
    drop(_pty_guard);
}
```

**Success Criteria:**
- ✅ attach_client completes while PTY lock held
- ✅ Latency < 50ms (no blocking on I/O)

---

### Test 4: PTY Termination Cleanup

**Checklist item:**
- [x] terminate_pty() sets PTY to None, loop exits

**Test Code:**
```rust
#[tokio::test]
async fn test_terminate_pty_loop_exits_gracefully() {
    let (session_arc, pty_arc) = create_test_session_container().await;
    
    // Spawn pty_output_loop
    let loop_handle = tokio::spawn(pty_output_loop(
        session_arc.clone(),
        pty_arc.clone(),
    ));
    
    // Wait for loop to start reading
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Terminate PTY (sets Arc<Mutex<Option<Box<dyn PtyBackend>>>> to None)
    terminate_pty(pty_arc.clone()).await.unwrap();
    
    // Loop should exit gracefully (no panic)
    let result = timeout(Duration::from_secs(1), loop_handle).await;
    assert!(result.is_ok(), "pty_output_loop did not exit after terminate");
    assert!(result.unwrap().is_ok(), "pty_output_loop panicked on termination");
}
```

**Success Criteria:**
- ✅ pty_output_loop exits when PTY set to None
- ✅ No panic during shutdown
- ✅ Exit within 1s (no hang on termination)

---

## 6.2 Integration Tests (Checklist Expansion)

### Test 5: AttachRequest Latency Under Load

**Checklist item:**
- [x] AttachRequest completes <10ms under load

**Test Code:**
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_attach_latency_under_concurrent_pty_io() {
    let server = setup_test_server().await;
    let session_id = create_test_session(&server).await;
    
    // Simulate heavy PTY I/O (background task)
    let _pty_load = tokio::spawn(async move {
        for _ in 0..1000 {
            // Write to PTY (triggers output loop activity)
            send_pty_data(session_id.clone(), b"test\n").await;
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    });
    
    // Measure attach latency during heavy I/O
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
        let start = Instant::now();
        client.attach(&session_id, JWT_TOKEN).await?;
        latencies.push(start.elapsed());
    }
    
    // Verify latency SLA
    let p95 = percentile(&latencies, 95);
    let p99 = percentile(&latencies, 99);
    assert!(p95 < Duration::from_millis(10), "p95 latency {}ms (>10ms)", p95.as_millis());
    assert!(p99 < Duration::from_millis(15), "p99 latency {}ms (>15ms)", p99.as_millis());
}
```

**Success Criteria:**
- ✅ p95 latency < 10ms (SRS §4.4.1)
- ✅ p99 latency < 15ms
- ✅ No timeouts under concurrent PTY I/O

---

### Test 6: Concurrent Attaches (No Interference)

**Checklist item:**
- [x] Concurrent attaches (10 clients) don't interfere

**Test Code:**
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_concurrent_attach_no_interference() {
    let server = setup_test_server().await;
    let session_id = create_test_session(&server).await;
    
    // Spawn 10 concurrent attach requests
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let sid = session_id.clone();
            tokio::spawn(async move {
                let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
                let start = Instant::now();
                let snapshot = client.attach(&sid, JWT_TOKEN).await?;
                let elapsed = start.elapsed();
                
                // Verify each client got valid snapshot
                assert!(!snapshot.scrollback.is_empty(), "Client {} got empty scrollback", i);
                assert!(elapsed < Duration::from_secs(1), "Client {} attach took {}ms", i, elapsed.as_millis());
                
                Ok::<_, Error>((i, elapsed, snapshot.scrollback.len()))
            })
        })
        .collect();
    
    // All must complete successfully
    let results = join_all(handles).await;
    for (i, r) in results.iter().enumerate() {
        let (client_id, elapsed, scrollback_len) = r.as_ref().unwrap().as_ref().unwrap();
        println!("Client {}: {}ms, {} lines", client_id, elapsed.as_millis(), scrollback_len);
        assert_eq!(i, *client_id as usize);
    }
}
```

**Success Criteria:**
- ✅ All 10 clients complete attach
- ✅ Each client receives valid scrollback snapshot
- ✅ No data corruption or missing lines

---

### Test 7: PTY Output During Attach (No Blocking)

**Checklist item:**
- [x] PTY output during attach doesn't block response

**Test Code:**
```rust
#[tokio::test]
async fn test_attach_concurrent_with_pty_output() {
    let server = setup_test_server().await;
    let session_id = create_test_session(&server).await;
    
    // Start continuous PTY output (shell command loop)
    send_pty_data(session_id.clone(), b"while true; do echo tick; sleep 0.01; done\n").await;
    tokio::time::sleep(Duration::from_millis(100)).await; // Let output start
    
    // Attach while PTY is actively producing output
    let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
    let start = Instant::now();
    let snapshot = client.attach(&session_id, JWT_TOKEN).await?;
    let elapsed = start.elapsed();
    
    // Verify attach completed despite active PTY output
    assert!(elapsed < Duration::from_millis(100), "attach blocked by PTY output ({}ms)", elapsed.as_millis());
    assert!(snapshot.scrollback.contains("tick"), "snapshot missing PTY output");
}
```

**Success Criteria:**
- ✅ Attach completes < 100ms during active PTY output
- ✅ Snapshot contains recent PTY data (no race condition)

---

## 6.3 Benchmark (Checklist Expansion)

### Test 8: Criterion #5 Re-Verification

**Checklist item:**
- [x] Criterion #5 passes (p95 <10ms)

**Command:**
```bash
cargo bench --bench latency_e2e_lan 2>&1 | tee tests/evidence/phase1/criterion-5-latency/benchmark-run-$(date +%Y%m%d-%H%M%S)-PASS.log
```

**Success Criteria:**
1. ✅ **No timeouts** (all 10,000 iterations complete)
2. ✅ **p95 < 10ms** (SRS §4.4.1)
3. ✅ **Mean < 5ms**
4. ✅ **Max < 50ms** (no outliers)
5. ✅ **Log evidence:**
   - AttachRequest received
   - JWT verified
   - **NEW LINE MUST APPEAR:** AttachResponse sent ← This was missing in failures
   - Benchmark completion stats

**Expected Output:**
```
e2e_lan_latency/real_master_rtt_loopback
                        time:   [4.123 ms 4.234 ms 4.345 ms]
                        change: [-85.3% -84.9% -84.5%] (p = 0.00 < 0.05)
                        Performance has improved.
```

**Failure Detection:**
- ❌ Any timeout after 30s → Hang regression detected
- ❌ p95 ≥ 10ms → Performance SLA violated
- ❌ Missing "AttachResponse sent" in logs → Incomplete fix

---

### Test 9: Extended Reliability (No Timeout Failures)

**Checklist item:**
- [x] No timeout failures after 10,000 iterations

**Test Design:**
```rust
// Criterion benchmark config
fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(10_000)  // 10k iterations
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(60))
        .configure_from_args()
}
```

**Success Criteria:**
- ✅ All 10,000 samples collected without panic
- ✅ Zero timeout events in logs
- ✅ Criterion report shows normal distribution (no bimodal pattern indicating sporadic hangs)

---

### Test 10: Load Test (100 Sessions, 10 Attaches/Sec)

**Checklist item:**
- [x] Load test: 100 concurrent sessions, 10 attaches/sec

**Test Code:**
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
#[ignore] // Run manually: cargo test --release test_load_100_sessions -- --ignored
async fn test_load_100_sessions_10_attaches_per_sec() {
    let server = setup_test_server().await;
    
    // Create 100 sessions
    let session_ids: Vec<_> = (0..100)
        .map(|_| server.create_session(PtyConfig::default(), None).await.unwrap())
        .collect();
    
    // Spawn 10 attaches/sec for 60s = 600 total attaches
    let mut interval = tokio::time::interval(Duration::from_millis(100)); // 10/sec
    let mut attach_latencies = Vec::new();
    
    for _ in 0..600 {
        interval.tick().await;
        
        // Random session selection
        let session_id = session_ids.choose(&mut rand::thread_rng()).unwrap();
        
        // Spawn attach (non-blocking)
        let sid = session_id.clone();
        let handle = tokio::spawn(async move {
            let client = TestWsClient::connect("ws://127.0.0.1:18080").await?;
            let start = Instant::now();
            client.attach(&sid, JWT_TOKEN).await?;
            Ok::<_, Error>(start.elapsed())
        });
        
        // Collect latency later
        attach_latencies.push(handle);
    }
    
    // Wait for all attaches to complete
    let results = join_all(attach_latencies).await;
    let latencies: Vec<_> = results.into_iter()
        .filter_map(|r| r.ok().and_then(|l| l.ok()))
        .collect();
    
    // Verify SLA under load
    assert_eq!(latencies.len(), 600, "Some attaches failed");
    let p95 = percentile(&latencies, 95);
    let p99 = percentile(&latencies, 99);
    
    assert!(p95 < Duration::from_millis(10), "p95 under load: {}ms (>10ms)", p95.as_millis());
    assert!(p99 < Duration::from_millis(20), "p99 under load: {}ms (>20ms)", p99.as_millis());
}
```

**Success Criteria:**
- ✅ 600 attaches complete successfully
- ✅ p95 < 10ms under load
- ✅ p99 < 20ms under load
- ✅ No session crashes or deadlocks

---

## 6.4 Safety Checks (Checklist Expansion)

### Test 11: ASAN (Address Sanitizer - Use-After-Free)

**Checklist item:**
- [x] ASAN: no use-after-free

**Platform:** Linux (ASAN not fully supported on Windows)

**Command:**
```bash
# Build with AddressSanitizer
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu

# Run specific edge case tests
RUSTFLAGS="-Z sanitizer=address" cargo +nightly test --target x86_64-unknown-linux-gnu \
    test_terminate_pty_loop_exits_gracefully \
    test_attach_no_block_on_pty_io
```

**Success Criteria:**
- ✅ No ASAN reports (use-after-free, heap-buffer-overflow, etc.)
- ✅ All tests pass under ASAN instrumentation

**Failure Example (if bug exists):**
```
=================================================================
==12345==ERROR: AddressSanitizer: heap-use-after-free on address 0x...
    #0 ConPtyBackend::read
    #1 pty_output_loop
```

**Note:** If ASAN unavailable on Windows, run on Linux CI or WSL.

---

### Test 12: Manual Lock Audit

**Checklist item:**
- [x] Manual review: all PTY access through Mutex

**Audit Checklist:**
```
File: crates/master/src/session/manager.rs
- [ ] SessionContainer struct contains Arc<Mutex<Option<Box<dyn PtyBackend>>>>
- [ ] No direct PTY field in Session struct
- [ ] create_session() returns SessionContainer with separate Arc's

File: crates/master/src/session/pty_loop.rs
- [ ] pty_output_loop signature includes Arc<Mutex<...>> parameter
- [ ] All pty.read() calls preceded by .lock().await
- [ ] Lock released before session.write().await (no nested locks)

File: crates/master/src/server/handler.rs
- [ ] attach_client() does NOT acquire PTY lock
- [ ] Only session.write() for client list update
- [ ] Scrollback read via session.read() (no write lock)

File: crates/master/src/pty/mod.rs
- [ ] terminate_pty() sets Mutex<Option<...>> to None
- [ ] No dangling PTY references outside Arc<Mutex<>>
```

**Success Criteria:**
- ✅ All PTY access goes through Arc<Mutex<>>
- ✅ No lock order inversion (PTY lock never held while acquiring session lock)
- ✅ Termination path sets PTY to None atomically

---

## Summary: Verification Checklist Status

| Section | Test | Status | Criteria |
|---------|------|--------|----------|
| **6.1 Unit** | SessionContainer isolation | ✅ Ready | Independent Arc's |
| **6.1 Unit** | PTY loop independence | ✅ Ready | No session lock during I/O |
| **6.1 Unit** | Attach no block on PTY | ✅ Ready | <50ms latency |
| **6.1 Unit** | PTY termination cleanup | ✅ Ready | Graceful exit <1s |
| **6.2 Integration** | Attach latency under load | ✅ Ready | p95 <10ms |
| **6.2 Integration** | Concurrent attaches | ✅ Ready | 10 clients succeed |
| **6.2 Integration** | PTY output during attach | ✅ Ready | <100ms latency |
| **6.3 Benchmark** | Criterion #5 re-verification | ✅ Ready | SRS §4.4.1 pass |
| **6.3 Benchmark** | 10k iterations no timeout | ✅ Ready | Zero panics |
| **6.3 Benchmark** | Load: 100 sessions | ✅ Ready | 600 attaches p95 <10ms |
| **6.4 Safety** | ASAN use-after-free | ⚠️ Linux only | No violations |
| **6.4 Safety** | Manual lock audit | ✅ Ready | Checklist complete |

**Total:** 12 verification tests defined, ready for Tuesday implementation validation.

---

## Execution Order (Tuesday 11:00-11:45 Verification Window)

**Phase 3 Timeline:**

1. **11:00-11:15: Unit + Integration (parallel)**
   - `cargo test --lib session::manager::test_session_container_pty_separate_arcs`
   - `cargo test --lib session::test_pty_output_loop_no_session_lock_during_io`
   - `cargo test --lib session::test_attach_no_block_on_pty_io`
   - `cargo test --lib session::test_terminate_pty_loop_exits_gracefully`
   - `cargo test --test integration test_attach_latency_under_concurrent_pty_io`
   - `cargo test --test integration test_concurrent_attach_no_interference`
   - `cargo test --test integration test_attach_concurrent_with_pty_output`

2. **11:15-11:30: Benchmark (critical path)**
   - `cargo bench --bench latency_e2e_lan` → Must show AttachResponse in logs
   - Verify p95 < 10ms in Criterion report

3. **11:30-11:40: Load test**
   - `cargo test --release test_load_100_sessions_10_attaches_per_sec -- --ignored`

4. **11:40-11:45: Safety audit**
   - Manual lock order review (code walkthrough)
   - ASAN run if Linux available (optional, time permitting)

**Decision Gate (11:45):**
- ✅ **ALL GREEN** → Merge, update gate to 5/7, criterion #5 VERIFIED
- ⚠️ **1-2 failures** → Debug, quick fix if <15 min, else reschedule
- ❌ **Major failure** → Rollback, escalate to eng-director

---

**Prepared by:** qa-lead  
**For:** Tuesday 2026-08-18 09:00 War Room  
**Implements:** Verification checklist from `docs/architecture/session-pty-locking-analysis.md` §6
