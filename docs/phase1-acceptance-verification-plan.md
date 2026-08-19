# Phase 1 Acceptance Verification Plan

**Version:** 1.0  
**Date:** August 15, 2026  
**Owner:** QA Lead  
**Phase:** Phase 1 (Windows + Web MVP)  
**Authority:** Final gate approval for Phase 1 → Phase 2 transition  

---

## 1. Executive Summary

This document defines the **executable verification plan** for Phase 1 acceptance criteria (SRS §7.1). Each criterion has specific test procedures, success metrics, and verification status. **Phase 1 cannot proceed to Phase 2 until all 7 criteria show ✅ VERIFIED status.**

**Gate Authority:** QA Lead has final sign-off authority. All criteria must be demonstrably met with documented evidence.

---

## 2. Acceptance Criteria (SRS §7.1)

| # | Criterion | Owner | Status | Evidence |
|---|-----------|-------|--------|----------|
| 1 | 60 FPS master rendering on Windows 10 1809+ | performance-engineer | ⏳ Pending | task-17 |
| 2 | Web client usable end-to-end (iPhone/Android browser, same LAN) | test-engineer-e2e | ⏳ Pending | task-14, task-16 |
| 3 | Monomind suggestion fires & dismisses correctly | test-engineer-e2e | ⏳ Pending | task-16 |
| 4 | Embedded dashboard (no separate service) | test-engineer-e2e | ⏳ Pending | task-12, task-16 |
| 5 | <10ms local latency (LAN p95) | performance-engineer | ⏳ Pending | task-17 |
| 6 | 70% test coverage | test-engineer-unit | 🔄 Running | task-15 |
| 7 | Zero crashes in 24-hour soak test | performance-engineer | ⏳ Pending | task-17 |

**Legend:**
- ⏳ Pending: Blocked by dependencies
- 🔄 Running: Active work in progress
- ✅ Verified: Criterion met with documented evidence
- ❌ Failed: Criterion not met (blocks Phase 2 gate)

---

## 3. Verification Procedures

### 3.1 Criterion #1: 60 FPS Master Rendering

**Test Environment:**
- Windows 10 1809 (build 17763) — minimum supported version
- Windows 11 23H2 (latest) — current version
- Hardware: GPU with DirectX 12 support (integrated graphics OK)

**Test Procedure:**
```powershell
# 1. Build master daemon in release mode
cargo build --release -p monoterminal-master

# 2. Launch master with FPS counter enabled
./target/release/monoterminal-master --fps-counter

# 3. Create session and run scrolling workload
# Inside terminal:
for /L %i in (1,1,10000) do @echo Line %i

# 4. Capture FPS metrics over 60 seconds
# Expected: p50 >= 60 FPS, p95 >= 58 FPS, p99 >= 55 FPS
```

**Success Criteria:**
- ✅ p50 FPS ≥ 60 on Windows 10 1809
- ✅ p50 FPS ≥ 60 on Windows 11 23H2
- ✅ p95 FPS ≥ 58 (allow ~3% variance for background processes)
- ✅ No frame drops during rapid scrolling

**Evidence Required:**
- [ ] Performance test report from task-17 (performance-engineer)
- [ ] FPS histogram screenshots (Windows 10 + Windows 11)
- [ ] Automated benchmark results (criterion.rs output)

**Verification Owner:** performance-engineer (task-17)

---

### 3.2 Criterion #2: Web Client Mobile Browser Usability

**Test Devices:**
- iOS Safari (iPhone 12+, iOS 16+)
- Android Chrome (Pixel 6+, Android 12+)
- Same LAN as Windows master

