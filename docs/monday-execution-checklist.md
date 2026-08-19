# Monday Execution Checklist - Aug 18, 2026

**Critical Day:** Criterion #5 Latency + Memory Leak Verification  
**Stakes:** 5/7 gate passage depends on Criterion #5 success  
**Teams:** performance-engineer (critical path) + rust-backend-lead (parallel)

---

## Timeline Overview

| Time | Activity | Owner | Status |
|------|----------|-------|--------|
| 08:45 | Criterion #5 pre-flight checks | performance-engineer | ⏳ Scheduled |
| 09:00 | Criterion #5 execution | performance-engineer | ⏳ Scheduled |
| 09:00 | Memory leak fix compilation | rust-backend-lead | ⏳ Scheduled |
| 09:05 | Criterion #5 expected completion | performance-engineer | ⏳ Expected |
| 09:30 | Memory leak 5-min smoke test | rust-backend-lead | ⏳ Scheduled |
| 10:00 | Memory leak results | rust-backend-lead | ⏳ Expected |
| 14:00 | Criterion #5 checkpoint | performance-engineer → eng-director | ⏳ Required |
| EOD | Criterion #5 evidence package | performance-engineer | ⏳ Expected |

---

## Track 1: Criterion #5 Latency (CRITICAL PATH 🎯)

### Pre-Flight Check (08:45)

**Environment Verification:**
```powershell
# 1. Port availability check
netstat -ano | findstr :18080
# Expected: No output (port free)
# If occupied: taskkill /PID <PID> /F

# 2. Cargo process cleanup
Get-Process | Where-Object {$_.ProcessName -like "*cargo*"}
# Expected: Minimal/no cargo processes
# If many: Investigate and clean up

# 3. Server binding smoke test
cd C:\Users\nokho\Desktop\projects\monoterminal
cargo run --release -p monoterminal-master -- --bind 127.0.0.1:18080 --test-mode
# Expected: "Server bound to 127.0.0.1:18080" within 5 seconds
# If fails: Check firewall, port conflicts, binary state
```

**Pre-Flight Checklist:**
- [ ] Port 18080 available
- [ ] No hung cargo processes
- [ ] Server binding test passes
- [ ] Benchmark file unchanged: `crates/master/benches/latency_e2e_lan.rs`
- [ ] Evidence directory ready: `tests/evidence/phase1/criterion-5-latency/`

**If ANY pre-flight fails:** Escalate to rust-backend-lead before 09:00 execution

---

### Execution (09:00)

**Command:**
```powershell
# Terminal 1 (main execution)
cd C:\Users\nokho\Desktop\projects\monoterminal
cargo bench --bench latency_e2e_lan --no-fail-fast 2>&1 | Tee-Object -FilePath tests/evidence/phase1/criterion-5-latency/benchmark-run-20260818-090000.log
```

**Monitoring (DO NOT WALK AWAY):**
- Watch for "Warming up for 3.0000 s" message
- **5-minute timeout:** If warmup exceeds 5 minutes → CTRL+C and escalate
- Expected completion: 3-5 minutes total
- Watch for "Analyzing" phase (indicates warmup success)

**Success Indicators:**
```
Benchmarking e2e_lan_latency/real_master_rtt_loopback
Warming up for 3.0000 s
Collecting 10000 samples...
Analyzing...
e2e_lan_latency/real_master_rtt_loopback
                        time:   [X.XXX ms X.XXX ms X.XXX ms]
```

**Failure Indicators:**
```
# Hang scenario (same as Aug 17 02:02)
Warming up for 3.0000 s
[NO FURTHER OUTPUT FOR >5 MINUTES]
→ ACTION: CTRL+C, escalate to rust-backend-lead

# Error scenario
Error: Failed to bind to 127.0.0.1:18080: ...
→ ACTION: Check pre-flight, retry once, then escalate

# Crash scenario
thread 'main' panicked at ...
→ ACTION: Capture full error, escalate to rust-backend-lead
```

---

### Results Extraction (09:05-09:30)

**Criterion.rs Output Locations:**
```powershell
# 1. HTML report
target/criterion/e2e_lan_latency/real_master_rtt_loopback/report/index.html

# 2. JSON estimates
target/criterion/e2e_lan_latency/real_master_rtt_loopback/base/estimates.json

# 3. Copy to evidence directory
Copy-Item target/criterion/e2e_lan_latency -Destination tests/evidence/phase1/criterion-5-latency/criterion-output -Recurse
```

**Extract p50/p95/p99 from JSON:**
```powershell
$json = Get-Content target/criterion/e2e_lan_latency/real_master_rtt_loopback/base/estimates.json | ConvertFrom-Json
$mean_ms = $json.mean.point_estimate / 1000000  # Convert ns to ms
Write-Host "Mean: $mean_ms ms"

# For percentiles, use criterion HTML report or parse raw measurements
```

