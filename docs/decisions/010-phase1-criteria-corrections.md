# ADR-010: Phase 1 Acceptance Criteria Corrections

**Status:** ✅ APPROVED (eng-director, 2026-08-16)  
**Date:** 2026-08-16  
**Deciders:** product-owner, eng-director  
**SRS Reference:** §7.1 (Phase 1 Roadmap, lines 1457-1463)  
**Corrects:** ADR-006 (file: 006-phase1-gate-passage-5-of-7.md)  
**Phase:** Phase 1 Gate Validation

---

## Context

During 2026-08-16 Phase 1 gate coordination plan validation, product-owner (Document Control authority per SRS §8) compared eng-director's 8-task coordination DAG against SRS §7.1 authoritative Phase 1 acceptance criteria.

**Error discovered:** ADR-006 (Phase 1 Gate Passage at 5/7 Criteria) contained two incorrect criterion target values, discovered when validating task-5 (latency benchmark) and task-6 (coverage measurement) against the SRS.

---

## Errors in ADR-006

### Error #1: Criterion #5 Latency Target

**ADR-006 stated (line 18):** "#5: <30ms LAN p95 latency"  
**SRS §7.1 line 1463 authoritative value:** "<10ms local latency"

**Source of error:**  
- ADR-006 copied the target from SRS §1.3 line 102 (v1.0 overall success metrics)
- §1.3 lists "<30ms LAN p95 latency" as the v1.0 *overall system target*
- SRS §7.1 Phase 1 acceptance criteria specifies "<10ms local latency" (loopback RTT)
- The Phase 1 gate tests *local* performance (Windows master + web client on same machine)
- The v1.0 metric tests *LAN* performance (distributed deployment across network)

**Impact:**
- task-5 (performance-engineer, latency benchmark) was targeting wrong threshold
- Benchmark would measure <30ms LAN instead of <10ms local
- Could pass gate with insufficient local performance

**Correction applied:** task-5 updated to measure <10ms local latency (127.0.0.1 loopback RTT)

---

### Error #2: Criterion #6 Coverage Target

**ADR-006 stated (line 19):** "#6: 80% test coverage"  
**SRS §7.1 line 1463 authoritative value:** "70% test coverage"

**Source of error:**
- ADR-006 copied the target from SRS §1.3 line 103 (v1.0 overall success metrics)
- §1.3 lists "80% test coverage" as the v1.0 *overall quality target*
- SRS §7.1 Phase 1 acceptance criteria specifies "70% test coverage"
- Phase 1 acceptance bar is intentionally lower (70%) than v1.0 overall target (80%)

**Impact:**
- task-6 (qa-lead, coverage measurement) already correctly targeted ≥70% (no correction needed)
- eng-director's coordination plan was already aligned with SRS §7.1 correct value
- No operational impact, but document control required correction for audit trail

---

## Decision

**SRS §7.1 Phase 1 acceptance criteria (lines 1457-1463) remain authoritative.**

### Corrected Criterion Values

1. **Criterion #5:** <10ms local latency (NOT <30ms LAN p95)
2. **Criterion #6:** 70% test coverage (NOT 80%)

### Document Control Actions

1. **ADR-006 preserved as-written** for historical accuracy and audit trail
2. **This ADR (ADR-010)** documents the corrections with full error analysis
3. **All coordination work** uses corrected SRS §7.1 values:
   - task-5 (performance-engineer): measures <10ms local latency
   - task-6 (qa-lead): targets ≥70% test coverage (already correct)

### Traceability

**Authoritative source:** SRS §7.1 "Phase 1 — Windows + Web" acceptance criteria (lines 1457-1463)

```
Line 1457: **Acceptance Criteria:**
Line 1458: 
Line 1459: - 60 FPS master rendering on Windows 10 1809+
Line 1460: - Web client usable, end to end, from an iPhone/Android browser on the same network
Line 1461: - Monomind suggestion fires correctly for a project without `.monomind/`, and stays dismissed once declined
Line 1462: - Embedded dashboard reflects live master state with no separate service to start
Line 1463: - <10ms local latency, 70% test coverage, zero crashes in a 24-hour soak test
```

**v1.0 overall targets (NOT Phase 1 gate):** SRS §1.3 lines 100-105

---

## Consequences

### Positive

- ✅ Coordination work (task-5, task-6) now measures against correct Phase 1 targets
- ✅ Clear separation between Phase 1 gate (70% coverage, <10ms local) and v1.0 overall targets (80% coverage, <30ms LAN)
- ✅ Audit trail preserved: ADR-006 errors documented, not silently corrected
- ✅ Document control authority (product-owner) validated all 7 Phase 1 criteria against SRS

### Neutral

- No impact on 5/7 gate threshold (ADR-006 decision logic remains valid)
- No impact on coordination timeline (corrections applied before tasks executed)
- task-6 already aligned with correct 70% target (no operational change needed)

### Negative

- None identified

---

## Validation Summary

**Phase 1 Acceptance Criteria (SRS §7.1, Corrected Values):**

| # | Criterion | Target | Status | SRS Ref |
|---|-----------|--------|--------|---------|
| 1 | 60 FPS rendering | Windows 10 1809+ | ✅ Verified: 75 FPS | §7.1 line 1459 |
| 2 | Web client E2E | iPhone/Android browser | ✅ Verified | §7.1 line 1460 |
| 3 | Monomind detection | Fires + dismisses correctly | ✅ Verified: 14/14 tests | §7.1 line 1461 |
| 4 | Embedded dashboard | No separate service | ✅ Verified | §7.1 line 1462 |
| 5 | Latency | **<10ms local** (corrected) | ⏳ task-5 in progress | §7.1 line 1463 |
| 6 | Coverage | **70%** (corrected) | ⏳ task-6 queued | §7.1 line 1463 |
| 7 | Soak test | Zero crashes in 24h | ⏳ task-8 queued | §7.1 line 1463 |

**Current gate progress:** 4/7 verified (57%), 3/7 in-flight with corrected targets

---

## References

- **SRS §7.1:** Phase 1 — Windows + Web (Months 1-3), lines 1434-1466
- **SRS §1.3:** Success Criteria (v1.0 overall targets), lines 98-118
- **ADR-006:** Phase 1 Gate Passage at 5/7 Criteria (file: 006-phase1-gate-passage-5-of-7.md)
- **Coordination session:** 2026-08-16, eng-director ↔ product-owner validation

---

## Approval

**Approved by:** eng-director (2026-08-16)  
**Document control executed by:** product-owner  
**Audit trail:** This ADR corrects ADR-006 while preserving original as historical record
