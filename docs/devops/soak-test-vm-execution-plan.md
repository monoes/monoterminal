# Phase 1 Soak Test - Infrastructure Execution Plan

**Owner:** devops-lead  
**Purpose:** Provide dedicated Windows machine for 24-hour soak test (Criterion #7)  
**Timeline:** August 22-24, 2026 (Friday-Sunday)  

---

## Decision: Azure Windows 11 VM

**Why Azure over GitHub Actions:**
- GitHub Actions has job timeout limits (6 hours free tier, configurable on paid tier but risky)
- Need 24+ hours uninterrupted runtime
- Need RDP access for real-time monitoring
- Azure VM has no job timeout (only pay for runtime)
- Auto-shutdown prevents runaway costs

**Why Azure over physical hardware:**
- No procurement delay (5 minutes to provision)
- No conflicts with other users/tests
- Pay-as-you-go (~$75 for weekend)
- Auto-shutdown guarantee (no manual cleanup risk)

---

## Timeline

| Date/Time | Event | Owner | Action |
|-----------|-------|-------|--------|
| **Friday Aug 22, 5:00 PM ET** | VM provisioning start | devops-lead | Run `azure-vm-setup.ps1` |
| **Friday Aug 22, 5:15 PM ET** | VM ready | devops-lead | Verify RDP access |
| **Friday Aug 22, 6:00 PM ET** | Credentials handoff | devops-lead | Send credentials file to performance-engineer |
| **Saturday Aug 23, 9:00 AM ET** | Soak test launch | performance-engineer | RDP in, run `run-full-soak-test.ps1 -DurationHours 24` |
| **Sunday Aug 24, 9:00 AM ET** | Soak test completes | Auto | Test exits, evidence collected |
| **Sunday Aug 24, 9:30 AM ET** | Evidence delivery | performance-engineer | Zip `soak-results/` and send to qa-lead |
| **Sunday Aug 24, 11:00 PM ET** | VM auto-shutdown | Auto | Azure deallocates VM (stops billing) |

---

## VM Specifications

| Spec | Value | Rationale |
|------|-------|-----------|
| **VM Size** | Standard_D4s_v5 | 4 vCPU, 16GB RAM (enough for daemon + monitor + headroom) |
| **OS** | Windows 11 Pro | Build 22000+ (ConPTY native, exceeds Win10 1809+ requirement) |
| **Region** | East US | Lowest latency for US-based team |
| **Storage** | 100 GB SSD | Default OS disk (more than enough for logs) |
| **GPU** | DirectX 12 capable | Integrated graphics via D-series (meets Phase 1 req) |
| **Network** | Standard public IP | RDP access (port 3389) |
| **Auto-shutdown** | Sunday 11 PM ET | Prevents runaway costs if manual cleanup forgotten |

**Cost Estimate:**
- Hourly rate: ~$1.40/hour (Standard_D4s_v5 in East US)
- Total runtime: 54 hours (Friday 6 PM → Sunday 11 PM)
- **Total cost: ~$75** (well within $50-100/month CI/CD budget per SRS §6.2)

---

## Setup Script

**Location:** `scripts/soak-monitor/azure-vm-setup.ps1`

**Usage:**
```powershell
# Default (creates resource group + VM)
.\azure-vm-setup.ps1

# Custom resource group name
.\azure-vm-setup.ps1 -ResourceGroup "my-rg" -VMName "my-vm"

# Skip resource group creation (if already exists)
.\azure-vm-setup.ps1 -SkipResourceGroupCreation
```

**What it does:**
1. Verifies Azure CLI installed and logged in
2. Creates resource group `monoterminal-phase1` (if needed)
3. Generates secure 24-character admin password
4. Provisions Windows 11 VM with RDP access
5. Configures auto-shutdown for Sunday 11 PM ET
6. Generates handoff document with credentials

**Output:**
- Handoff file: `soak-test-vm-credentials-YYYYMMDD-HHMMSS.md`
- Contains: RDP IP, username, password, setup instructions

---

## Handoff to performance-engineer

**What they receive:**
- Credentials file (secure channel only - NOT via email/Slack)
- RDP IP address + username + password
- Setup instructions (install Rust, Git, protoc, clone repo)
- Execution command: `run-full-soak-test.ps1 -DurationHours 24`

**What they need to do:**
1. **Friday 6 PM:** Receive credentials file
2. **Saturday 9 AM:** RDP into VM
3. **Saturday 9-9:15 AM:** Install Rust + Git + protoc + clone repo
4. **Saturday 9:15 AM:** Run `.\scripts\soak-monitor\run-full-soak-test.ps1 -DurationHours 24`
5. **Sunday 9 AM:** Test completes, review SUMMARY.json
6. **Sunday 9:30 AM:** Zip `soak-results/` and send to qa-lead

**Total hands-on time for performance-engineer:** ~30 minutes (setup) + 15 minutes (evidence collection)

---

## Monitoring Infrastructure

**Scripts (already built):**
- `scripts/soak-monitor/run-full-soak-test.ps1` - Orchestrator (one command)
- `scripts/soak-monitor/external-monitor.ps1` - Crash detection + metrics
- `scripts/soak-monitor/collect-evidence.ps1` - Forensics automation

**What gets monitored:**
- Process crashes (immediate CRITICAL alert)
- Memory growth (WARNING at 8%, CRITICAL at 10%)
- Handle leaks (WARNING at 50% growth)
- CPU usage (WARNING at sustained 80%+)
- Network connections (TCP + WebSocket count)

**Evidence collected:**
- `SUMMARY.json` - Pass/fail verdict + metrics
- `external-metrics-*.csv` - 1440 rows (1 per minute for 24h)
- `alerts-*.log` - All warnings/criticals with timestamps
- `evidence-*/event-log-*.csv` - Windows Event Logs (Application, System, crashes)
- `evidence-*/crash-dumps/` - WinDbg-ready dumps (if process crashed)

---

## Risk Mitigation

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| **VM provisioning fails** | HIGH | Retry with different region/size; fallback to GitHub Actions with extended timeout | Pre-tested script |
| **Auto-shutdown fails** | MEDIUM | Manual cleanup script provided; Azure cost alerts configured | Monitored |
| **RDP access blocked** | HIGH | NSG rule for RDP included in provisioning; public IP is Standard SKU | Built-in |
| **Test crashes VM** | HIGH | Azure monitoring alerts if VM stops; snapshot taken before test | Automated |
| **Performance-engineer unavailable** | HIGH | Backup: devops-lead can execute test if needed | Documented |
| **Cost overrun** | LOW | Auto-shutdown at 54 hours; max cost $75 even if runs full week (unlikely) | Capped |

---

## Evidence Delivery to qa-lead

**What qa-lead receives Sunday 9:30 AM:**
1. Zip file: `soak-results-YYYYMMDD-HHMMSS.zip`
2. Contains:
   - `SUMMARY.json` - Pass/fail verdict
   - `external-metrics-*.csv` - 1440 rows
   - `alerts-*.log` - All alerts
   - `evidence-*/` - Event logs, perf snapshots, crash dumps

**Criterion #7 sign-off requirements (from qa-phase1-status-summary.md):**
- ✅ `SUMMARY.json` → `"TestResult": "PASSED"`
- ✅ `external-metrics-*.csv` → Final `MemoryGrowth%` ≤ 10.0
- ✅ `alerts-*.log` → Zero CRITICAL alerts
- ✅ `evidence-*/event-log-crashes.csv` → Empty (no crashes)

**If all pass:** qa-lead marks Criterion #7 as ✅ Verified  
**If any fail:** qa-lead reviews alerts log + crash dumps → postmortem → eng-director escalation

---

## Cost Tracking

**Budget:** $50-100/month CI/CD (SRS §6.2)  
**This weekend:** ~$75 (one-time expense for Phase 1 gate testing)  
**Remaining budget:** ~$25-100 for August (depending on baseline)

**August 2026 projected spend:**
- Soak test VM: $75 (this weekend)
- GitHub Actions: $0 (free tier sufficient for PR checks)
- **Total: ~$75** (within budget)

**Ongoing costs (Phase 2+):**
- GitHub Actions minutes: Free tier → 2000 minutes/month (sufficient for Phase 1)
- Paid tier upgrade: $4/user/month (if needed for extended timeout)
- Code signing certificate: $200-400/year (one-time, amortized)

---

## Cleanup Checklist

**Auto-cleanup (Sunday 11 PM ET):**
- ✅ VM auto-shutdown configured
- ✅ Azure cost alerts enabled

**Manual verification (Monday morning):**
```powershell
# Check VM status (should be "VM deallocated")
az vm show -g monoterminal-phase1 -n monoterminal-soak-weekend --query "provisioningState" -o tsv

# If still running, manually deallocate
az vm deallocate -g monoterminal-phase1 -n monoterminal-soak-weekend
```

**Full cleanup (after evidence archived):**
```powershell
# Delete VM (keeps disks)
az vm delete -g monoterminal-phase1 -n monoterminal-soak-weekend --yes

# Delete entire resource group (VM + disks + public IP)
az group delete -n monoterminal-phase1 --yes
```

**Recommendation:** Keep resource group until Phase 1 gate approval (in case re-run needed)

---

## Contingency Plans

### Scenario 1: Azure provisioning fails Friday 5 PM

**Fallback Option 1:** GitHub Actions with extended timeout
- Edit `.github/workflows/test.yml` line 250: `timeout-minutes: 1500` (25 hours)
- Verify organization has GitHub Actions paid tier (required for >6h jobs)
- Trigger workflow manually via `workflow_dispatch`
- **Risk:** No RDP access for monitoring (blind run)

**Fallback Option 2:** AWS EC2 Windows instance
- Similar setup to Azure (t3.xlarge Windows 2022)
- Cost: ~$80 for 54 hours (similar to Azure)
- Same RDP access + auto-shutdown capabilities

**Decision timeline:** If Azure fails by 6 PM Friday, switch to Fallback Option 1 immediately (no time for AWS setup)

### Scenario 2: Performance-engineer unavailable Saturday morning

**Backup executor:** devops-lead
- I have credentials + setup knowledge
- Can execute soak test myself if needed
- Deliver evidence to qa-lead on performance-engineer's behalf

### Scenario 3: Test fails (crashes before 24 hours)

**Immediate actions:**
1. Review `alerts-*.log` for timestamp of first CRITICAL alert
2. Correlate with `evidence-*/event-log-crashes.csv`
3. If crash dumps exist: Download for WinDbg analysis
4. Report to eng-director with root cause analysis
5. Decide: Fix + re-run vs. escalate as Phase 1 blocker

**Re-run decision:**
- If fix is <4 hours: Re-run Sunday afternoon (VM still available)
- If fix is >4 hours: Extend VM reservation +1 week, re-run next weekend

---

## Success Criteria

**This infrastructure is successful if:**
1. ✅ VM provisioned Friday 6 PM ET (on time)
2. ✅ performance-engineer receives credentials Friday 6 PM ET
3. ✅ Soak test launches Saturday 9 AM ET (no setup blockers)
4. ✅ Test runs uninterrupted for 24 hours (no VM crashes)
5. ✅ Evidence delivered to qa-lead Sunday 9:30 AM ET
6. ✅ VM auto-stops Sunday 11 PM ET (no cost overrun)
7. ✅ Total cost ≤ $75

**Measurement:**
- Azure cost report Monday morning
- qa-lead confirmation of evidence receipt

---

## Post-Execution Review

**After Phase 1 gate approval, document:**
- Actual cost vs. estimate ($75)
- Timeline accuracy (did we hit Friday 6 PM handoff?)
- Any manual interventions needed
- Lessons learned for Phase 2/3 multi-platform testing

**Improvements for Phase 2:**
- Consider GitHub Actions self-hosted runners (if recurring tests needed)
- Automate Rust + Git + protoc installation (custom Azure VM image)
- Set up Grafana dashboard for live soak test monitoring

---

## Questions & Approvals

**Decision Authority:** devops-lead (infrastructure), eng-director (budget)

**Approval Required:** None (within delegated budget authority per SRS §6.2)

**Questions:** Contact devops-lead

**Related Documents:**
- `scripts/soak-monitor/README.md` - Monitoring suite overview
- `tests/evidence/phase1/criterion-7-soak/VERIFICATION.md` - Acceptance criteria
- `docs/qa-phase1-status-summary.md` - Risk register line 135

---

**Status:** ✅ Ready to execute Friday Aug 22, 2026 5 PM ET  
**Next Action:** devops-lead runs `azure-vm-setup.ps1` Friday 5 PM  
**Blocked:** None  
**Risk:** LOW (all mitigations in place)
