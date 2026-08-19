# Soak Test Integration Plan
**Status:** DRAFT - Awaiting eng-director approval  
**Owner:** devops-lead  
**Created:** 2026-08-15  
**Target:** Criterion #7 - 24h Stability Validation (SRS §7.1)

## Current State Assessment

### Completed ✅
1. **Soak test harness infrastructure** (`crates/master/tests/soak/stability_24h.rs`)
   - Memory monitoring (Windows + cross-platform abstraction)
   - Zombie process detection (Windows ConPTY + Linux/macOS)
   - Configurable duration/intervals via env vars
   - Async memory monitor thread with background sampling
   - ~400 lines, well-structured, documented

2. **PowerShell monitoring tool** (`tools/soak-test-monitor.ps1`)
   - Real-time process monitoring with CSV output
   - Crash detection with automatic test failure
   - Memory/handle leak detection with thresholds
   - Timestamped trend data for post-analysis

3. **Session CRUD APIs** (`crates/master/src/session/manager.rs`)
   - `create_session(working_dir, rows, cols)` ✅
   - `kill_session(session_id)` ✅
   - `send_input(session_id, data)` ✅
   - `resize_session(session_id, rows, cols)` ✅
   - `attach_client/detach_client` ✅
   - All async, error-handled, production-ready

### Critical Gap ❌
**The soak test does NOT use the actual Session CRUD APIs.**

Current implementation (lines 220-244 of `stability_24h.rs`):
```rust
fn simulate_session_workload(iteration: usize, session_id: usize) {
    // In a real test, this would create actual sessions via the Master API
    // For now, we simulate the workload patterns
    
    thread::sleep(Duration::from_millis(10));  // ← Not testing real system!
    // ... more sleep() calls
}
```

**Impact:** Test runs successfully but validates nothing about MONOTERMINAL's actual stability.

## Integration Work Required

### Phase 1: Test Runtime Setup (2 hours)
1. **Add tokio runtime to test**
   ```rust
   #[tokio::test]
   #[ignore]
   async fn test_24h_stability_zero_crashes() {
       let runtime = tokio::runtime::Runtime::new().unwrap();
       // ... rest of test
   }
   ```

2. **Initialize SessionManager**
   ```rust
   let session_manager = Arc::new(SessionManager::new(Some("cmd.exe".to_string())));
   ```

3. **Handle async/sync boundary**
   - Current test uses `thread::spawn()` for parallelism
   - Need to bridge sync test threads with async SessionManager calls
   - Options:
     - Convert entire test to tokio tasks
     - Use `runtime.block_on()` in each thread
     - **Recommended:** Hybrid - tokio tasks for session lifecycle, sync for monitoring

### Phase 2: Replace Simulated Workload (2 hours)
Replace `simulate_session_workload()` with real SessionManager calls:

```rust
async fn real_session_workload(
    session_manager: Arc<SessionManager>,
    iteration: usize,
    session_id: usize,
) -> Result<(), String> {
    // 1. Create session
    let sid = session_manager
        .create_session(None, 24, 80)
        .await
        .map_err(|e| format!("Failed to create session: {}", e))?;
    
    // 2. Send commands to exercise PTY
    let commands = vec![
        b"echo 'Soak test iteration'\n",
        b"dir\n",  // Windows
        b"echo Done\n",
    ];
    
    for cmd in commands {
        session_manager
            .send_input(sid, cmd)
            .await
            .map_err(|e| format!("Failed to send input: {}", e))?;
        
        // Brief delay to let PTY process
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    // 3. Clean up session
    session_manager
        .kill_session(sid)
        .await
        .map_err(|e| format!("Failed to kill session: {}", e))?;
    
    println!("[SESSION] Iteration {}: Session {} lifecycle complete", iteration, session_id);
    Ok(())
}
```

### Phase 3: Update Test Loop (1 hour)
Modify main test loop to handle async workload:

```rust
while start_time.elapsed() < duration {
    iteration += 1;
    
    // Spawn session workload tasks
    let mut task_handles = vec![];
    for session_id in 0..config.sessions_per_iteration {
        let mgr = session_manager.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = real_session_workload(mgr, iteration, session_id).await {
                eprintln!("ERROR: Session workload failed: {}", e);
                panic!("Session workload failed: {}", e);
            }
        });
        task_handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in task_handles {
        handle.await.expect("Session task panicked!");
    }
    
    // ... rest of loop (zombie check, sleep)
}
```

### Phase 4: Integration Testing (1 hour)
Before 24h run, validate the integrated harness:

1. **1-hour smoke test**
   ```bash
   SOAK_DURATION_HOURS=1 cargo test --release --test stability_24h -- --ignored --nocapture
   ```

2. **Verify metrics collection**
   - Memory baseline captured correctly
   - Handle count tracked (Windows)
   - Sessions created/destroyed without leaks

3. **Verify failure detection**
   - Inject intentional memory leak → test should fail
   - Kill process mid-test → monitor should detect crash