**Verdict Determination:**
- p95 < 10ms → **PASS** ✅
- p95 = 10-11ms → **BORDERLINE** (qa-lead review)
- p95 > 11ms → **FAIL** ❌

---

### Checkpoint Report (14:00)

**Format (one sentence to eng-director):**

**If PASS:**
```
PASS: p95=8.4ms (15% margin below 10ms threshold)
```

**If BORDERLINE:**
```
BORDERLINE: p95=10.3ms (3% above threshold, requesting qa-lead review)
```

**If FAIL:**
```
FAIL: p95=12.7ms (27% above threshold)
```

**If HUNG:**
```
HUNG: Warmup phase exceeded 5-min timeout, escalated to rust-backend-lead
```

**Delivery Method:** org_send to eng-director

---

### Evidence Package (EOD Monday or Tuesday Morning)

**Required Deliverables:**
1. **VERIFICATION.md** (create new file)
   - Verdict: PASS/FAIL
   - Results: p50, p95, p99 latencies
   - Test configuration: 10,000 samples, loopback
   - Evidence locations

2. **Criterion HTML Report**
   - Copy from target/criterion to evidence directory
   - Full histogram, percentiles, confidence intervals

3. **Criterion JSON Data**
   - estimates.json (for programmatic verification)

4. **Execution Log**
   - benchmark-run-20260818-090000.log
   - Server startup, warmup, execution, completion

**Template VERIFICATION.md:**
```markdown
# Criterion #5: <10ms LAN Latency (p95) - Verification Report

**Date:** 2026-08-18  
**Verifier:** performance-engineer  
**Status:** [PASS/FAIL]

## Results

**Target:** p95 < 10ms (SRS §7.1)  
**Actual Results:**
- p50: X.XX ms
- p95: X.XX ms  ← PRIMARY METRIC
- p99: X.XX ms

**Verdict:** [✅ PASS / ❌ FAIL]

## Test Configuration
- Samples: 10,000
- Warmup: 3 seconds
- Measurement: ~30 seconds
- Network: Loopback (127.0.0.1)
- Server: 127.0.0.1:18080
- Protocol: WebSocket + TLS 1.3 + JWT

## Evidence
- HTML Report: criterion-output/report/index.html
- JSON Data: criterion-output/base/estimates.json
- Execution Log: benchmark-run-20260818-090000.log
```

---

## Track 2: Memory Leak Fix Verification (PARALLEL)

### Compilation + Unit Tests (09:00-09:15)

**Owner:** rust-backend-lead

**Commands:**
```powershell
# 1. Build master crate
cd C:\Users\nokho\Desktop\projects\monoterminal
cargo build -p monoterminal-master

# 2. Run PTY unit tests
cargo test -p monoterminal-master --lib pty::windows

# Expected: All tests pass (no heap corruption, no PTY hangs)
```

**Success Criteria:**
- ✅ Compilation succeeds (no errors)
- ✅ Unit tests pass (0 failures)
- ✅ No STATUS_HEAP_CORRUPTION crashes

**If Fails:** Debug compilation errors or test failures before smoke test

---

### 5-Minute Smoke Test (09:30-09:40)

**Command:**
```powershell
cd C:\Users\nokho\Desktop\projects\monoterminal\scripts\soak-monitor
.\run-full-soak-test.ps1 -DurationHours 0.083  # 5 minutes
```

**Monitoring:**
- Memory metrics sampled every 1 minute
- Target: <10% growth across all metrics
- Baseline comparison: Aug 16 failure (52.1% WS growth)

**Success Criteria:**
| Metric | Baseline | Target Growth | Aug 16 Failure | Status |
|--------|----------|---------------|----------------|--------|
| Working Set | 8.01 MB | <10% | +52.1% | ⏳ |
| Private Bytes | 1.36 MB | <10% | +244% | ⏳ |
| Handle Count | 134 | <5% | +46.3% | ⏳ |

**Verdict:**
- All metrics <threshold → **PASS** → Schedule 1-hour smoke
- Any metric >threshold → **FAIL** → Debug fix

---

### Results Report (10:00)

**To:** eng-director + qa-lead

**Format:**
```
Memory Leak Fix - 5-min Smoke Test Results:
- Working Set: +X.X% (target: <10%)
- Private Bytes: +X.X% (target: <10%)
- Handle Count: +X.X% (target: <5%)
Verdict: [PASS/FAIL]
[If PASS: 1-hour smoke scheduled 10:30-11:30]
```

---

## Contingency Plans

