# 24-Hour Soak Test Execution Plan
**Phase 1 Acceptance Criterion #7: Zero Crashes**

## Document Control
- **Version:** 1.0
- **Date:** 2026-08-16
- **Owner:** devops-lead
- **Status:** Ready to Execute (Pending task-7 validation)

## Dependencies
- ✅ Test harness implemented: `crates\master\tests\soak\stability_24h.rs`
- ⏳ **BLOCKED:** task-4 (AbortOnDrop implementation) - RUNNING
- ⏳ **BLOCKED:** task-7 (Memory leak validation) - Pending task-4
- ⏳ **PENDING:** sre-observability-engineer monitoring setup

## Test Objective
Validate SRS §7.1 Phase 1 acceptance criterion #7:
- **Zero crashes** in 24-hour continuous operation
- **Memory growth ≤ 10%** from baseline
- **No handle leaks** (Windows)
- **No zombie PTY processes**

## Previous Run Context
Previous soak test run identified SessionManager memory leak:
- **Issue:** Background tasks not cancelled on drop → 52.1% memory leak
- **Fix:** AbortOnDrop pattern (task-4, rust-engineer-storage)
- **Status:** Implementation in progress, validation pending

## Test Configuration

### Primary Test (Via Orchestrator - RECOMMENDED)
```powershell
# Full 24-hour run with external monitoring
cd scripts\soak-monitor
.\run-full-soak-test.ps1 -DurationHours 24

# With crash dump collection
.\run-full-soak-test.ps1 -DurationHours 24 -EnableCrashDumps
```

### Development Validation (1-hour run)
```powershell
# Quick validation after memory leak fix
cd scripts\soak-monitor
.\run-full-soak-test.ps1 -DurationHours 1
```

### Manual Test Execution (If Needed)
```bash
# Direct cargo test (no orchestration)
cd crates\master
$env:SOAK_DURATION_HOURS=24
cargo test --release --test stability_24h -- --ignored --nocapture test_24h_stability_zero_crashes
```

### Test Parameters
- **Duration:** 24 hours (86,400 seconds)
- **Session creation:** 10 sessions every 5 minutes
- **Total sessions created:** ~2,880 sessions
- **Memory sampling:** Every 5 minutes (288 samples)
- **Zombie check:** Every 10 iterations (~50 minutes)
- **Default shell:** cmd.exe

## Test Workload
Each session (via real SessionManager APIs):
1. **Create session:** `SessionManager::create_session()` with PTY backend
2. **Send commands:**
   - `echo Soak test iteration\n`
   - `dir\n` (Windows directory listing)
   - `echo Done\n`
3. **Clean up:** `SessionManager::kill_session()`

**Key:** All operations use real APIs, not mocks. Exercises:
- ConPTY backend creation/teardown
- PTY I/O paths
- Session lifecycle management
- Memory allocation/deallocation

## Success Criteria

### Pass Conditions
1. ✅ **Zero crashes** - Test runs to completion (24 hours)
2. ✅ **Memory stable** - Final working set growth ≤ 10%
3. ✅ **No handle leaks** - Handle count growth ≤ 10%
4. ✅ **No zombie processes** - No orphaned shell processes

### Automatic Failure Conditions
Test will **panic and fail** if:
- Memory growth exceeds 10% at ANY checkpoint
- Zombie process count > 100
- Session API call fails (create/input/kill)
- Memory monitor detects leak during run

## Monitoring Setup

### Built-in Monitoring (Test Process)
- **Memory metrics:** Working Set, Private Bytes, Handle Count (Windows)
- **Sample interval:** 5 minutes (300 seconds)
- **Zombie checks:** Every 10 iterations
- **Method:** PowerShell Get-Process queries

### External Monitoring (SRE - COMPLETE ✅)
**Infrastructure:** `scripts/soak-monitor/` suite by sre-observability-engineer
- **Orchestrator:** `run-full-soak-test.ps1` - Single entry point
- **Monitor:** `external-monitor.ps1` - 1-minute samples, independent of test process
- **Evidence:** `collect-evidence.ps1` - Automated forensics collection

**Metrics collected (1-min intervals):**
- Memory (Working Set, Private Bytes, Handle Count)
- CPU (per-core utilization)
- Network (TCP connections, WebSocket port 5000)
- Process health (thread count, page faults)
- Crash detection (immediate CRITICAL alert)

**Alerting thresholds:**
- Memory growth >8% → WARNING (before 10% test failure)
- Handle growth >50% → WARNING (potential leak)
- CPU >80% sustained → WARNING (potential spin loop)
- Process crash → CRITICAL (test failure)

**Evidence auto-collected:**
- Windows Event Logs (Application, System, Crashes)
- Performance counter snapshots
- Network statistics (TCP states, WebSocket)
- Disk I/O stats
- System info (OS, memory, uptime)
- Crash dumps (if `-EnableCrashDumps` flag used)

**Output structure:**
```
soak-results/run-YYYYMMDD-HHMMSS/
├── SUMMARY.json                    # ← Final verdict (PASSED/FAILED)
├── soak-test-output.log            # ← Cargo test console
├── external-monitor-output.log     # ← Monitor console
├── external-metrics-*.csv          # ← 1440 rows (1/min)
├── alerts-*.log                    # ← All alerts
└── evidence-YYYYMMDD-HHMMSS/       # ← Forensics package
    ├── event-log-application.csv
    ├── event-log-system.csv
    ├── event-log-crashes.csv
    ├── perf-counters-snapshot.json
    ├── network-tcp-connections.csv
    └── ...
```

