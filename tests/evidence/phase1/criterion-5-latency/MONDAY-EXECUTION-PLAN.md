# Criterion #5 Latency Benchmark - Monday Execution Plan

**Date:** 2026-08-18 (Monday)  
**Owner:** performance-engineer  
**Approved by:** eng-director (2026-08-17 decision)

---

## Executive Summary

**Decision:** Fresh start execution Monday 09:00 (not tonight)  
**Reason:** Overnight benchmark (Aug 17 02:02) hung during warmup - clean slate approach  
**Timeline:** Monday 09:00 start → Monday 14:00 checkpoint → Tuesday EOD completion

---

## Pre-Flight Checklist (Monday 08:45)

### 1. Port Availability Check

```powershell
# Check if port 18080 is in use
netstat -ano | findstr :18080

# If any process found, kill it:
# taskkill /PID <PID> /F
```

**Expected:** No output (port available)  
**If occupied:** Kill process, verify clear

### 2. Server Binding Smoke Test

```powershell
# Quick test - server can bind to 127.0.0.1:18080
# (Use existing test or minimal server startup)
```

**Expected:** Server binds successfully, no errors  
**If fails:** Check Windows Firewall, port conflicts

### 3. Clean Environment

```powershell
# Check for hung cargo/benchmark processes
tasklist | findstr /I "cargo monoterminal latency"

# If any found, kill them
```

**Expected:** No cargo processes running  
**Cleanup:** Kill any stale processes

### 4. Directory Preparation

```powershell
# Ensure log directory exists
mkdir -Force tests\evidence\phase1\criterion-5-latency

# Clear any partial criterion data (optional)
# rm -Recurse -Force target\criterion\e2e_lan_latency -ErrorAction SilentlyContinue
```

---

## Execution (Monday 09:00)

### Command

```powershell
# Full benchmark with logging
cargo bench --bench latency_e2e_lan --no-fail-fast 2>&1 | Tee-Object -FilePath tests/evidence/phase1/criterion-5-latency/benchmark-run-20260818-090000.log
```

### Configuration (from benchmark code)

- **Samples:** 100 (SHORT_TEST env var) or 10,000 (full)
- **Warmup:** 3 seconds
- **Measurement:** 30 seconds
- **Expected runtime:** 3-5 minutes total

### Monitoring Protocol

**DO:**
- ✅ Watch output in real-time (live tail)
- ✅ Note warmup completion timestamp
- ✅ Note "Analyzing" phase start
- ✅ Wait for criterion results output

**DON'T:**
- ❌ Walk away during warmup
- ❌ Assume it's working and multitask
- ❌ Wait more than 5 minutes if stuck in warmup

### Success Indicators

**Phase 1: Compilation (1-2 min)**
```
   Compiling monoterminal-master v0.1.0
   Finished `bench` profile [optimized] target(s) in 0.77s
```

**Phase 2: Server Setup (5 sec)**
```
✓ Server successfully bound to 127.0.0.1:18080
✅ Server ready for benchmark
```

**Phase 3: Warmup (3 sec)**
```
Benchmarking e2e_lan_latency/real_master_rtt_loopback
Benchmarking e2e_lan_latency/real_master_rtt_loopback: Warming up for 3.0000 s
[Progress messages...]
```

**Phase 4: Collecting Samples (30 sec)**
```
Benchmarking e2e_lan_latency/real_master_rtt_loopback: Collecting 100 samples
[Progress bar or iteration messages...]
```

**Phase 5: Analysis (<10 sec)**
```
Benchmarking e2e_lan_latency/real_master_rtt_loopback: Analyzing
time:   [X.XXX ms Y.YYY ms Z.ZZZ ms]
```

**Phase 6: Completion**
```
e2e_lan_latency/real_master_rtt_loopback
                        time:   [5.234 ms 5.678 ms 6.012 ms]
```

### Failure Indicators

**Hang Symptoms:**
- Warmup phase running >5 minutes
- No progress messages after "Warming up for 3.0000 s"
- CPU usage drops to 0% (process idle)
- No new log entries >2 minutes

**Action if hung:**
1. CTRL+C to stop
2. Note exact hang point (last log line)
3. Capture process state if possible
4. Escalate to rust-backend-lead

---

## Results Extraction

### Criterion Output Locations

```
target/criterion/e2e_lan_latency/real_master_rtt_loopback/
├── report/
│   └── index.html           (Visual report)
├── base/
│   └── estimates.json       (Raw percentile data)
└── new/
    └── estimates.json       (Latest run)
```

### Extract Percentiles

```powershell
# Parse JSON for p50, p95, p99
$json = Get-Content target/criterion/e2e_lan_latency/real_master_rtt_loopback/new/estimates.json | ConvertFrom-Json

# Mean estimate (nanoseconds → milliseconds)
$mean_ns = $json.mean.point_estimate
$mean_ms = [math]::Round($mean_ns / 1000000, 3)

# Note: Criterion doesn't directly expose p50/p95/p99 in JSON
# Use HTML report or raw samples for percentiles
```

**Alternative:** Read from HTML report or console output

### Pass/Fail Verdict