### Scenario 1: Criterion #5 Hangs (Same as Aug 17)

**Symptoms:**
- Warmup message appears
- No further output for >5 minutes
- Server/client connected but RTT measurement stuck

**Actions:**
1. CTRL+C to terminate
2. Capture last 50 lines of log
3. Send to rust-backend-lead: "Criterion #5 hung at warmup, same as Aug 17 02:02"
4. rust-backend-lead investigates:
   - Server/client communication issue
   - RTT measurement loop deadlock
   - Timeout configuration

**Timeline Impact:**
- If debugged Monday → Retry Tuesday
- If complex → Escalate to principal-architect

**Gate Impact:**
- 4/7 remains (below 5/7 threshold)
- Need alternative path to 5/7 (coverage boost or wait for soak)

---

### Scenario 2: Criterion #5 Fails (p95 > 10ms)

**Symptoms:**
- Benchmark completes successfully
- Results show p95 = 11-15ms (above threshold)

**Actions:**
1. Verify loopback vs. real LAN test
2. Check for background processes (antivirus, Windows Update)
3. Retry on clean system
4. If consistent failure:
   - Document actual latency
   - Request qa-lead ruling (borderline case)
   - Consider Phase 2 deferral

**Gate Impact:**
- 4/7 remains (below 5/7 threshold)
- Need coverage boost (1 week) OR soak test (Wed-Thu)

---

### Scenario 3: Memory Leak Persists

**Symptoms:**
- 5-min smoke test shows >10% growth
- Fix didn't resolve issue

**Actions:**
1. Verify fix was compiled in (check binary timestamp)
2. Capture memory profile during test
3. Analyze which resources are leaking
4. Iterate fix (additional cleanup needed)

**Timeline Impact:**
- Delays soak test until fix verified
- Does NOT block 5/7 (Criterion #7 is deferred)

**Gate Impact:**
- No impact (memory leak is Phase 2 work)
- Soak test remains deferred

---

## Success Criteria Summary

**For 5/7 Gate Passage:**
- ✅ Criterion #5 PASS (p95 < 10ms) **← REQUIRED**
- ✅ 4/7 verified (already achieved)
- = **5/7 ACHIEVED**

**For Phase 2 Readiness:**
- ✅ Memory leak fix verified (5-min smoke)
- ✅ 1-hour smoke passes
- ✅ 24-hour soak ready (Wed-Thu)

---

## Communication Protocol

### Real-Time Updates (09:00-10:00)

**DO NOT wait until 14:00 if critical events occur:**

**Immediate Escalation Triggers:**
- Criterion #5 hangs >5 minutes
- Criterion #5 crashes/errors
- Memory leak smoke test fails catastrophically

**Method:** org_send to eng-director

### Checkpoint (14:00) - MANDATORY

**One sentence summarizing Criterion #5 status:**
- PASS/BORDERLINE/FAIL/HUNG format
- Include p95 value if available

**Method:** org_send to eng-director

### EOD Summary (Monday Evening or Tuesday Morning)

**Full results package:**
- Criterion #5: Detailed results + evidence
- Memory leak: Smoke test results + next steps

---

## Team Readiness Confirmation

**performance-engineer:**
- ✅ Execution protocol confirmed
- ✅ Pre-flight checklist ready
- ✅ Contingency plans understood
- ✅ Checkpoint format agreed
- ✅ High confidence (infrastructure validated)

**rust-backend-lead:**
- ✅ Memory leak fix code complete
- ✅ Monday morning verification plan
- ✅ Smoke test procedure ready
- ✅ Quality-over-speed approach confirmed

**qa-lead:**
- ✅ 4/7 verified count confirmed
- ✅ Gate strategy finalized
- ✅ Standing by for results

**eng-director:**
- ✅ All teams aligned
- ✅ Critical path clear
- ✅ Gate passage strategy defined
- ✅ Phase 2 plan drafted

---

## Monday Evening Wrap-Up

**If Criterion #5 PASSES:**
- 🎉 **5/7 ACHIEVED**
- ✅ Gate passage authorized (pending qa-lead final sign-off Tuesday)
- ✅ Phase 2 transition APPROVED
- 📋 Tuesday: Phase 2 kickoff planning

**If Criterion #5 needs retry/investigation:**
- 📊 Assess root cause
- 🔧 Fix or adjust approach
- 📅 Reschedule verification
- 📋 Update gate timeline

**Either way:**
- ✅ Memory leak status known (fix validated or iteration needed)
- ✅ Clear path forward defined
- ✅ Team aligned on next steps

---

**Prepared by:** eng-director  
**Date:** 2026-08-17 (Sunday evening)  
**For:** Monday Aug 18 execution  
**Status:** READY - All teams confirmed
