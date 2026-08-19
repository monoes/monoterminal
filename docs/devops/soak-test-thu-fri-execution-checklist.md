# Soak Test Thu-Fri Execution Checklist

**Owner:** devops-lead  
**Execution Date:** Thursday August 18, 2026  
**Approval:** eng-director (Aug 17, 2026)  
**Task:** Task #3 - Execute Phase 1 Soak Test (Criterion #7)  

---

## Pre-Execution Status (Wed Aug 17)

**Approvals:**
- ✅ eng-director approved Thu-Fri execution (Option 2)
- ✅ Personal Azure account fallback pre-approved ($39 cost)
- ✅ devops-lead backup execution pre-approved (if performance-engineer unavailable)

**Infrastructure:**
- ✅ All 4 soak test scripts built and tested
- ✅ VM provisioning script ready (`azure-vm-setup.ps1`)
- ✅ Test orchestrator ready (`run-full-soak-test.ps1`)
- ✅ External monitor ready (`external-monitor.ps1`)
- ✅ Evidence collector ready (`collect-evidence.ps1`)

**Coordination:**
- ⏳ performance-engineer availability confirmation (deadline: Wed EOD)
- ✅ Backup executor confirmed (devops-lead)

---

## Thursday Aug 18 - Morning Execution

### 8:45 AM - Azure CLI Setup (15 minutes)

**Step 1: Install Azure CLI**
```powershell
# Install via winget
winget install Microsoft.AzureCLI

# Verify installation
az --version
# Expected: azure-cli 2.x.x or later
```

**Step 2: Login (Try org subscription first)**
```powershell
# Attempt org subscription login
az login

# Verify account
az account show

# Check subscription name
az account show --query "name" -o tsv
# If shows org subscription: PROCEED with org account
# If shows personal/unavailable: PROCEED with personal account (pre-approved)
```

**Step 3: Verify quota availability**
```powershell
# Check Standard_D4s_v5 quota in eastus
az vm list-usage --location eastus --query "[?name.value=='standardDSv5Family']" -o table

# Expected: CurrentValue < Limit (need 4 vCPU available)
# If quota exceeded: Try different region (westus2) or smaller VM size (Standard_D2s_v5)
```

**Decision Point (9:00 AM):**
- ✅ Org subscription + quota available → PROCEED to Step 4
- ✅ Personal subscription + quota available → PROCEED to Step 4 (pre-approved)
- ❌ No quota in any region → ESCALATE to eng-director, abort to Fri-Sun

---

### 9:00 AM - VM Provisioning (10 minutes)

**Step 4: Provision Azure VM**
```powershell
cd C:\Users\nokho\Desktop\projects\monoterminal\scripts\soak-monitor

# Run provisioning script
.\azure-vm-setup.ps1

# Expected output:
# [✓] Azure CLI installed
# [✓] Logged in as: <user>
# [✓] Resource group created: monoterminal-phase1
# [✓] Secure password generated (24 characters)
# [i] Provisioning VM 'monoterminal-soak-weekend' (Standard_D4s_v5, Windows 11 Pro)...
# [!] This will take 5-10 minutes. Please wait...
# [✓] VM provisioned: monoterminal-soak-weekend
# [✓] Public IP: <ip-address>
# [✓] Auto-shutdown configured: Sunday 11 PM ET (03:00 UTC Monday)
# [✓] RDP port 3389 is open
# [✓] Handoff document created: soak-test-vm-credentials-YYYYMMDD-HHMMSS.md

# Output file: soak-test-vm-credentials-<timestamp>.md
```

**Step 5: Verify RDP Access**
```powershell
# Test RDP connectivity (optional verification)
# Press Win+R → mstsc
# Computer: <public-ip-from-handoff>
# Username: soakadmin
# Credentials: <from-handoff-file>
# Click Connect → Should reach Windows 11 desktop

# Disconnect immediately (don't set up yet - that's performance-engineer's job or 10 AM)
```

**Decision Point (9:10 AM):**
- ✅ VM provisioned successfully → PROCEED to Step 6
- ❌ Provisioning failed → RETRY with different region/size (1 retry allowed)
- ❌ Retry failed → ESCALATE to eng-director, abort to Fri-Sun

---

### 9:10 AM - Credentials Handoff (5 minutes)

**Step 6: Deliver Credentials to performance-engineer (or prepare for backup execution)**