**Target:** p95 < 10ms (Phase 1 gate requirement per SRS §7.1)

**Thresholds:**
- **PASS:** p95 < 10.0 ms ✅
- **BORDERLINE:** p95 = 9.5-10.5 ms ⚠️ (within measurement error)
- **FAIL:** p95 > 10.5 ms ❌

**Expected range (from component analysis):**
- Loopback (127.0.0.1): p50 ~3-7ms, p95 ~6-10ms
- **Likely outcome:** PASS with ~1-2ms margin

---

## Checkpoint Report (Monday 14:00)

### Format (One Sentence)

**Success scenarios:**
- `PASS: p95=7.2ms` (if p95 < 10ms)
- `BORDERLINE: p95=9.8ms` (if p95 ≈ 10ms, needs review)

**Failure scenarios:**
- `FAIL: p95=12.3ms` (if p95 > 10ms)
- `HUNG: escalated to rust-backend-lead` (if warmup timeout repeated)

### Delivery

**To:** eng-director  
**Subject:** Criterion #5 Checkpoint - Monday 14:00  
**Message:** [One sentence from above]

---

## Final Delivery (Tuesday EOD)

### Evidence Package

1. **Criterion HTML Report**
   - Copy: `target/criterion/e2e_lan_latency/report/index.html`
   - To: `tests/evidence/phase1/criterion-5-latency/criterion-report.html`

2. **Criterion JSON Data**
   - Copy: `target/criterion/e2e_lan_latency/new/estimates.json`
   - To: `tests/evidence/phase1/criterion-5-latency/estimates.json`

3. **Benchmark Log**
   - Already saved: `benchmark-run-20260818-090000.log`

4. **VERIFICATION.md**
   - Create: `tests/evidence/phase1/criterion-5-latency/VERIFICATION.md`
   - Content: Pass/fail verdict, percentile breakdown, evidence links

### VERIFICATION.md Template

```markdown
# Criterion #5: LAN Latency Verification

**Date:** 2026-08-18  
**Status:** ✅ PASS / ❌ FAIL  
**Requirement:** p95 < 10ms (SRS §7.1)

## Results

| Metric | Result | Target | Status |
|--------|--------|--------|--------|
| p50 | X.X ms | < 5 ms | ✅/❌ |
| p95 | Y.Y ms | < 10 ms | ✅/❌ |
| p99 | Z.Z ms | < 15 ms | ✅/❌ |

## Evidence

- Criterion HTML: criterion-report.html
- Criterion JSON: estimates.json
- Benchmark log: benchmark-run-20260818-090000.log

## Verdict

[PASS/FAIL with reasoning]
```

---

## Contingency: Repeated Hang

**If Monday execution hangs during warmup (same as Aug 17 02:02):**

### Escalation to rust-backend-lead

**Message:**
```
Subject: P0: Criterion #5 Latency Benchmark - Repeated Warmup Hang

Benchmark hangs during warmup phase (both Aug 17 and Aug 18 attempts).

**Symptoms:**
- Server binds successfully to 127.0.0.1:18080
- WebSocket handshake completes
- PTY session created
- Warmup starts, but never completes (>5 min timeout)
- No error messages, no panic, silent hang

**Evidence:**
- Aug 17 log: tests/evidence/phase1/criterion-5-latency/short-test-retry-20260817-020224.log
- Aug 18 log: tests/evidence/phase1/criterion-5-latency/benchmark-run-20260818-090000.log

**Last successful operations (both attempts):**
- TLS handshake completed
- WebSocket handshake completed
- PTY session created
- Warmup phase started

**Request:** Debugging session to identify RTT measurement deadlock/timeout

**Impact:** Blocks 5/7 gate passage (Criterion #5 unverified)
```

### Debug Approach (if needed)

1. **Add timeout to RTT measurement loop**
2. **Enable verbose logging** (`RUST_LOG=trace`)
3. **Isolate components** (test WebSocket RTT without PTY)
4. **Profile** (sample where process is spending time during hang)

---

## Timeline Summary

| Time | Activity | Duration | Deliverable |
|------|----------|----------|-------------|
| **08:45** | Pre-flight checks | 15 min | Port clear, server test pass |
| **09:00** | Launch benchmark | - | Command started |
| **09:00-09:05** | Execution | 3-5 min | Results or hang detection |
| **09:05-09:30** | Extract results | 25 min | Percentiles, pass/fail verdict |
| **14:00** | Checkpoint report | - | One-sentence status to eng-director |
| **Tue EOD** | Final delivery | - | Evidence package + VERIFICATION.md |

---

## Success Criteria

**Monday 14:00:**
- ✅ Benchmark completed without hang
- ✅ p95 < 10ms (or clear FAIL/BORDERLINE verdict)
- ✅ One-sentence status delivered to eng-director

**Tuesday EOD:**
- ✅ Evidence package complete
- ✅ VERIFICATION.md with pass/fail verdict
- ✅ Ready for qa-lead gate review

---

**Status:** READY FOR MONDAY EXECUTION  
**Next action:** Monday 08:45 pre-flight checks  
**Owner:** performance-engineer
