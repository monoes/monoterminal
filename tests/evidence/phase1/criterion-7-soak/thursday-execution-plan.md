# Criterion #7: 24-Hour Soak Test Execution Plan
**Execution Date:** Thursday 2026-08-19  
**Owner:** devops-lead  
**Status:** READY (pending Monday smoke test validation)

## Pre-Execution Checklist

### Monday Preparation (COMPLETE)
- [x] SessionManager integration verified (already in code)
- [ ] 1-hour smoke test passed (in progress - results at 11:33 AM)
- [ ] Monitor script reviewed (`tools/soak-test-monitor.ps1`)
- [ ] Execution procedures documented (this file)

### Thursday Morning Pre-Flight (8:00-9:00 AM)
- [ ] Verify workspace builds clean: `cargo build --workspace --all-features`
- [ ] Verify no pending code changes (clean git status)
- [ ] Close unnecessary applications (free memory)
- [ ] Disable Windows sleep/hibernate for 24+ hours
- [ ] Verify disk space > 10GB free (for logs)

## Execution Timeline

### Thursday 9:00 AM: Start 24h Soak Test

**Terminal 1: Soak Test Execution**
```powershell
cd C:\Users\nokho\Desktop\projects\monoterminal

# Set environment
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:SOAK_DURATION_HOURS = "24"

# Run test with logging
cargo test --release --test stability_24h -- --ignored --nocapture 2>&1 | Tee-Object -FilePath tests\evidence\phase1\criterion-7-soak\soak-test-24h.log
```

**Expected output:**
```
==================================================
 MONOTERMINAL 24-Hour Soak Test
==================================================
Configuration:
  Duration:              24 hours
  Session interval:      300 seconds (5 min)
  Sessions per iteration: 10
  Memory check interval: 300 seconds
  Max memory growth:     10.0%
==================================================
```

### Thursday 9:05 AM: Start Monitor (Parallel)

**Terminal 2: PowerShell Monitor**
```powershell
cd C:\Users\nokho\Desktop\projects\monoterminal

# Wait for test binary to start (check Task Manager for monoterminal process)
# Then start monitor:
.\tools\soak-test-monitor.ps1 -ProcessName "monoterminal" -DurationHours 24 -OutputCsv tests\evidence\phase1\criterion-7-soak\soak-monitor-24h.csv
```

**Expected output:**
```
==================================================
 MONOTERMINAL 24-Hour Soak Test Monitor
==================================================
Configuration:
  Process Name:     monoterminal
  Interval:         300 seconds
  Duration:         24 hours
  Output CSV:       tests\evidence\phase1\criterion-7-soak\soak-monitor-24h.csv

[09:05:00] Found process: monoterminal (PID: 12345)

Baseline Measurements:
  Working Set:      245.32 MB
  Private Bytes:    312.45 MB
  Handle Count:     1234
  Thread Count:     8
```

### Thursday 9:00 AM - Friday 9:00 AM: Monitoring

**Periodic Check-ins:**
- **12:00 PM (3h):** Check both terminals, verify progress
- **6:00 PM (9h):** Check memory growth < 5%
- **9:00 PM (12h):** Midpoint check
- **6:00 AM (21h):** Morning check
- **9:00 AM (24h):** Expected completion

**What to check:**
1. Both terminals still running (no crashes)
2. Test log showing iterations progressing
3. Monitor CSV showing stable metrics
4. Task Manager: monoterminal process still alive

**If issues detected:**
- Process crash → Test FAILS immediately
- Memory growth > 10% → Test FAILS
- Handle leak detected → Test FAILS
- Document time of failure and last known good state

### Friday 9:00 AM: Collect Results

**Terminal 1 (Test) should show:**
```
==================================================
 Soak Test Complete!
==================================================
Final memory:
  Working Set:  XXX MB (growth: X.X%)
  Private Bytes: XXX MB

SRS §7.1 Acceptance Criteria:
  ✅ Zero crashes - test ran to completion
  ✅ Memory stable (X.X% growth ≤ 10.0%)
  ✅ No zombie processes detected
  ✅ Memory monitor completed successfully

🎉 24-HOUR SOAK TEST PASSED
Total iterations: XXX
Total runtime: 24.00 hours
```

**Terminal 2 (Monitor) should show:**
```
==================================================
 24-Hour Soak Test Complete!
==================================================
Final Measurements:
  Working Set:      XXX MB (growth: X.X%)
  Handle Count:     XXXX (growth: X.X%)

SRS §7.1 Acceptance Criteria:
  ✅ Zero crashes
  ✅ Memory stable (< 10% growth)
  ✅ No handle leaks

🎉 SOAK TEST PASSED
Results saved to: tests\evidence\phase1\criterion-7-soak\soak-monitor-24h.csv
```

## Evidence Collection

### Required Evidence Files

1. **Test log** (from Terminal 1):
   - File: `tests/evidence/phase1/criterion-7-soak/soak-test-24h.log`
   - Contents: Full test output with iterations, memory checks, final verdict
   - Size: ~50-100MB (24h of console output)

2. **Monitor CSV** (from Terminal 2):
   - File: `tests/evidence/phase1/criterion-7-soak/soak-monitor-24h.csv`
   - Contents: Time-series data (timestamp, memory, handles, status)
   - Rows: ~288 (24h * 60min / 5min intervals)