**Test Procedure:**
```typescript
// E2E test: web/e2e/mobile-browser.spec.ts

test('mobile browser full workflow', async ({ page, browserName }) => {
  // 1. Navigate to web client from mobile device
  await page.goto('http://<master-ip>:8080');
  
  // 2. Verify PWA installability
  const installPrompt = await page.waitForEvent('beforeinstallprompt');
  expect(installPrompt).toBeTruthy();
  
  // 3. Connect to session
  await page.click('[data-testid="connect-button"]');
  await page.fill('[data-testid="session-id-input"]', 'test-session-1');
  await page.click('[data-testid="attach-button"]');
  
  // 4. Type command and verify output
  await page.locator('.xterm').click();
  await page.keyboard.type('echo "Hello from mobile"\n');
  await expect(page.locator('.xterm')).toContainText('Hello from mobile', { timeout: 5000 });
  
  // 5. Verify touch scrolling works
  await page.touchscreen.tap(320, 240);
  await page.evaluate(() => {
    const terminal = document.querySelector('.xterm-screen');
    terminal.scrollTop = 100;
  });
  
  // 6. Detach cleanly
  await page.click('[data-testid="detach-button"]');
  await expect(page.locator('[data-testid="status"]')).toContainText('Detached');
});
```

**Manual Verification (Physical Devices):**
1. **iOS Safari (iPhone):**
   - [ ] Can access `http://<master-ip>:8080` on LAN
   - [ ] Terminal renders correctly (no layout breaks)
   - [ ] Touch keyboard appears when tapping terminal
   - [ ] Can type commands and see output
   - [ ] Touch scrolling works smoothly
   - [ ] PWA "Add to Home Screen" works
   - [ ] No console errors

2. **Android Chrome:**
   - [ ] Same checklist as iOS
   - [ ] PWA install banner appears
   - [ ] Haptic feedback on key press (optional)

**Success Criteria:**
- ✅ All manual checklist items pass on iOS Safari
- ✅ All manual checklist items pass on Android Chrome
- ✅ E2E test suite passes on mobile viewport (Playwright mobile emulation)
- ✅ No crash or frozen screen during 5-minute session

**Evidence Required:**
- [ ] E2E test report from task-16 (test-engineer-e2e)
- [ ] Manual test checklist with device screenshots
- [ ] Video recording of full workflow on real iPhone + Android device

**Verification Owner:** test-engineer-e2e (task-14, task-16)

---

### 3.3 Criterion #3: Monomind Detection & Dismissal

**Test Scenarios:**

**Scenario A: Project without `.monomind/`**
```powershell
# 1. Create test directory without monomind
New-Item -ItemType Directory -Path C:\temp\test-no-monomind
Set-Location C:\temp\test-no-monomind

# 2. Attach session in this directory
# 3. Verify suggestion appears within 5 seconds
# Expected: Banner/notification with "Install monomind?" message
```

**Scenario B: Dismiss suggestion**
```powershell
# 1. Click "Dismiss" on suggestion
# 2. Reload web client
# 3. Verify suggestion does NOT reappear
# Expected: Dismiss flag persisted (SQLite or localStorage)
```

**Scenario C: Project with `.monomind/`**
```powershell
# 1. Create test directory WITH monomind
npx monomind@latest init --wizard
# 2. Attach session
# Expected: NO suggestion appears
```

**Automated Test:**
```rust
// crates/monomind-bridge/tests/detection_test.rs

#[tokio::test]
async fn test_monomind_detection_workflow() {
    // Setup: directory without .monomind/
    let temp_dir = TempDir::new().unwrap();
    let session = create_session_in_dir(&temp_dir).await;
    
    // Act: Check detection
    let detector = MonomindDetector::new();
    let result = detector.check_directory(temp_dir.path()).await.unwrap();
    
    // Assert: Suggestion should fire
    assert!(result.should_show_suggestion);
    assert_eq!(result.reason, DetectionReason::MissingMonominDir);
    
    // Act: Dismiss
    detector.dismiss_suggestion(&session.id).await.unwrap();
    
    // Assert: Should not show again
    let result2 = detector.check_directory(temp_dir.path()).await.unwrap();
    assert!(!result2.should_show_suggestion);
}
```

**Success Criteria:**
- ✅ Suggestion fires within 5s for directories without `.monomind/`
- ✅ Suggestion does NOT fire for directories with `.monomind/`
- ✅ Dismissed suggestions stay dismissed across web client reloads
- ✅ Dismissal persists across master daemon restart
- ✅ Each session checks independently (session A dismiss ≠ session B dismiss)

