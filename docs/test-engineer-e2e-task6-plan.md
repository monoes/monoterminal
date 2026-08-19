# Task-6: E2E Test Assertion Updates - Execution Plan

**Owner:** test-engineer-e2e  
**Target Date:** Monday, August 18, 2026  
**Estimated Effort:** 1 day  
**Status:** ✅ READY FOR FULL EXECUTION - Zero Blockers!

**BACKEND UPDATE (2026-08-15 EOD):** task-7 is 100% complete!
- ✅ detection.rs (328 lines, 16 tests)
- ✅ health.rs (675 lines, 13 tests)
- ✅ dashboard.rs (508 lines, 7 tests)
- ✅ Protobuf schema complete (lines 74-203)
- ✅ Handler integration complete (lines 375-525)
- **Location:** `crates/monomind-bridge/src/` (not daemon/src/monomind/)

---

## Pre-Execution Assessment (2026-08-15)

### ✅ Good News: Frontend Already Has data-testid Attributes!

**Verified Components:**
- `MonomindPanel.tsx`: All sections have data-testid attributes
  - `dashboard-panel`, `panel-header`, `close-button`
  - `health-section`, `health-result`, `run-health-check`
  - `org-section`, `org-status`, `org-name`, `agent-count`
  - `upgrade-section`, `upgrade-button`
- `MonomindSuggestion.tsx`: All elements tagged
  - `monomind-suggestion`, `monomind-suggestion-dismiss`
  - `suggestion-text`, `suggestion-message`
- `App.tsx`: Dashboard toggle has `data-testid="dashboard-toggle"`

**Implication:** The frontend blocker mentioned by eng-director may already be resolved, OR the blocker is actually backend API completion (task-7).

### 🔍 Current Test Status

**File: `tests/e2e/test_session_flow.py`**
- ✅ 5 tests with REAL assertions (no stubs!)
- ✅ Full session lifecycle working
- Status: READY (no updates needed)

**File: `tests/integration/test_monomind_integration.py`**
- ⚠️ 6 tests ALL marked with `pytest.skip()`
- ⚠️ Assertions are commented out
- Status: BLOCKED by backend APIs

**File: `tests/integration/test_multi_client_attach.py`**
- ✅ 4 tests with real assertions
- ⚠️ 1 test (`test_presence_notifications`) using `pytest.skip()`
- Status: Mostly ready

### 🚧 Dependencies

**Backend APIs (task-7, 85% complete EOD Friday):**
1. `detection.rs` - Monomind detection logic
2. `health.rs` - Health check endpoint
3. `dashboard.rs` - Dashboard data endpoint

**Protocol Updates:**
- `AttachResponse` must include monomind detection fields
- Dashboard API messages (DashboardRequest/DashboardResponse)
- Health check messages (HealthCheckRequest/HealthCheckResponse)

---

## Monday Execution Plan

### Phase 1: Review Backend Implementation (30 min)

**Backend Status:** ✅ 100% COMPLETE (36 unit tests)

```bash
# 1. Pull latest changes
git pull origin main

# 2. Review backend implementation (CORRECT LOCATION)
cat crates/monomind-bridge/src/detection.rs    # 328 lines, 16 tests
cat crates/monomind-bridge/src/health.rs       # 675 lines, 13 tests
cat crates/monomind-bridge/src/dashboard.rs    # 508 lines, 7 tests

# 3. Review protobuf schema (lines 74-203)
cat proto/monoterminal/v1/messages.proto | sed -n '74,203p'
# Messages: DetectionRequest/Response, DashboardRequest/Response,
#           HealthCheckRequest/Response, UpgradeRequest/Response

# 4. Review handler integration
cat crates/master/src/server/handler.rs | sed -n '375,525p'

# 5. Run backend tests to verify
cargo test -p monomind-bridge --lib

# 6. Build workspace
cargo build --workspace
```

**Expected Outcome:** All backend tests pass (36/36), API contracts understood

---

### Phase 2: Update Criterion #3 Tests (Monomind Detection) (2 hours)

**File:** `tests/integration/test_monomind_integration.py`

#### Test 1: `test_monomind_detection_no_project`
**Current:** Skipped  
**Action:**
```python
# REMOVE:
pytest.skip("Monomind detection not yet implemented")

# UNCOMMENT and UPDATE:
assert response.monomind_suggestion is True
assert response.monomind_banner_text is not None
assert ".monomind/" in response.monomind_banner_text
```

**Verification:**
```bash
pytest tests/integration/test_monomind_integration.py::test_monomind_detection_no_project -v
```

#### Test 2: `test_monomind_detection_with_project`
**Action:**
```python
# REMOVE skip
# UNCOMMENT:
assert response.monomind_suggestion is False
```