3. **Screenshots**:
   - `tests/evidence/phase1/criterion-7-soak/final-test-output.png` (Terminal 1 final screen)
   - `tests/evidence/phase1/criterion-7-soak/final-monitor-output.png` (Terminal 2 final screen)
   - `tests/evidence/phase1/criterion-7-soak/task-manager-final.png` (Windows Task Manager showing process)

### Analysis and Report

**Create**: `tests/evidence/phase1/criterion-7-soak/RESULTS.md`

```markdown
# Criterion #7: 24-Hour Soak Test Results

**Execution Date:** Thursday 2026-08-19 9:00 AM - Friday 2026-08-20 9:00 AM  
**Test Duration:** 24.00 hours  
**SRS Requirement:** §7.1 - Zero crashes, memory stable, no resource leaks  

## Verdict: ✅ PASS / ❌ FAIL

## Metrics Summary

| Metric | Baseline | Final | Growth | Threshold | Status |
|--------|----------|-------|--------|-----------|--------|
| Working Set | XXX MB | XXX MB | X.X% | < 10% | ✅ PASS |
| Private Bytes | XXX MB | XXX MB | X.X% | < 10% | ✅ PASS |
| Handle Count | XXXX | XXXX | X.X% | < 5% | ✅ PASS |
| Thread Count | XX | XX | X.X% | Stable | ✅ PASS |
| Process Crashes | 0 | - | - | 0 | ✅ PASS |

## SRS §7.1 Acceptance Criteria

- [x] **Zero crashes** - Process ran continuously for 24 hours
- [x] **Memory stable** - Working set growth X.X% (< 10% threshold)
- [x] **No handle leaks** - Handle count growth X.X% (< 5% threshold)
- [x] **No zombie PTY processes** - Verified via periodic checks

## Evidence Files

- Test log: `soak-test-24h.log` (XXX MB)
- Monitor CSV: `soak-monitor-24h.csv` (XXX rows)
- Screenshots: 3 files (final outputs + Task Manager)

## Notable Events

- None (clean 24h run)
- OR: [Document any warnings, close calls, temporary spikes]

## Conclusion

MONOTERMINAL master daemon successfully completed 24-hour stability test under continuous load:
- XXX session create/destroy cycles
- XXX total sessions exercised
- Zero crashes, stable memory, no resource leaks

**SRS §7.1 Criterion #7: ✅ PASSED**
```

## Troubleshooting

### Issue: Test binary doesn't start

**Symptoms:** "cargo test" compiles but test doesn't execute

**Fix:**
```powershell
# Verify test exists
cargo test --release --test stability_24h --list

# If not found, check test file exists
Test-Path crates\master\tests\soak\stability_24h.rs
```

### Issue: Monitor can't find process

**Symptoms:** "Process 'monoterminal' not found"

**Fix:**
1. Check Task Manager for actual process name
2. Test process might be named differently (e.g., "stability_24h.exe")
3. Update monitor: `.\tools\soak-test-monitor.ps1 -ProcessName "actual-name"`

### Issue: Memory grows rapidly

**Symptoms:** Memory growth > 5% in first few hours

**Action:**
1. **Do NOT stop test** - collect evidence of leak
2. Monitor CSV will capture trend
3. Test will fail at 10% threshold
4. Escalate to rust-backend-lead with CSV evidence

### Issue: Process crashes mid-test

**Symptoms:** Monitor detects "CRASH DETECTED!"

**Action:**
1. Note crash time from monitor output
2. Check test log for last iteration before crash
3. Check Windows Event Viewer for crash details
4. Collect: crash time, last log entries, Event Viewer errors
5. Escalate to rust-backend-lead with crash evidence

## Post-Execution Cleanup

```powershell
# After test completes:

# 1. Stop monitor if still running (Ctrl+C in Terminal 2)

# 2. Verify evidence files exist
Test-Path tests\evidence\phase1\criterion-7-soak\soak-test-24h.log
Test-Path tests\evidence\phase1\criterion-7-soak\soak-monitor-24h.csv

# 3. Create screenshots (manual)

# 4. Write RESULTS.md (manual)

# 5. Commit evidence to git
git add tests/evidence/phase1/criterion-7-soak/
git commit -m "Add Criterion #7 24h soak test evidence"

# 6. Notify qa-lead for Phase 1 gate report inclusion
```

## Rollback Plan

**If soak test fails:**

1. **Document failure** in RESULTS.md (FAIL verdict)
2. **Collect all evidence** (logs, CSV, screenshots of failure)
3. **Escalate to rust-backend-lead** with:
   - Failure mode (crash, memory leak, handle leak)
   - Time of failure (hours elapsed)
   - Reproduction steps (if crash is consistent)
4. **Decision points:**
   - Is failure reproducible in 1h smoke test?
   - Is issue in SessionManager, PTY backend, or soak test harness?
   - Can issue be fixed before Phase 1 gate?
   - OR: Defer Criterion #7 to Phase 2 (non-blocking per ADR)

---

**Execution Plan Ready** - Pending Monday smoke test validation results.