**Evidence Required:**
- [ ] Integration test report (crates/monomind-bridge)
- [ ] E2E test screenshots showing suggestion UI
- [ ] Manual verification on 3 different projects

**Verification Owner:** test-engineer-e2e (task-16)

---

### 3.4 Criterion #4: Embedded Dashboard (No Separate Service)

**Test Procedure:**

**Integration Test:**
```typescript
// web/e2e/dashboard-embedded.spec.ts

test('dashboard embedded in web client', async ({ page }) => {
  // 1. Navigate to web client
  await page.goto('http://localhost:8080');
  
  // 2. Open dashboard panel (same port/domain)
  await page.click('[data-testid="dashboard-toggle"]');
  
  // 3. Verify dashboard shows monomind org status
  await expect(page.locator('[data-testid="org-status"]')).toBeVisible();
  await expect(page.locator('[data-testid="agent-count"]')).toContainText(/\d+ agents/);
  
  // 4. Verify health check available
  await page.click('[data-testid="run-health-check"]');
  await expect(page.locator('[data-testid="health-result"]')).toContainText('✅', { timeout: 10000 });
  
  // 5. Verify upgrade button exists
  await expect(page.locator('[data-testid="upgrade-button"]')).toBeVisible();
  
  // 6. Verify NO separate auth/token needed
  // (Dashboard API calls should use same WebSocket connection)
  const requests = [];
  page.on('request', req => requests.push(req.url()));
  await page.reload();
  
  // Assert: No separate OAuth flow or token exchange
  expect(requests.filter(url => url.includes('/oauth'))).toHaveLength(0);
  expect(requests.filter(url => url.includes('/token'))).toHaveLength(0);
});
```

**Manual Verification:**
1. Open web client at `http://<master-ip>:8080`
2. Verify dashboard toggle/tab exists in UI
3. Click dashboard → should open immediately (no separate login)
4. Verify shows:
   - [ ] Current org name
   - [ ] Active agents list
   - [ ] Run status (running/stopped)
   - [ ] Health check button
   - [ ] Upgrade button
5. Run health check → should complete within 10s
6. Verify NO separate browser tab or port (e.g., NOT `http://localhost:9000/dashboard`)

**Success Criteria:**
- ✅ Dashboard accessible from main web client UI
- ✅ Same WebSocket connection (no separate port)
- ✅ No separate authentication required
- ✅ Shows live monomind state (org, agents, runs)
- ✅ Health check executes and displays result
- ✅ One-click upgrade button present

**Evidence Required:**
- [ ] E2E test report (web/e2e/dashboard-embedded.spec.ts)
- [ ] Screenshot showing dashboard panel in web client
- [ ] Network trace showing single WebSocket connection

**Verification Owner:** test-engineer-e2e (task-16, depends on task-12)

---

### 3.5 Criterion #5: <10ms Local Latency (LAN p95)

**Test Environment:**
- Master and web client on same LAN (1 Gbps switch)
- No WAN traffic (disable internet to isolate LAN-only path)

**Test Procedure:**
```powershell
# 1. Build master with latency instrumentation
cargo build --release --features latency-tracing -p monoterminal-master

# 2. Run latency benchmark
cargo bench --bench latency_p95

# 3. Capture round-trip time (RTT) for input → output
# Measure: Client sends 'X' → Master echoes 'X' → Client renders
# Sample size: 10,000 measurements over 5 minutes
```

**Latency Measurement Points:**
```
[Web Client] ---(1)---> [WebSocket] ---(2)---> [Master PTY Input]
                                                      |
                                                      v
[Web Client] <---(4)--- [WebSocket] <---(3)--- [Master PTY Output]

Total RTT = (4) - (1)
Target: p95 < 10ms
```

**Automated Benchmark:**
```rust
// crates/master/benches/latency_p95.rs

fn benchmark_local_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("local-latency");
    group.sample_size(10_000);
    
    group.bench_function("websocket-rtt-p95", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                // Send input, wait for echo
                client.send_input(b"X\n").await;
                client.recv_output().await;
            }
            start.elapsed()
        });
    });
    
    group.finish();
}
```

