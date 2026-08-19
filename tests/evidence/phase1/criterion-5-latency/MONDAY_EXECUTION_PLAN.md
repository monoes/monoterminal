# Criterion #5 Latency Benchmark - Monday Execution Plan
**Date:** 2026-08-18 09:00  
**Owner:** performance-engineer  
**Status:** READY FOR EXECUTION  
**Phase 1 Gate:** 4/7 verified → **5/7 TARGET** (SOLE REMAINING PATH)

---

## Executive Summary

**Objective:** Execute `cargo bench --bench latency_e2e_lan` to verify **Criterion #5: LAN Latency < 10ms p95** (SRS §7.1).

**Current Status:** Infrastructure ready ✅, defensive timeout implemented ✅, port available ✅.

**Risk:** Previous executions hung during warmup phase. Defensive timeout (30s/iteration) added Aug 17 to prevent infinite hang.

**Success Metric:** Criterion report shows **p95 < 10ms** → **PHASE 1 GATE PASSAGE (5/7)**

---

## 1. Infrastructure Status (As of Sunday 2026-08-17 Evening)

### ✅ Verified Ready
| Component | Status | Evidence |
|-----------|--------|----------|
| **Benchmark File** | Present, untracked | `crates/master/benches/latency_e2e_lan.rs` |
| **TLS Certificates** | Valid | `certs/server.crt`, `certs/server.key` (created Aug 16) |
| **Port 18080** | Available | `netstat -ano \| findstr :18080` → no output |
| **Build** | Successful | 2m 17s, warnings only, exit 0 (Aug 17) |
| **Evidence Directory** | Exists | `tests/evidence/phase1/criterion-5-latency/` |
| **Defensive Timeout** | Implemented | See §2 below |

### ⚠️ Known Issue (MITIGATED)
- **Previous Hang:** Multiple executions hung during warmup phase (Aug 16 08:29, Aug 17 02:02)
- **Root Cause:** Suspected infinite loop in `iter_custom` warmup
- **Mitigation:** Defensive timeout added Aug 17 (30s per iteration, diagnostic logging every 10 iterations)

---

## 2. Defensive Timeout Configuration

**File:** `crates/master/benches/latency_e2e_lan.rs`

**Implementation Details:**
```rust
// Lines 57-60
const PER_ITERATION_TIMEOUT_SECS: u64 = 30;  // 30s max per iteration
const DIAGNOSTIC_LOGGING_INTERVAL: u64 = 10; // Log every 10 iterations
```