#### Test 3: `test_suggestion_dismiss_marker`
**Action:**
```python
# REMOVE skip
# UNCOMMENT:
assert response1.monomind_suggestion is True

# Create dismiss marker (already present)
dismiss_marker = tmp_path / ".monomind-suggestion-dismissed"
dismiss_marker.write_text("dismissed")

# UNCOMMENT:
assert response2.monomind_suggestion is False
```

**Expected Outcome:** 3/3 tests pass

---

### Phase 3: Update Criterion #4 Tests (Embedded Dashboard) (2 hours)

#### Test 4: `test_monomind_health_check`
**Current:** Skipped  
**Action:**
```python
# REMOVE skip
# IMPLEMENT:
client = ProtocolClient(daemon_process.base_url)
await client.connect(auth_jwt=sample_jwt)

# Request health check via WebSocket (NOT separate HTTP endpoint!)
health_response = await client.send_health_check_request({
    "project_dir": ""
})

assert health_response.type == "HealthCheckResponse"
assert health_response.installed is not None
assert health_response.version is not None
assert "status" in health_response
assert health_response.status in ["healthy", "degraded", "unhealthy"]
```

#### Test 5: `test_monomind_dashboard_session_status`
**Action:**
```python
# REMOVE skip
# IMPLEMENT:
client = ProtocolClient(daemon_process.base_url)
await client.connect(auth_jwt=sample_jwt)

# Attach to session first
await client.send_attach_request(test_session_id)

# Query dashboard for session status via WebSocket
dashboard_response = await client.send_dashboard_request({
    "request_type": "session_status"
})

assert dashboard_response.type == "DashboardResponse"
assert "sessions" in dashboard_response
assert any(s.session_id == test_session_id for s in dashboard_response.sessions)
```

#### Test 6: `test_monomind_upgrade_check`
**Action:**
```python
# REMOVE skip
# IMPLEMENT:
upgrade_response = await client.send_dashboard_request({
    "request_type": "upgrade_check"
})

assert upgrade_response.type == "DashboardResponse"
assert "current_version" in upgrade_response
assert "latest_version" in upgrade_response
assert "upgrade_available" in upgrade_response
```

**Critical Verification - No Separate Service:**
```python
@pytest.mark.integration
async def test_dashboard_embedded_no_separate_port():
    """
    Verify dashboard is embedded (no separate HTTP service).
    
    Criterion #4: Dashboard must NOT use separate port/auth.
    """
    import socket
    
    # Verify WebSocket port is open (5000)
    ws_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    assert ws_socket.connect_ex(('localhost', 5000)) == 0
    ws_socket.close()
    
    # Verify NO separate dashboard port (e.g., 9000)
    dashboard_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    result = dashboard_socket.connect_ex(('localhost', 9000))
    dashboard_socket.close()
    
    assert result != 0, \
        "Separate dashboard port detected - violates SRS §2.4.2 embedded requirement"
```

**Expected Outcome:** 4/4 tests pass

---

### Phase 4: Add Criterion #2 Tests (Mobile Browser) (3 hours)

**New File:** `tests/e2e/test_mobile_browser.py`