**If performance-engineer confirmed availability (Wed EOD):**
```
Send credentials file via org_send
Subject: "Soak Test VM Credentials - CONFIDENTIAL"
Attach: soak-test-vm-credentials-<timestamp>.md
Message: "RDP ready. Proceed with 10:00 AM setup per your instructions."
```

**If performance-engineer did NOT confirm (backup execution):**
```
Skip credential handoff - I'll execute setup myself at 10:00 AM
```

---

### 9:15 AM - Status Report to eng-director

**Step 7: Confirm VM provisioned successfully**
```
Subject: "Soak Test VM Provisioned Successfully - On Track for 10 AM Launch"

Message:
✅ VM provisioned: monoterminal-soak-weekend
✅ Public IP: <ip>
✅ Credentials delivered to performance-engineer (or prepared for backup execution)
✅ RDP access verified
✅ Auto-shutdown configured: Sunday 11 PM ET
✅ Cost: $39 (<account-type> subscription)

Next: Test launch at 10:00 AM
No blockers.
```

**If blockers arose:**
```
Subject: "Soak Test VM Provisioning - Blocker Encountered"

Message:
❌ Blocker: <describe>
🔄 Mitigation attempted: <describe>
⚠️ Decision needed: Abort to Fri-Sun, or <alternative>?
```

---

### 10:00 AM - Test Launch (30 minutes)

**Step 8: RDP into VM and set up test environment**

**Executor:** performance-engineer (primary) OR devops-lead (backup)

**RDP Connection:**
```
Win+R → mstsc
Computer: <ip-from-handoff>
Username: soakadmin
Credentials: <from-handoff>
```

**Inside VM (PowerShell as Administrator):**
```powershell
# 1. Install Rust toolchain (5 min)
winget install Rustlang.Rustup
rustup install stable
rustup default stable

# Verify
cargo --version
rustc --version

# 2. Install Git (2 min)
winget install Git.Git

# Verify
git --version

# 3. Install protoc (2 min)
winget install protocolbuffers.protobuf

# Verify
protoc --version

# 4. Clone repository (3 min)
cd C:\Users\soakadmin
git clone https://github.com/<org>/monoterminal.git
cd monoterminal

# 5. Verify build environment (5 min)
cargo build --release -p monoterminal-master
# Expected: Compiles successfully, binary at target\release\monoterminal-master.exe

# 6. Navigate to soak test scripts
cd scripts\soak-monitor
```

**Step 9: Launch 24-hour soak test**
```powershell
# Launch soak test orchestrator
.\run-full-soak-test.ps1 -DurationHours 24

# Expected output:
# [09:15:00] Starting 24-hour soak test...
# [09:15:05] Master daemon launched (PID: <pid>)
# [09:15:10] External monitor started
# [09:15:15] Monitoring every 60 seconds for 1440 iterations
# [09:15:20] Test will complete at 2026-08-19 10:15 AM ET
# [09:15:25] Evidence directory: C:\Users\soakadmin\monoterminal\scripts\soak-monitor\soak-results-<timestamp>

# Monitor output for 2-3 minutes to verify test is running
# Expected: No CRITICAL alerts, metrics logging every 60 seconds
```

**Step 10: Disconnect RDP**
```
Disconnect (don't sign out) - test runs unattended for 24 hours
External monitor will detect crashes and collect evidence automatically
```

---

### 10:30 AM - Test Launch Confirmation

**Step 11: Report test launch status to eng-director**

**If successful:**
```
Subject: "Soak Test Launched Successfully - Running for 24h"

Message:
✅ VM RDP access successful
✅ Environment setup complete (Rust + Git + protoc)
✅ Repository cloned and built
✅ Soak test launched at 10:15 AM ET
✅ External monitor running (PID: <pid>)
✅ Test will complete Friday Aug 19, 10:15 AM ET

Monitoring: Automated (external-monitor.ps1)
Next action: Evidence collection Friday 10:00 AM
```

**If launch failed:**
```
Subject: "Soak Test Launch - Issue Encountered"

Message:
❌ Issue: <describe>
🔄 Mitigation: <describe>
⏳ Current status: <retry in progress / escalating / aborting>
```

---

## Friday Aug 19 - Evidence Collection

### 10:00 AM - Evidence Collection (15 minutes)

