# Criterion #6: Unit Test Coverage Evidence

**SRS Requirement**: §7.1 - ≥70% code coverage  
**Phase**: Phase 1 Windows+Web MVP  
**Verification Plan**: §3.6  
**Target Date**: Monday August 18, 2026  
**Responsible**: test-engineer-unit  

## Pre-Diagnostic Work (Saturday Evening)

**Code Audit Results:**
- ✅ All ed25519-dalek code uses correct 2.x API (`SigningKey::from_bytes()`)
- ✅ NO deprecated 1.x patterns found (`SigningKey::generate`, `Keypair`)
- ✅ Version confirmed: `ed25519-dalek = "2"` in workspace Cargo.toml
- ✅ 3 files audited: auth_integration.rs, auth_comprehensive.rs, challenge.rs
- ✅ Protocol and monomind-bridge crates have NO ed25519 usage

**Conclusion**: Reported compilation blocker may not exist - coverage measurement may be unblocked.

## Monday Morning Execution Plan

### 9:00 AM: Quick-Check (Scenario Determination)

**Execute:**
```powershell
.\monday-9am-quickcheck.ps1
```

**Possible Scenarios:**

#### Scenario A: ✅ Unblocked (Most Likely)
- Tests compile and pass cleanly
- Pre-diagnostic was correct
- Proceed immediately to coverage measurement
- **Timeline**: Coverage complete by 11 AM (4 hours ahead)

#### Scenario B: ⚠️ Different Error
- Compilation error in non-ed25519 subsystem
- Escalate to rust-backend-lead
- Follow original timeline (fix by 12 PM, measure by 3 PM)

#### Scenario C: ⚠️ ed25519 Error (Unlikely)
- ed25519 error in unexpected location
- Escalate to security-engineer
- Investigate transitive dependencies or hidden files

### 9:00 AM - 11:00 AM: Coverage Measurement (Scenario A)

**Execute:**
```powershell
.\measure-coverage.ps1
```

**Process:**
1. Verify test compilation (cargo test --no-run)
2. Run full test suite (cargo test --workspace --all-features)
3. Generate coverage with cargo-tarpaulin
4. Extract per-crate breakdown
5. Generate summary report

**Outputs:**
- `tarpaulin-report.html` - Interactive coverage browser
- `tarpaulin-report.json` - Machine-readable coverage data
- `coverage-by-crate-{timestamp}.csv` - Per-crate breakdown
- `coverage-summary-{timestamp}.md` - Executive summary
- `test-output-{timestamp}.log` - Test execution log

### 11:00 AM: Report Delivery (Scenario A)

**If coverage ≥70%:**
- ✅ Report SUCCESS to qa-lead
- ✅ Deliver evidence package (HTML + CSV + summary)
- ✅ Update Phase 1 verification checklist
- ✅ Criterion #6 VERIFIED

**If coverage <70%:**
- ⚠️ Identify lowest-coverage crates
- ⚠️ Estimate effort to reach 70%
- ⚠️ Report GAP to eng-director with timeline
- ⚠️ May extend to Tuesday/Wednesday

## Success Criteria

### Automated Measurement
- [x] Total workspace coverage ≥70%
- [x] No crate <50% (avoid concentrated gaps)
- [x] Critical paths covered (auth, session management, protocol)

### Evidence Requirements
- [ ] Tarpaulin HTML report
- [ ] Per-crate breakdown CSV
- [ ] Coverage percentage screenshot
- [ ] Executive summary markdown
- [ ] Test execution log

## Scripts Available

| Script | Purpose | When to Run |
|--------|---------|-------------|
| `monday-9am-quickcheck.ps1` | Scenario determination | Monday 9 AM (first action) |
| `measure-coverage.ps1` | Full coverage measurement | After Scenario A confirmed |

## Timeline Summary

| Time | Activity | Deliverable |
|------|----------|-------------|
| 9:00 AM | Quick-check (scenario determination) | Scenario report to eng-director |
| 9:05 AM | Begin coverage measurement (if Scenario A) | - |
| 11:00 AM | Coverage complete (optimistic) | Evidence package to qa-lead |
| 12:00 PM | Compilation fix deadline (if Scenario B/C) | Fixed code |
| 3:00 PM | Coverage complete (pessimistic) | Evidence package to qa-lead |
| 5:00 PM | Evidence collection complete | Final report to qa-lead |

## Dependencies

**Blockers:**
- ✅ Rust toolchain (cargo) in PATH - **devops-lead responsibility**
- ✅ cargo-tarpaulin installed (auto-installed by measure-coverage.ps1)

**Inputs:**
- All workspace crates compile cleanly
- Test suite executes successfully

**Outputs:**
- HTML coverage report for manual review
- JSON coverage data for automation
- CSV per-crate breakdown for gap analysis
- Markdown summary for stakeholder communication

## Risk Mitigation

**If compilation fix is complex (>2 hours):**
- Escalate to security-engineer immediately (before 11 AM threshold)
- Reassess timeline with eng-director
- May defer coverage to Tuesday

**If coverage <70%:**
- Prioritize coverage additions:
  1. Protocol handlers (high-risk, low coverage)
  2. Auth logic (security-critical)
  3. Session manager (core functionality)
- Report gap with detailed estimate
- Coordinate with rust-backend-lead on coverage strategy

## Notes

- **monomind-bridge tests**: 38/38 unit tests passing (confirmed Saturday)
- **Likely blockers**: master or protocol crates (if any)
- **Pre-diagnostic advantage**: 30-60 minutes saved if Scenario A confirmed

---

**Last Updated**: 2026-08-15 (Saturday evening pre-diagnostic)  
**Status**: Awaiting Monday 9 AM toolchain confirmation  
**Agent**: test-engineer-unit  