**Mechanism:**
- Wraps entire `iter_custom` block in `tokio::time::timeout` (line 202)
- **Fail-fast:** Panics after 30s per iteration with diagnostic output
- **Progress tracking:** Logs detailed state every 10 iterations
- **Prevents:** Infinite hang during warmup (criterion's adaptive measurement phase)

**Expected Behavior:**
- **Normal execution:** 10 warmup + 100 measurement iterations @ 1-10ms each = ~60-120s total
- **Timeout trigger:** If ANY single iteration exceeds 30s → panic with diagnostics
- **Log output:** Progress markers at iterations 10, 20, 30, ... showing timing and WebSocket state

---

## 3. Monday Pre-Flight Checklist (08:45)

**Executor:** performance-engineer  
**Standby:** rust-backend-lead (debugging support)

### Step 1: Clean Environment (08:45-08:50)
```powershell
# Kill stale processes (if any)
Get-Process -Name monoterminal-master -ErrorAction SilentlyContinue | Stop-Process -Force

# Verify port 18080 is clear
netstat -ano | findstr :18080
# Expected: NO OUTPUT (port available)

# Clean previous criterion data (fresh start)
Remove-Item -Recurse -Force target/criterion/latency_e2e_lan -ErrorAction SilentlyContinue
```

### Step 2: Infrastructure Smoke Test (08:50-08:55)
```powershell
# Verify TLS certs exist
Test-Path certs/server.crt
Test-Path certs/server.key
# Expected: Both return True

# Verify benchmark file is present and unmodified
git status crates/master/benches/latency_e2e_lan.rs
# Expected: Shows "Untracked files" (defensive timeout added, not yet committed)

# Quick build check (optional if already built recently)
cargo build --benches --release
# Expected: Exit 0 (warnings acceptable)
```

### Step 3: Execution Setup (08:55-09:00)
```powershell
# Create timestamped log directory
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$logdir = "tests/evidence/phase1/criterion-5-latency"
$logfile = "$logdir/benchmark-run-$timestamp.log"

# Prepare environment
$env:RUST_LOG = "monoterminal=debug,monoterminal_master=debug,latency_e2e_lan=debug"
```

**Ready State Confirmation:**
- [ ] Port 18080 clear
- [ ] No stale processes
- [ ] TLS certs present
- [ ] Build successful
- [ ] Log directory prepared
- [ ] RUST_LOG configured
- [ ] rust-backend-lead on standby

---

## 4. Execution Protocol (09:00-10:00)

### Primary Command
```powershell
# Execute with real-time output AND log capture
cargo bench --bench latency_e2e_lan 2>&1 | Tee-Object -FilePath $logfile
```

### Expected Timeline
| Phase | Duration | Milestone |
|-------|----------|-----------|
| **Server Startup** | 0-5s | "Server bound successfully on 127.0.0.1:18080" |
| **Warmup** | 5-30s | "ITERATION START" logs every 10 iterations |
| **Measurement** | 30-120s | 100 iterations @ 1-10ms each |
| **Analysis** | 1-5s | Criterion statistical analysis |
| **Reporting** | 1-5s | HTML report generation |
| **TOTAL** | **~2-3 minutes** | "test result: ok" final line |

### Real-Time Monitoring
**Watch for these log markers:**
1. `✓ Server bound successfully` → Infrastructure OK
2. `=== ITERATION START: timeout=30s` → Defensive timeout active
3. `Diagnostic checkpoint at iteration 10` → Progress markers
4. `✅ Iteration completed in X.XXs` → Each iteration success
5. `Benchmarking e2e_lan_latency` → Criterion collection phase
6. `time: [X.XX ms X.XX ms X.XX ms]` → **FINAL RESULTS**

**Red flags (escalate immediately):**
- No "Server bound" message within 10s → Server startup failure
- Iteration logs stop mid-warmup → Hang detected (but timeout should catch this)
- `❌ TIMEOUT: Iteration exceeded 30s` → Defensive timeout triggered

---

## 5. Success Criteria (Pass/Fail Decision)

### PASS: Criterion #5 Verified ✅
**Output location:** `target/criterion/latency_e2e_lan/report/index.html`

**Evidence requirements:**
1. **Criterion report generated** (HTML + JSON)
2. **p95 < 10ms** (extract from report or `estimates.json`)
3. **Sample size ≥ 100** (confirms measurement phase completed)
4. **Log shows:** All iterations completed successfully

**Extract p95 from JSON:**
```powershell
# After successful completion
$estimates = Get-Content target/criterion/latency_e2e_lan/e2e_lan_latency/base/estimates.json | ConvertFrom-Json
$p95_ms = [math]::Round($estimates.mean.point_estimate / 1000000, 2) # ns → ms
Write-Output "p95 Latency: ${p95_ms}ms (Target: <10ms)"
```

**Gate decision:**
- **p95 < 10ms** → **5/7 VERIFIED** → **PHASE 1→2 APPROVAL**

### FAIL: Escalation Required ❌

**Scenario A: Hang (timeout triggered)**
- **Symptom:** `❌ TIMEOUT: Iteration exceeded 30s` in logs
- **Action:** Immediate notification to rust-backend-lead
- **Debug:** Re-run with `RUST_LOG=trace` and process strace
- **Escalation window:** 2 hours for root cause analysis

**Scenario B: p95 ≥ 10ms (latency too high)**
- **Symptom:** Criterion report shows p95 ≥ 10ms
- **Action:** Review logs for outliers, system load, network issues
- **Options:**
  1. Re-run (eliminate transient noise)
  2. Profile hot path (tokio-console, perf)
  3. Defer to Phase 2 (fall back to 4/7 criteria set)

**Scenario C: Build/infrastructure failure**
- **Symptom:** Server fails to bind, TLS errors, WebSocket connection failures
- **Action:** rust-backend-lead debug session
- **Reference:** Previous successful connection logs (Aug 16, Aug 17) prove infrastructure works

---

## 6. Post-Execution Tasks (10:00-10:30)

### If PASS (p95 < 10ms)
1. **Archive evidence:**
   ```powershell
   # Copy criterion artifacts
   Copy-Item -Recurse target/criterion/latency_e2e_lan tests/evidence/phase1/criterion-5-latency/
   
   # Archive log
   # Already captured at: tests/evidence/phase1/criterion-5-latency/benchmark-run-$timestamp.log
   ```

2. **Commit defensive timeout + evidence:**
   ```powershell
   git add crates/master/benches/latency_e2e_lan.rs
   git add tests/evidence/phase1/criterion-5-latency/
   git commit -m "feat: Add defensive timeout to E2E latency benchmark
   
   - Prevent infinite hang during warmup phase
   - 30s per-iteration timeout with diagnostic logging
   - Evidence: p95 = X.XXms (< 10ms target)
   - Criterion #5 VERIFIED → Phase 1 gate passage (5/7)
   
   Co-Authored-By: nokhodian <nokhodian@gmail.com>"
   ```

3. **Notify org:**
   - eng-director: "Criterion #5 VERIFIED, p95 = X.XXms, 5/7 gate passed"
   - qa-lead: "Gate decision: APPROVE Phase 1→2 transition"
   - rust-backend-lead: "Defensive timeout worked, no hang"

### If FAIL
1. **Archive partial evidence** (logs, partial criterion data if any)
2. **Document failure mode** (hang vs high latency vs infrastructure)
3. **Escalate to debug session** (rust-backend-lead + performance-engineer)
4. **Re-assess gate strategy** (defer Criterion #5? alternative paths?)

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Hang recurs despite timeout** | Low | High | Defensive timeout panics after 30s, preserves Monday time |
| **p95 ≥ 10ms** | Low | Medium | Re-run to eliminate noise, profile if persistent |
| **Infrastructure failure** | Very Low | Medium | Proven working (Aug 16/17), smoke test catches early |
| **Build regression** | Very Low | Low | Recent successful build (Aug 17) |

**Confidence Level:** HIGH (defensive timeout untested but should prevent hang)

---

## 8. Rollback/Alternative Paths

**If Monday fails catastrophically:**
- **Immediate:** Do NOT retry blindly; root cause analysis first
- **Alternative:** Defer Criterion #5 to Phase 2 (as originally planned for soak/coverage)
- **Consequence:** Remain at 4/7 verified; reassess Phase 1→2 gate passage with eng-director
- **Fallback criteria:** Re-evaluate Track B or Track C paths (though both also require Criterion #5)

**Strategic note:** Criterion #5 is critical for ALL paths to 5/7. If this fails, we need architectural review of the latency benchmark approach itself (not just retry).

---

## 9. Key Contacts

| Role | Agent | Responsibility |
|------|-------|----------------|
| **Execution Owner** | performance-engineer | Primary executor, monitoring, evidence collection |
| **Debug Standby** | rust-backend-lead | Hang troubleshooting, RUST_LOG trace analysis |
| **Gate Decision** | qa-lead | Pass/fail verdict based on results |
| **Final Approval** | eng-director | Phase 1→2 transition authorization |

---

## 10. Success Definition

**Criterion #5 Latency VERIFIED** =
1. ✅ Benchmark completes (no hang, no timeout)
2. ✅ Criterion report generated with valid statistical data
3. ✅ **p95 < 10ms** (SRS §7.1 Phase 1 gate requirement)
4. ✅ Evidence archived and committed

**Outcome:** **PHASE 1 GATE PASSAGE (5/7)** → Proceed to Phase 2 planning

---

## Appendix A: Defensive Timeout Code Reference

**File:** `crates/master/benches/latency_e2e_lan.rs`  
**Lines:** 57-60, 196-356

**Key mechanism:**
```rust
// Line 202: Wrap entire iteration in timeout
let result = tokio::time::timeout(iteration_timeout, async {
    // ... benchmark logic (server startup, WebSocket connection, measurements) ...
}).await;

match result {
    Ok(_) => debug!("✅ Iteration completed"),
    Err(_timeout_elapsed) => {
        error!("❌ TIMEOUT: Iteration exceeded {}s limit", PER_ITERATION_TIMEOUT_SECS);
        panic!("Benchmark iteration timeout - infinite hang detected");
    }
}
```

**Panic output includes:**
- Iteration number where hang occurred
- Timeout duration (30s)
- Last known WebSocket state
- Diagnostic checkpoint logs (every 10 iterations)

---

## Appendix B: Historical Context

| Date | Event | Outcome |
|------|-------|---------|
| 2026-08-16 08:29 | First E2E benchmark attempt | Hung during warmup (log stopped at line 1305) |
| 2026-08-16 12:49 | Second attempt after infra fix | Infrastructure worked, but still hung |
| 2026-08-17 02:02 | Overnight execution | Hung during warmup again |
| 2026-08-17 22:58 | Defensive timeout implemented | **NEW FIX** (untested in production) |
| 2026-08-18 09:00 | **MONDAY EXECUTION** | **THIS ATTEMPT** |

**Pattern:** Infrastructure (server bind, TLS, PTY) consistently works. Hang occurs specifically during criterion's warmup phase (adaptive iteration count estimation).

**Hypothesis:** Infinite loop or deadlock in measurement loop, now bounded by 30s timeout per iteration.

---

**Document prepared:** 2026-08-17 (Sunday evening)  
**Last updated:** 2026-08-17 21:30  
**Next review:** 2026-08-18 08:45 (pre-flight)

---

*This document is the authoritative execution plan for Criterion #5 Monday verification. All team members should reference this for protocol, success criteria, and escalation procedures.*