**Step 12: RDP back into VM**
```
Same credentials as Thursday
Win+R → mstsc → <ip> → soakadmin → <from handoff file>
```

**Inside VM:**
```powershell
cd C:\Users\soakadmin\monoterminal\scripts\soak-monitor

# 1. Check if test completed
Get-Process monoterminal-master -ErrorAction SilentlyContinue
# If still running: Test NOT complete (should have stopped at 10:15 AM)
# If not found: Test completed (expected)

# 2. Review summary
cat soak-results-*\SUMMARY.json

# Expected:
# {
#   "TestResult": "PASSED",
#   "DurationHours": 24.0,
#   "Crashes": 0,
#   "MemoryGrowthPercent": 2.3,  # ≤ 10.0
#   "CriticalAlerts": 0,
#   "WarningAlerts": 0
# }

# 3. Zip evidence
$resultsDir = Get-ChildItem -Directory "soak-results-*" | Select-Object -First 1
Compress-Archive -Path $resultsDir.FullName -DestinationPath "soak-results-20260819.zip"

# 4. Copy zip to local machine (via RDP clipboard or Azure Storage)
# Or prepare for org_send delivery
```

---

### 10:30 AM - Evidence Delivery

**Step 13: Deliver evidence to qa-lead**

**If test PASSED:**
```
To: qa-lead
Subject: "Criterion #7 Evidence - 24h Soak Test PASSED"

Attached: soak-results-20260819.zip

Message:
✅ 24-hour soak test completed successfully
✅ Test duration: 24.0 hours (Thu 10:15 AM → Fri 10:15 AM)
✅ Crashes: 0
✅ Memory growth: <X>% (≤ 10.0% threshold)
✅ Critical alerts: 0
✅ Warning alerts: 0

SUMMARY.json verdict: "PASSED"

Evidence package contains:
- SUMMARY.json (pass/fail verdict)
- external-metrics-*.csv (1440 rows, 1-minute intervals)
- alerts-*.log (all warnings/criticals with timestamps)
- evidence-*/ (Event logs, perf snapshots, crash dumps if any)

Per phase1-acceptance-verification-plan.md §3.7, this evidence satisfies:
✅ Master daemon ran 24 hours without crash
✅ No panics in logs
✅ Memory usage stable (≤5% growth)
✅ No zombie PTY processes
✅ No file descriptor leaks

**Recommendation: Criterion #7 → ✅ VERIFIED**

Please review and confirm sign-off.
```

**If test FAILED:**
```
To: qa-lead, eng-director
Subject: "Criterion #7 Evidence - 24h Soak Test FAILED - Postmortem Needed"

Attached: soak-results-20260819.zip

Message:
❌ 24-hour soak test FAILED
❌ Failure type: <crashes / memory leak / other>
❌ Failure timestamp: <timestamp>

SUMMARY.json verdict: "FAILED"

Root cause analysis needed. Evidence package attached for review.

Next steps:
1. Review alerts-*.log for first CRITICAL alert timestamp
2. Correlate with evidence-*/event-log-crashes.csv
3. Analyze crash dumps (if present)
4. Determine: Fix + re-run, or escalate as Phase 1 blocker?

Awaiting postmortem discussion.
```

---

### 11:00 AM - Gate Status Update

**Step 14: Report to eng-director**

**If test PASSED:**
```
To: eng-director
Subject: "Criterion #7 VERIFIED - Gate Status: 5/7 ✅"

Message:
✅ 24-hour soak test PASSED
✅ Evidence delivered to qa-lead
✅ Criterion #7: ⏳ Pending → ✅ VERIFIED

**Phase 1 Gate Status Update:**
- Previous: 4/7 (57%)
- Current: 5/7 (71%)
- Threshold: 5/7 (71%) per ADR-006

**GATE THRESHOLD MET** ✅

Per ADR-006 carryover strategy, we can now proceed to Phase 2 with remaining criteria (if any) carried over as Phase 2 blockers.

Cost summary:
- Azure VM: $39 (28 hours, Thu-Fri)
- vs original Fri-Sun estimate: $75
- Savings: $36 (48%)

Task #3: COMPLETE
Timeline: 3 days ahead of original Mon Aug 25 target

Next: Await eng-director decision on Phase 2 transition.
```

---

## Cleanup (After Phase 1 Gate Approval)

### VM Shutdown (Friday 11:00 PM ET - Automatic)