**Documentation:** `scripts/soak-monitor/README.md`

## Evidence Collection

### Test Artifacts (Auto-generated)
1. **Console output log** (stdout/stderr)
2. **Memory statistics** (embedded in console output)
3. **Final report** (stdout summary)

### Manual Collection (Pre-test)
- [ ] Baseline system state snapshot
- [ ] Windows Event Viewer baseline
- [ ] Process list snapshot
- [ ] Network connection baseline

### Manual Collection (Post-test)
- [ ] Final system state snapshot
- [ ] Windows Event Viewer logs
- [ ] Process list diff
- [ ] Network connection diff
- [ ] Any crash dumps (if applicable)

## Execution Checklist

### Pre-Execution
- [ ] ✅ Memory leak fix validated (task-7 complete)
- [ ] ✅ SRE monitoring setup complete (scripts/soak-monitor/ suite)
- [ ] **⚠️ WAIT FOR EXPLICIT APPROVAL from eng-director** (required before 24h execution)
- [ ] Run pre-flight check: `scripts\soak-test-preflight.ps1` (must pass all 10 checks)
- [ ] Optional: Run 1-hour smoke test and review results
- [ ] System stable, no other heavy processes
- [ ] PowerShell 5.1+ available
- [ ] Cargo toolchain available
- [ ] `monoterminal-master` crate builds clean
- [ ] Port 5000 available (daemon WebSocket server)
- [ ] Sleep disabled for 24h+ duration
- [ ] Notify eng-director: Test starting (when approved)

### During Execution
- [ ] Monitor console output periodically (don't disturb test)
- [ ] Check SRE alerts (if configured)
- [ ] Do NOT interact with test process
- [ ] Do NOT restart/suspend system

### Post-Execution
- [ ] Collect all evidence artifacts
- [ ] Generate summary report
- [ ] Compare baseline vs final state
- [ ] Report results to eng-director

## Risk Mitigation

### Known Risks
1. **Power loss** - Ensure laptop plugged in, power settings configured
2. **System sleep** - Disable sleep/hibernate for 24h duration
3. **Network interruption** - Test is local-only, low risk
4. **Resource exhaustion** - Built-in thresholds will fail test early

### Contingencies
- **Test fails at <12h:** Analyze failure, fix, restart full 24h run
- **Test fails at >12h:** Consult eng-director on acceptable shortened run
- **Inconclusive results:** May need to extend duration or add instrumentation

## Expected Outcomes

### If Pass
- **Criterion #7 verified:** ✅ Zero crashes in 24h validated
- **Phase 1 gate progress:** 5/7 criteria complete
- **Next steps:** Continue with remaining criteria (if any)
- **Report to:** eng-director with full evidence

### If Fail
- **Root cause analysis:** Memory profiling, crash dump analysis
- **Fix required:** Cycle back to implementation team
- **Re-test required:** Full 24h run after fix

## Timeline

### Estimated Schedule
- **Current:** Preparing test environment (task-8 active)
- **Unblock:** When task-7 completes (memory leak validated)
- **Start:** Immediately after unblock + SRE coordination
- **Duration:** 24 hours
- **Report:** Within 2 hours of completion

## Reporting Template

### Success Report
```
SUBJECT: ✅ Criterion #7 VERIFIED - 24h Soak Test PASSED

Summary:
- Duration: 24.0 hours (full run)
- Sessions created: 2,880
- Crashes: 0
- Memory growth: X.X% (≤10% threshold)
- Handle growth: X.X% (≤10% threshold)
- Zombie processes: 0

Evidence:
- Console log: [attached]
- Baseline snapshots: [attached]
- Final snapshots: [attached]
- SRE monitoring report: [linked]

Status: Phase 1 Criterion #7 COMPLETE ✅
Next: Criteria #5 (pending) and #6 (pending)
```

### Failure Report
```
SUBJECT: ❌ Criterion #7 NOT MET - 24h Soak Test FAILED

Summary:
- Duration: X.X hours (failed at iteration Y)
- Failure mode: [crash / memory leak / handle leak / zombie processes]
- Root cause: [analysis]

Evidence:
- Console log with failure: [attached]
- Memory profile: [attached]
- Crash dump (if applicable): [attached]

Action Required:
- [Fix needed]
- Re-test: Full 24h run after fix
```

## References
- SRS §7.1: Phase 1 Acceptance Criteria
- Architecture §2: Session Management
- Test: `crates\master\tests\soak\stability_24h.rs`
- Previous run context: mem-run-20260816073249 (E2E test infrastructure)

## Notes
- **DO NOT run shortened test for gate validation** - Must be full 24 hours
- **1-hour test is for development validation only** (post-fix smoke test)
- **Test is CPU-light, memory-moderate** - Should not impact system significantly
- **Windows-specific** - Uses PowerShell, cmd.exe, Windows handle tracking
- **Rust test is authoritative** - Python E2E test is supplementary