**tcpdump Verification (Manual):**
```powershell
# Capture WebSocket packets on loopback
# Windows: Use Wireshark on loopback adapter
1. Start Wireshark, filter: tcp.port == 8080
2. Type single character in terminal
3. Measure time from client→server packet to server→client response
4. Verify p95 < 10ms in Wireshark statistics
```

**Success Criteria:**
- ✅ Automated benchmark: p50 < 5ms, p95 < 10ms, p99 < 15ms
- ✅ Manual tcpdump/Wireshark: 95% of samples < 10ms
- ✅ No packet loss (0% dropped frames)
- ✅ Consistent latency under load (10 concurrent clients)

**Evidence Required:**
- [ ] Criterion.rs benchmark report (HTML + JSON)
- [ ] Wireshark packet capture (PCAP file + statistics screenshot)
- [ ] Latency histogram graph (p50/p95/p99 markers)

**Verification Owner:** performance-engineer (task-17)

---

### 3.6 Criterion #6: 70% Test Coverage

**Coverage Framework:**
- Tool: cargo-tarpaulin + codecov
- Configuration: `.tarpaulin.toml`, `.codecov.yml`
- Exclusions: Generated code (`proto/generated/*`), test code, benches

**Test Execution:**
```powershell
# 1. Install tarpaulin
cargo install cargo-tarpaulin

# 2. Run full test suite with coverage
cargo tarpaulin --workspace --all-features --out Html --out Xml --timeout 300

# 3. Verify coverage >= 70%
# Output: coverage/index.html
# Expected: Total coverage: 70.00% or higher
```

**Coverage Breakdown (Target):**
| Crate | Target | Critical Modules (≥85%) |
|-------|--------|-------------------------|
| `monoterminal-master` | 75% | `pty/`, `session/`, `auth/` |
| `monoterminal-protocol` | 80% | `encoding/`, `compression/` |
| `monomind-bridge` | 70% | `detection/`, `health/` |
| `web` (Vitest) | 65% | `websocket.ts`, `terminal.tsx` |

**CI Enforcement:**
```yaml
# .github/workflows/coverage.yml (from test-strategy-phase1.md)
- name: Check coverage threshold
  run: |
    $coverage = (Get-Content coverage/cobertura.xml | Select-String -Pattern 'line-rate="([0-9.]+)"').Matches[0].Groups[1].Value
    $coveragePct = [double]$coverage * 100
    if ($coveragePct -lt 70.0) {
      Write-Error "Coverage ${coveragePct}% is below 70% threshold"
      exit 1
    }
    Write-Host "✅ Coverage: ${coveragePct}%"
```

**Success Criteria:**
- ✅ Total workspace coverage ≥ 70%
- ✅ Core modules (`pty/`, `session/`, `auth/`) ≥ 85%
- ✅ No coverage regression in CI (codecov comment on PRs)
- ✅ Coverage badge shows ≥70% on README