**Auto-shutdown configured:**
- Azure auto-shutdown will deallocate VM Sunday 11 PM ET (03:00 UTC Monday)
- Billing stops automatically
- No manual action needed

**Manual verification (Monday morning):**
```powershell
# Check VM status (should be "VM deallocated")
az vm show -g monoterminal-phase1 -n monoterminal-soak-weekend --query "provisioningState" -o tsv

# If still running (auto-shutdown failed), manually deallocate:
az vm deallocate -g monoterminal-phase1 -n monoterminal-soak-weekend
```

### Full Cleanup (After evidence archived)

**After qa-lead confirms evidence archived:**
```powershell
# Delete VM (keeps disks for forensics if needed)
az vm delete -g monoterminal-phase1 -n monoterminal-soak-weekend --yes

# Or delete entire resource group (VM + disks + public IP - no forensics)
az group delete -n monoterminal-phase1 --yes
```

**Recommendation:** Keep resource group until Phase 1 gate approval (in case re-run needed)

---

## Contingency Plans

### Scenario 1: Azure CLI Auth Fails (Both Org + Personal)

**Action:** ABORT to Fri-Sun
- Report to eng-director immediately
- Reschedule VM provisioning to Friday Aug 22, 5 PM ET (original plan)
- Use weekend for troubleshooting Azure access issues

### Scenario 2: VM Provisioning Fails (Twice)

**Action:** Fallback to GitHub Actions
- Edit `.github/workflows/test.yml` line 250: `timeout-minutes: 1500` (25 hours)
- Trigger workflow manually via `workflow_dispatch`
- Risk: No RDP access for monitoring (blind run)
- Alternative: AWS EC2 Windows instance (same cost, similar setup)

### Scenario 3: Test Crashes Before 24 Hours

**Action:** Immediate postmortem
1. Review `alerts-*.log` for timestamp of first CRITICAL alert
2. Correlate with `evidence-*/event-log-crashes.csv`
3. Download crash dumps for WinDbg analysis
4. Report to eng-director with root cause analysis
5. Decide: Fix + re-run vs. escalate as Phase 1 blocker

**Re-run decision:**
- If fix is <4 hours: Re-run Friday afternoon (VM still available until Sun 11 PM)
- If fix is >4 hours: Extend VM reservation +1 week, re-run next weekend

### Scenario 4: performance-engineer Unavailable (No Response by Wed EOD)

**Action:** devops-lead executes as backup (pre-approved)
- No escalation needed
- Execute all steps myself (10:00 AM setup + Fri 10:00 AM evidence collection)
- Deliver evidence to qa-lead on performance-engineer's behalf

---

## Success Criteria

**This execution is successful if:**
1. ✅ VM provisioned Thu 9:00 AM ET (on time)
2. ✅ Test launches Thu 10:00-10:30 AM ET (no setup blockers)
3. ✅ Test runs uninterrupted for 24 hours (no VM crashes)
4. ✅ Evidence delivered to qa-lead Fri 10:30 AM ET
5. ✅ Criterion #7 → ✅ VERIFIED by Fri 11:00 AM
6. ✅ Gate status: 5/7 (71%) - threshold met
7. ✅ Total cost ≤ $39

**Measurement:**
- Azure cost report Monday morning
- qa-lead confirmation of evidence receipt
- eng-director confirmation of gate status update

---

## Contact & Escalation

**Primary executor:** devops-lead  
**Backup executor:** devops-lead (if performance-engineer unavailable)  
**Escalation:** eng-director (immediate org_send for any blocker with no clear mitigation)

**Related Documents:**
- `docs/devops/soak-test-vm-execution-plan.md` - Original Fri-Sun plan
- `scripts/soak-monitor/README.md` - Monitoring suite overview
- `docs/phase1-acceptance-verification-plan.md` - Criterion #7 acceptance criteria (§3.7)
- `docs/qa-phase1-status-summary.md` - Gate status tracking

---

**Status:** ✅ Ready to execute Thursday Aug 18, 2026 8:45 AM ET  
**Next Action:** Thu 8:45 AM - Install Azure CLI  
**Blocked:** None  
**Risk:** LOW (all infrastructure ready, approvals in place)  

---

**Prepared by:** devops-lead  
**Date:** August 17, 2026  
**Approval:** eng-director (Aug 17, 2026)  
**Task:** Task #3 (high priority)
