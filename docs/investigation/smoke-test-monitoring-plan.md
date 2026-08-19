# Smoke Test Monitoring Plan - Monday 2026-08-17

**Test ID:** smoke-test-1h-verification  
**Start Time:** Monday 02:35  
**PID:** 16308  
**Owner:** rust-backend-lead

---

## Test Configuration

**Duration:** 60 minutes  
**Sample Interval:** 5 minutes  
**Total Samples:** 12

**Baseline (t=0):**
- Working Set: 16.14 MB
- Private Memory: 21.3 MB
- Handle Count: 187

---

## Monitoring Checkpoints

### Checkpoint 1: t=5min (First Sample)
**Purpose:** Verify no immediate leak  
**Expected:** <1% growth from baseline  
**Action:** Document first sample, continue monitoring

### Checkpoint 2: t=15min (3 Samples)
**Purpose:** Trend analysis  
**Expected:** Linear growth <0.5% per 5min  
**Action:** Report interim results to eng-director

### Checkpoint 3: t=30min (6 Samples - Midpoint)
**Purpose:** Stability verification  
**Expected:** <2% cumulative growth  
**Action:** If anomaly detected, alert immediately

### Checkpoint 4: t=60min (12 Samples - Completion)
**Purpose:** Final verdict  
**Expected:** <1% cumulative WS growth, <5% PM growth, ≤5 handle delta  
**Action:** Begin analysis phase

---

## Pass/Fail Criteria

### PASS Criteria (All must be true)
- ✅ WS growth <1% over 60 minutes
- ✅ PM growth <5% over 60 minutes
- ✅ Handle delta ≤5 handles
- ✅ No crashes/exits
- ✅ No error messages in logs

### FAIL Criteria (Any one triggers FAIL)
- ❌ WS growth ≥1%
- ❌ PM growth ≥5%
- ❌ Handle delta >5
- ❌ Process crash
- ❌ Exponential growth pattern (even if below thresholds)

---

## Analysis Phase (t=60 to t=90)

### 1. Data Collection (5 minutes)
- Extract all 12 samples from test output
- Calculate growth rates and deltas
- Generate trend charts (if needed)

### 2. Verdict Determination (5 minutes)
- Apply pass/fail criteria
- Document any anomalies
- Compare to original failed smoke test

### 3. Report Generation (20 minutes)
- **If PASS:**
  - Declare memory leak FIXED
  - Document fix commit (8d3259c)
  - Coordinate Wed-Thu soak test with devops-lead
  - Update VERIFICATION.md
  
- **If FAIL:**
  - Analyze failure pattern vs original
  - Identify top suspect from investigation doc
  - Outline next investigation steps
  - Coordinate deeper analysis session

---

## Coordination Plan (If PASS)

### Immediate Notifications (Monday 2 PM)
1. **eng-director:** Smoke test PASSED - leak confirmed fixed
2. **devops-lead:** Wed-Thu soak test execution planning
3. **qa-lead:** Evidence timeline update
4. **performance-engineer:** 24h monitoring coordination

### Wed-Thu Soak Test Execution Plan

**Execution Details:**
- **When:** Wednesday 2026-08-19 9:00 AM
- **Duration:** 24 hours (until Thursday 9:00 AM)
- **Executor:** rust-backend-lead (primary), performance-engineer (standby)
- **Machine:** Local workstation (same as smoke test)
- **Monitoring:** External PowerShell script (`run-full-soak-test.ps1`)

**Coordination:**
- **devops-lead:** Handoff execution plan, evidence delivery format
- **performance-engineer:** Alert protocol if test fails mid-run
- **qa-lead:** Evidence delivery timeline (Thu 9:30 AM)

**Fallback:**
- **Weekend Azure VM:** Keep as fallback ($75 insurance)
- **If Wed-Thu fails:** Execute Sat-Sun on Azure VM as planned

---

## Evidence Files

**Smoke Test:**
- Output log: Task output file (will be archived)
- Analysis: This monitoring plan + final report

**Final Report (Monday 2 PM):**
- `docs/investigation/smoke-test-results-2026-08-17.md`
- Will include:
  - All 12 sample data points
  - Growth rate analysis
  - Pass/fail verdict
  - Coordination plan for next steps

---

**Status:** ⏳ IN PROGRESS - Awaiting first sample at t=5min