### Phase 5: 24h Production Run (24+ hours)
1. **Start soak test** (Saturday morning recommended)
   ```bash
   cargo test --release --test stability_24h -- --ignored --nocapture 2>&1 | tee soak-test.log
   ```

2. **Start PowerShell monitor** (parallel process)
   ```powershell
   .\tools\soak-test-monitor.ps1 -ProcessName monoterminal -OutputCsv soak-results.csv
   ```

3. **Monitor progress** (periodic check-ins)
   - Check `soak-test.log` for progress
   - Check `soak-results.csv` for trends
   - Alert if memory growth > 5% by 12h mark

4. **Collect results**
   - Test log with final ✅/❌ verdict
   - CSV with full time-series data
   - Screenshots of final metrics
   - Evidence for SRS §7.1 criterion #7

## Effort Estimate

| Phase | Hours | Dependencies |
|-------|-------|--------------|
| Test runtime setup | 2 | None |
| Replace simulated workload | 2 | Phase 1 |
| Update test loop | 1 | Phase 2 |
| Integration testing (1h smoke) | 1 | Phase 3 |
| **Subtotal: Integration work** | **6** | |
| 24h production run | 24+ | Phase 4 ✅ |
| **Total: Ready to deliver** | **30** | |

## Timeline Options

### Option A: Push to Thursday/Friday Delivery
- **Monday:** Complete integration work (6h)
- **Tuesday:** 1h smoke test + fixes
- **Wednesday:** Start 24h run (morning)
- **Thursday:** Collect results (morning), deliver report
- **Risk:** Low - full validation cycle
- **Recommendation:** ✅ Most robust

### Option B: Compress to Wednesday Delivery (RISKY)
- **Saturday:** Complete integration work (6h)
- **Sunday:** 1h smoke test + fixes, start 24h run (evening)
- **Monday:** Monitor (passive)
- **Tuesday:** Collect results (morning), deliver report by EOD
- **Wednesday:** Buffer day for issues
- **Risk:** Medium - compressed timeline, weekend work
- **Recommendation:** ⚠️ Achievable but tight

### Option C: 1h Smoke Test Wednesday, Full 24h Deferred
- **Saturday:** Complete integration work (6h)
- **Sunday:** 1h smoke test, deliver smoke results
- **Wednesday:** Present 1h smoke test results as "validation in progress"
- **Weekend:** Full 24h soak test
- **Next Monday:** Deliver full 24h results
- **Risk:** Low - staged delivery
- **Recommendation:** ✅ If criterion #7 is non-blocking for Phase 2 entry

### Option D: Defer Entirely (Per Earlier Discussion)
- Criterion #7 noted as "parallel/non-blocking" in coordination session
- Could defer to post-Phase 2 if other criteria (1-6) are blockers
- **Risk:** None to Phase 2 entry, but missing acceptance gate validation
- **Recommendation:** Only if other criteria are critical path

## Integration Issues Discovered

### Issue 1: Async/Sync Bridge Required
- **Problem:** Test harness is sync (`std::thread`), SessionManager is async (`tokio`)
- **Solution:** Hybrid approach or full tokio conversion
- **Impact:** 2h of Phase 1 effort

### Issue 2: No Explicit Session Lifecycle Cleanup Tracked
- **Problem:** Test creates sessions but doesn't verify they're fully destroyed
- **Solution:** Add session count assertions between iterations
- **Impact:** 30m additional testing in Phase 4

### Issue 3: PTY Output Not Consumed
- **Problem:** Sessions generate PTY output that's not read, may cause backpressure
- **Solution:** Attach mock client to drain output channel
- **Impact:** 1h additional work in Phase 2

## Acceptance Criteria (Self-Check Before Delivery)

- [ ] Soak test calls real `SessionManager::create_session()` ✅
- [ ] Soak test calls real `SessionManager::kill_session()` ✅  
- [ ] Soak test sends real input via `send_input()` ✅
- [ ] 1-hour smoke test passes with 0 crashes ✅
- [ ] 1-hour smoke test shows memory growth < 2% ✅
- [ ] 1-hour smoke test shows handle count stable ✅
- [ ] 24-hour production run completes (if approved) ✅
- [ ] Results documented with evidence for SRS §7.1 ✅

## Open Questions for eng-director

1. **Which timeline option do you prefer?** (A/B/C/D)
2. **Is criterion #7 blocking Phase 2 entry, or parallel?** (impacts urgency)
3. **Weekend availability approved?** (needed for Option B)
4. **What's the acceptable risk level?** (affects smoke vs full test decision)

## Rollback Plan (If Integration Fails)

If integration work reveals deeper issues:
1. **Document blockers** (e.g., SessionManager not thread-safe, PTY backend crashes)
2. **Escalate to rust-backend-lead** (API issues)
3. **Escalate to principal-architect** (architectural issues)
4. **Fall back to simulated harness** for timeline, note as "infrastructure validated, API integration deferred"

---

**Next Step:** Awaiting eng-director decision on timeline option (A/B/C/D) before proceeding with integration work.