**Evidence Required:**
- [ ] Codecov report URL (https://codecov.io/gh/...)
- [ ] Coverage HTML report (coverage/index.html)
- [ ] Per-crate coverage breakdown table
- [ ] CI workflow passing (GitHub Actions badge)

**Verification Owner:** test-engineer-unit (task-15)

---

### 3.7 Criterion #7: Zero Crashes in 24-Hour Soak Test

**Test Environment:**
- Windows 10 1809 or Windows 11 23H2
- Release build (--release)
- Isolated test machine (no other dev work)

**Test Procedure:**
```powershell
# 1. Build release binary
cargo build --release -p monoterminal-master

# 2. Run soak test (24 hours)
cargo test --release --test soak_test_24h_no_crashes --ignored -- --nocapture

# Test script: tests/soak/24h-stability.rs (from test-strategy-phase1.md)
# Workload:
# - Create 10 sessions every 5 minutes
# - Send random input (echo commands)
# - Terminate 5 sessions randomly
# - Repeat for 24 hours (288 iterations)
```

**Monitoring:**
```powershell
# Monitor memory usage (should be stable, no leaks)
while ($true) {
    $proc = Get-Process monoterminal-master -ErrorAction SilentlyContinue
    if ($proc) {
        $memMB = [math]::Round($proc.WorkingSet64 / 1MB, 2)
        $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        Write-Host "$timestamp - Memory: ${memMB} MB"
        Add-Content -Path soak-memory.log -Value "$timestamp,$memMB"
    }
    Start-Sleep -Seconds 300  # Log every 5 minutes
}
```

**Success Criteria:**
- ✅ Master daemon runs for 24 hours without crash
- ✅ No panics in logs
- ✅ Memory usage stable (≤5% growth over 24h)
- ✅ All created sessions terminate cleanly
- ✅ No zombie PTY processes
- ✅ No file descriptor leaks

**Crash Detection:**
- Any `panic!` or unhandled exception = FAIL
- Process exit code ≠ 0 = FAIL
- Windows Event Viewer application crash = FAIL

**Evidence Required:**
- [ ] Soak test output log (24h-soak-test.log)
- [ ] Memory usage graph (soak-memory.log → Excel chart)
- [ ] Windows Event Viewer screenshot (no crashes)
- [ ] Final test assertion: `assert!(ctx.daemon().is_running())`

**Verification Owner:** performance-engineer (task-17)

---

## 4. Verification Schedule & Dependencies

### 4.1 Dependency Graph

```
task-19 (Phase 1 Acceptance) ← depends on
    ├── task-17 (Performance Validation) ← depends on
    │       └── task-16 (Integration Tests) ← depends on
    │               ├── task-14 (E2E Tests)
    │               └── task-15 (Unit Tests @ 70%)
    └── task-18 (CI Windows Pipeline)
```

### 4.2 Timeline

| Week | Milestone | Owner | Deliverable |
|------|-----------|-------|-------------|
| 1-8 | Implementation (tasks 1-13) | Engineering team | Code complete |
| 9 | Unit tests + 70% coverage | test-engineer-unit | task-15 DONE |
| 10 | E2E tests + integration | test-engineer-e2e | task-14, task-16 DONE |
| 11 | Performance validation | performance-engineer | task-17 DONE |
| 11 | CI pipeline operational | devops-lead | task-18 DONE |
| **12** | **Phase 1 Acceptance Verification** | **qa-lead** | **task-19 DONE** |

**Current Week:** Week 1 (Implementation started)

---

## 5. Coordination Plan

### 5.1 Team Communication

**Daily Standups (async via org_send):**
- test-engineer-unit: Coverage progress report
- test-engineer-e2e: E2E test status + blockers
- performance-engineer: Benchmark results + soak test prep

**Weekly Review (Fridays):**
- QA Lead sends status summary to eng-director
- Gate approval readiness assessment

### 5.2 Escalation Path

**Blockers:**
1. Report to QA Lead immediately via org_send
2. QA Lead escalates to eng-director if critical path blocked
3. eng-director resolves resource conflicts

**Criteria Failure:**
- If any criterion shows ❌ Failed → immediate escalation
- Root cause analysis required before retry
- Phase 2 gate blocked until all ✅ Verified

---

## 6. Approval Process

### 6.1 Sign-Off Checklist

**QA Lead Final Checklist:**
- [ ] All 7 acceptance criteria show ✅ Verified status
- [ ] All evidence documents collected and reviewed
- [ ] No critical bugs open (P0/P1)
- [ ] CI pipeline green for 5 consecutive days
- [ ] Manual smoke test passed on fresh Windows 10 + Windows 11 machines
- [ ] Mobile browser tests passed on real iPhone + Android devices
- [ ] Soak test completed with zero crashes
- [ ] Coverage stable at ≥70% with no regression

### 6.2 Approval Authority

**Gate Approval Flow:**
1. **QA Lead** verifies all criteria → signs off on task-19
2. **QA Lead** sends final report to eng-director
3. **eng-director** reviews and approves Phase 1 → Phase 2 transition

**Rejection Authority:**
- QA Lead can BLOCK Phase 2 gate if any criterion fails
- No override without eng-director explicit approval + documented risk acceptance

---

## 7. Test Execution Log

### 7.1 Verification Status Tracking

| Criterion | Status | Last Updated | Notes |
|-----------|--------|--------------|-------|
| #1 60 FPS rendering | ⏳ Pending | 2026-08-15 | Waiting on task-17 |
| #2 Mobile browser | 🔴 STUB | 2026-08-15 | Test file exists but is stub; needs auth wiring + implementation |
| #3 Monomind detection | 🔴 STUB | 2026-08-15 | Test file exists but all assertions are TODOs; needs implementation |
| #4 Embedded dashboard | 🔴 STUB | 2026-08-15 | Test file exists but all assertions commented; needs dashboard UI implementation |
| #5 <10ms latency | ⏳ Pending | 2026-08-15 | Needs auth wiring + benchmark implementation |
| #6 70% coverage | 🔄 VERIFYING | 2026-08-15 | Tarpaulin executing now (eng-director); awaiting results |
| #7 24h soak test | ⏳ Pending | 2026-08-15 | Waiting on task-17 |

**Status Legend:**
- ⏳ Pending: Blocked by dependencies
- 🔴 STUB: Test scaffolding exists but needs implementation before verification possible
- ✅ READY: Infrastructure ready, can verify once prerequisites met
- 🔄 Running: Active work in progress
- ✅ Verified: Criterion met with documented evidence
- ❌ Failed: Criterion not met (blocks Phase 2 gate)

### 7.2 Evidence Repository

**Location:** `tests/evidence/phase1/`

```
tests/evidence/phase1/
├── criterion-1-fps/
│   ├── win10-fps-report.html
│   ├── win11-fps-report.html
│   └── benchmark-results.json
├── criterion-2-mobile/
│   ├── ios-safari-video.mp4
│   ├── android-chrome-video.mp4
│   └── e2e-test-report.html
├── criterion-3-monomind/
│   ├── detection-test-screenshots/
│   └── integration-test-report.md
├── criterion-4-dashboard/
│   ├── dashboard-screenshot.png
│   └── network-trace.har
├── criterion-5-latency/
│   ├── wireshark-capture.pcapng
│   ├── latency-histogram.png
│   └── benchmark-report.json
├── criterion-6-coverage/
│   ├── codecov-report-url.txt
│   ├── coverage-html.zip
│   └── per-crate-breakdown.csv
└── criterion-7-soak/
    ├── 24h-test-log.txt
    ├── memory-usage-graph.png
    └── event-viewer-screenshot.png
```

---

## 8. Risk Register

| Risk | Impact | Mitigation | Owner |
|------|--------|------------|-------|
| Soak test infrastructure unavailable | HIGH | Reserve dedicated test machine 2 weeks before | devops-lead |
| Mobile devices not available for testing | MEDIUM | Procure iPhone + Android device in Week 8 | qa-lead |
| Coverage dips below 70% late in cycle | HIGH | Daily coverage monitoring from Week 6 | test-engineer-unit |
| Latency spikes on CI hardware | MEDIUM | Run latency tests on local LAN, not CI | performance-engineer |
| Monomind detection logic changes late | MEDIUM | Lock monomind-bridge API in Week 7 | monomind-integration-engineer |

---

## 9. Continuous Monitoring (Post-Gate)

**Even after Phase 1 gate approval, continue monitoring:**
- Weekly soak test runs (automated)
- Coverage ratchet (prevent regression)
- Latency benchmarks on every release candidate
- Mobile browser compatibility checks on new iOS/Android versions

**Phase 2 Entry Condition:** Phase 1 criteria must STAY ✅ Verified throughout Phase 2 development.

---

## 10. Document Maintenance

**Version History:**

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-08-15 | Initial verification plan created by qa-lead |

**Next Review:** After task-17 completion (performance validation)

---

**Document Owner:** QA Lead  
**Approval Status:** Draft (pending eng-director review)  
**Phase 1 Gate Status:** 🔴 NOT READY (0/7 criteria verified)