```python
"""
E2E Test: Mobile Browser Support (Criterion #2)
Tests web client usability on iPhone/Android browsers

Verification Plan §3.2:
- iOS Safari viewport simulation
- Android Chrome viewport simulation
- Touch keyboard interaction
- PWA installability
"""

import asyncio
import pytest
from playwright.async_api import async_playwright

from tests.common.protocol import ProtocolClient


@pytest.mark.e2e
@pytest.mark.mobile
@pytest.mark.asyncio
async def test_mobile_ios_safari_workflow(daemon_process, sample_jwt):
    """
    Full session workflow on iOS Safari mobile browser.
    
    Emulates iPhone 12, iOS 16+, Safari.
    """
    async with async_playwright() as p:
        # Launch browser with mobile viewport
        browser = await p.webkit.launch()
        context = await browser.new_context(
            viewport={"width": 390, "height": 844},
            device_scale_factor=3,
            user_agent="Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) "
                       "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 "
                       "Mobile/15E148 Safari/604.1"
        )
        page = await context.new_page()
        
        try:
            # 1. Navigate to web client
            base_url = daemon_process.base_url.replace("ws://", "http://")
            await page.goto(base_url)
            
            # 2. Wait for connection
            await page.wait_for_selector('[data-testid="connection-status"]', timeout=10000)
            
            # 3. Verify dashboard toggle exists
            dashboard_toggle = await page.query_selector('[data-testid="dashboard-toggle"]')
            assert dashboard_toggle is not None, "Dashboard toggle not found"
            
            # 4. Verify terminal renders
            terminal = await page.query_selector('.xterm')
            assert terminal is not None, "xterm.js terminal not rendered"
            
            # 5. Simulate touch on terminal to show keyboard
            await page.tap('.xterm')
            await asyncio.sleep(0.5)
            
            # 6. Type command via mobile keyboard
            await page.keyboard.type('echo "mobile test"')
            await page.keyboard.press('Enter')
            
            # 7. Verify output appears
            terminal_content = await page.inner_text('.xterm-screen')
            assert "mobile test" in terminal_content.lower(), \
                f"Expected 'mobile test' in terminal, got: {terminal_content}"
            
            # 8. Test touch scrolling
            await page.evaluate("""
                const terminal = document.querySelector('.xterm-screen');
                terminal.scrollTop = 100;
            """)
            
            # 9. Verify no console errors
            console_errors = []
            page.on('console', lambda msg: 
                console_errors.append(msg.text) if msg.type == 'error' else None
            )
            await asyncio.sleep(1.0)
            
            assert len(console_errors) == 0, \
                f"Console errors detected: {console_errors}"
            
        finally:
            await browser.close()


@pytest.mark.e2e
@pytest.mark.mobile
@pytest.mark.asyncio
async def test_mobile_android_chrome_workflow(daemon_process):
    """
    Full session workflow on Android Chrome mobile browser.
    
    Emulates Pixel 6, Android 12+, Chrome.
    """
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        context = await browser.new_context(
            viewport={"width": 412, "height": 915},
            device_scale_factor=2.625,
            user_agent="Mozilla/5.0 (Linux; Android 12; Pixel 6) "
                       "AppleWebKit/537.36 (KHTML, like Gecko) "
                       "Chrome/100.0.4896.127 Mobile Safari/537.36"
        )
        page = await context.new_page()
        
        try:
            # Same test flow as iOS Safari
            base_url = daemon_process.base_url.replace("ws://", "http://")
            await page.goto(base_url)
            
            await page.wait_for_selector('[data-testid="connection-status"]', timeout=10000)
            
            terminal = await page.query_selector('.xterm')
            assert terminal is not None
            
            await page.tap('.xterm')
            await page.keyboard.type('echo "android test"')
            await page.keyboard.press('Enter')
            
            terminal_content = await page.inner_text('.xterm-screen')
            assert "android test" in terminal_content.lower()
            
        finally:
            await browser.close()


@pytest.mark.e2e
@pytest.mark.mobile
@pytest.mark.manual
def test_mobile_pwa_installability_checklist():
    """
    MANUAL TEST: PWA install verification on real devices.
    
    This test documents the manual verification checklist.
    Automated PWA testing is limited - real device testing required.
    """
    checklist = """
    iOS Safari (Real iPhone):
    [ ] Navigate to http://<master-ip>:8080 on LAN
    [ ] Terminal renders without layout breaks
    [ ] Tap terminal → keyboard appears
    [ ] Type command → see output
    [ ] Touch scrolling works smoothly
    [ ] Share → "Add to Home Screen" available
    [ ] Install PWA → app icon appears on home screen
    [ ] Launch PWA → full-screen mode, no browser chrome
    [ ] No console errors (Safari Web Inspector)
    
    Android Chrome (Real Android):
    [ ] Navigate to http://<master-ip>:8080 on LAN
    [ ] Terminal renders correctly
    [ ] PWA install banner appears (or Add to Home Screen in menu)
    [ ] Tap terminal → keyboard appears
    [ ] Type command → see output
    [ ] Touch scrolling works
    [ ] Install PWA → app in app drawer
    [ ] Launch PWA → standalone mode
    [ ] No console errors (Chrome DevTools)
    """
    
    pytest.skip(f"Manual test - complete checklist:\n{checklist}")
```

**Dependencies:**
```bash
# Install Playwright for mobile browser testing
pip install playwright pytest-playwright
python -m playwright install webkit chromium
```

**Expected Outcome:** 2/2 automated tests pass, manual checklist documented

---

### Phase 5: Run Full Test Suite + Generate Evidence (1 hour)

```bash
# 1. Unit tests (already passing - verify no regressions)
pytest tests/unit/ -v

# 2. Integration tests (Criterion #3, #4)
pytest tests/integration/test_monomind_integration.py -v

# 3. E2E tests (Criterion #2)
pytest tests/e2e/ -v -m e2e

# 4. Mobile tests (Criterion #2)
pytest tests/e2e/test_mobile_browser.py -v -m mobile

# 5. Generate HTML report for qa-lead (REQUIRED)
pytest tests/integration/ tests/e2e/ \
  --html=tests/evidence/phase1/automated-test-report.html \
  --self-contained-html

# 6. Generate coverage report
pytest --cov=. --cov-report=html --cov-report=term

# 7. Verify 70% coverage maintained (Criterion #6)
coverage report --fail-under=70
```

