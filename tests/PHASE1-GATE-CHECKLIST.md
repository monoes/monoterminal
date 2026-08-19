# Phase 1 Acceptance Gate Checklist

**Last Updated:** 2026-08-15  
**Gate Keeper:** qa-lead  
**Phase 2 Entry:** ALL items must be ✅  

---

## Gate Status: 🔴 NOT READY (0/7 verified)

**Phase 1 → Phase 2 transition is BLOCKED until all criteria show ✅**

---

## Acceptance Criteria Verification

### ✅ = VERIFIED | 🔄 = IN PROGRESS | ⏳ = PENDING | ❌ = FAILED

---

### Criterion #1: 60 FPS Master Rendering

**Status:** ⏳ PENDING  
**Owner:** performance-engineer (task-17)  
**Target:** p50 ≥ 60 FPS, p95 ≥ 58 FPS on Windows 10 1809+ and Windows 11

**Evidence Checklist:**
- [ ] Performance test report from task-17
- [ ] FPS histogram screenshots (Windows 10 + Windows 11)
- [ ] Criterion.rs benchmark results
- [ ] Manual verification on both OS versions

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Awaiting task-17 completion

---

### Criterion #2: Web Client Mobile Browser Usability

**Status:** ⏳ PENDING  
**Owner:** test-engineer-e2e (task-14, task-16)  
**Target:** Full workflow on iPhone Safari + Android Chrome on same LAN

**Evidence Checklist:**
- [ ] E2E test report (web/e2e/mobile-browser.spec.ts)
- [ ] Manual test checklist completed (iOS Safari)
- [ ] Manual test checklist completed (Android Chrome)
- [ ] Video recording on real iPhone device
- [ ] Video recording on real Android device
- [ ] PWA installability verified on both platforms

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Requires physical iPhone + Android device (procure by Week 8)

---

### Criterion #3: Monomind Detection & Dismissal

**Status:** ⏳ PENDING  
**Owner:** test-engineer-e2e (task-16)  
**Target:** Suggestion fires for no `.monomind/`, stays dismissed

**Evidence Checklist:**
- [ ] Integration test report (crates/monomind-bridge)
- [ ] E2E test screenshots showing suggestion UI
- [ ] Manual verification on 3 different projects
- [ ] Scenario A verified: Suggestion fires without `.monomind/`
- [ ] Scenario B verified: Dismiss persists across reloads
- [ ] Scenario C verified: No suggestion with `.monomind/`

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Depends on task-16 (monomind-bridge integration tests)

---

### Criterion #4: Embedded Dashboard

**Status:** ⏳ PENDING  
**Owner:** test-engineer-e2e (task-16, depends on task-12)  
**Target:** Dashboard in web client, no separate service/port/auth

**Evidence Checklist:**
- [ ] E2E test report (web/e2e/dashboard-embedded.spec.ts)
- [ ] Screenshot showing dashboard panel in web client
- [ ] Network trace (HAR file) showing single WebSocket connection
- [ ] Manual verification: Dashboard opens without separate login
- [ ] Dashboard shows: org name, agents, run status, health check, upgrade button

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Blocked on task-12 (dashboard implementation)

---

### Criterion #5: <10ms Local Latency (LAN p95)

**Status:** ⏳ PENDING  
**Owner:** performance-engineer (task-17)  
**Target:** p95 < 10ms round-trip time on same LAN

**Evidence Checklist:**
- [ ] Criterion.rs benchmark report (p50/p95/p99)
- [ ] Wireshark PCAP file (10,000 samples)
- [ ] Wireshark statistics screenshot
- [ ] Latency histogram graph
- [ ] Manual tcpdump verification (p95 < 10ms confirmed)

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Test on local LAN only (disable internet during test)

---

### Criterion #6: 70% Test Coverage

**Status:** 🔄 IN PROGRESS  
**Owner:** test-engineer-unit (task-15)  
**Target:** ≥70% total workspace coverage via cargo-tarpaulin

**Evidence Checklist:**
- [ ] Codecov report URL (≥70% shown)
- [ ] Coverage HTML report (coverage/index.html)
- [ ] Per-crate coverage breakdown table
- [ ] CI workflow passing with coverage gate
- [ ] Core modules (pty/, session/, auth/) ≥85%
- [ ] Coverage badge on README shows ≥70%

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** task-15 currently running, target completion Week 9

---

### Criterion #7: 24-Hour Soak Test (Zero Crashes)

**Status:** ⏳ PENDING  
**Owner:** performance-engineer (task-17)  
**Target:** Zero crashes, panics, or leaks over 24 hours

**Evidence Checklist:**
- [ ] 24-hour test log (tests/soak/24h-test.log)
- [ ] Memory usage graph (stable, ≤5% growth)
- [ ] Windows Event Viewer screenshot (no crash events)
- [ ] Test assertion passed: `daemon.is_running() == true`
- [ ] No zombie PTY processes
- [ ] No file descriptor leaks

**Verification Date:** _Not verified_  
**Verified By:** _Pending_  
**Notes:** Requires dedicated Windows test machine (reserve 2 weeks before Week 11)

---

## Pre-Approval Checklist (All Must Be ✅)

- [ ] All 7 acceptance criteria verified (✅ status)
- [ ] All evidence collected and stored in tests/evidence/phase1/
- [ ] No critical (P0/P1) bugs open
- [ ] CI pipeline green for 5 consecutive days
- [ ] Manual smoke test passed on fresh Windows 10 + Windows 11
- [ ] Mobile browser tests passed on real iPhone + Android devices
- [ ] Soak test completed with zero crashes
- [ ] Coverage stable at ≥70% with no regression

---

## Final Gate Approval

**QA Lead Sign-Off:**
- [ ] All pre-approval checklist items ✅
- [ ] Final acceptance report compiled
- [ ] Recommendation: APPROVE / BLOCK Phase 2

**Signature:** _________________________  
**Date:** _________________________

---

**eng-director Approval:**
- [ ] QA Lead sign-off reviewed
- [ ] Final decision: APPROVE / BLOCK / RISK-ACCEPT Phase 2

**Signature:** _________________________  
**Date:** _________________________

---

## Risk Acceptance (If Applicable)

If any criterion is ❌ FAILED but Phase 2 approved:

**Failed Criterion:** _________________________  
**Risk Accepted By:** _________________________  
**Mitigation Plan:** _________________________  
**Documentation:** _________________________

---

## Revision History

| Date | Change | Updated By |
|------|--------|------------|
| 2026-08-15 | Initial checklist created | qa-lead |
| | | |
| | | |

---

**Reference:** docs/phase1-acceptance-verification-plan.md (detailed procedures)