**Expected Output:**
- HTML report: `tests/evidence/phase1/automated-test-report.html`
- 9/9 tests passing (0 failed, 0 skipped)
- Coverage: ≥70% maintained

---

### Phase 6: Deliver Results to qa-lead (30 min)

**Evidence Collection Workflow (Per qa-lead guidance):**

**My Responsibility (Automated Evidence):**
- ✅ pytest HTML report (self-contained)
- ✅ Plain-text summary (pass/fail counts)
- ✅ Test file locations
- ✅ Issues encountered

**qa-lead Responsibility (Manual Evidence - Week 10):**
- ❌ Manual device screenshots (iOS/Android) - NOT my job
- ❌ Manual device videos (full workflow) - NOT my job
- ❌ Network traces (Wireshark PCAP) - NOT my job

**Create evidence directory structure:**
```bash
mkdir -p tests/evidence/phase1/criterion-2-mobile/automated
mkdir -p tests/evidence/phase1/criterion-3-monomind/automated
mkdir -p tests/evidence/phase1/criterion-4-dashboard/automated
```

**Prepare delivery package:**
```bash
# 1. Copy main HTML report
cp tests/evidence/phase1/automated-test-report.html \
   tests/evidence/phase1/

# 2. Generate plain-text summary
pytest tests/integration/ tests/e2e/ --tb=short > \
   tests/evidence/phase1/test-results.txt

# 3. Archive test files
cp tests/integration/test_monomind_integration.py \
   tests/evidence/phase1/criterion-3-monomind/automated/
cp tests/e2e/test_mobile_browser.py \
   tests/evidence/phase1/criterion-2-mobile/automated/
```

**Send results to qa-lead via org_send:**
- Subject: "task-6 COMPLETE - 9/9 Tests Passing"
- Include: Summary, file locations, recommendation
- Evidence location: `tests/evidence/phase1/automated-test-report.html`

---

## Acceptance Criteria for task-6 Completion

| Criterion | Test Count | Status | Evidence |
|-----------|------------|--------|----------|
| #2 (Mobile) | 2 automated + 1 manual | ⏳ | HTML report + screenshots |
| #3 (Detection) | 3 integration tests | ⏳ | HTML report + screenshots |
| #4 (Dashboard) | 4 integration tests | ⏳ | HTML report + network trace |

**Definition of Done:**
- [ ] All 9 tests pass (3 detection + 4 dashboard + 2 mobile)
- [ ] No pytest.skip() decorators remaining in criterion #2/#3/#4 tests
- [ ] Evidence collected in `tests/evidence/phase1/`
- [ ] Coverage remains ≥70%
- [ ] Report sent to qa-lead with test results

---

## Risk Mitigation

### ~~Risk: Backend APIs not ready Monday morning~~ ✅ CLEARED
**Status:** Backend 100% complete (2026-08-15 EOD)
- monomind-integration-engineer delivered all APIs with 36 tests
- Zero blockers for Monday execution
- Fallback Plan B NOT needed

### Risk: Playwright mobile emulation doesn't match real devices
**Mitigation:**
- Automated tests verify basic functionality only
- Real device testing deferred to qa-lead manual verification (Week 10)
- Document limitations in test docstrings

### Risk: Protocol schema changes break existing tests
**Mitigation:**
- Review protobuf changes before updating tests
- Update `tests/common/protocol.py` helper methods first
- Run full test suite after each change

---

## Communication Plan

**Monday Morning (9 AM):**
- [ ] Message eng-director: "Starting task-6, verified backend readiness"
- [ ] Message qa-lead: "Beginning assertion updates, ETA 5 PM"

**Monday Afternoon (2 PM):**
- [ ] Progress update to qa-lead: "Criterion #3/#4 complete, working on #2"

**Monday EOD (5 PM):**
- [ ] Final report to qa-lead:
  - Test results (pass/fail counts)
  - Evidence location
  - Any blockers encountered
  - Recommendation for criterion verification status

---

## Next Steps After task-6

**Immediate follow-up (task-16):**
- qa-lead reviews evidence
- Updates criterion status: ⏳ Pending → ✅ Verified (if all tests pass)
- Escalates to eng-director for gate approval

**Week 10:**
- Manual mobile device testing (real iPhone + Android)
- Video recording of full workflow
- Final criterion #2 verification

**Week 11:**
- Integration with task-17 (performance validation)
- Criterion #1, #5, #7 verification
- Full Phase 1 gate assessment

---

**Document Owner:** test-engineer-e2e  
**Last Updated:** 2026-08-15  
**Status:** Ready for Monday execution
