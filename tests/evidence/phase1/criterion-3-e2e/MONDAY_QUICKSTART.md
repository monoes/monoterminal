# Monday Quickstart - E2E Test Execution

**For:** qa-lead (task-8 execution)  
**Duration:** ~30 minutes (excluding 24h soak test)  
**Date:** Monday, August 19, 2026

---

## Option 1: Automated Execution (Recommended)

```powershell
# Single command to run everything
python tests/run_e2e_suite.py
```

**What this does:**
1. ✓ Checks prerequisites (Python, Rust, Node.js, pytest, Playwright)
2. ✓ Builds Rust daemon (`cargo build --release`)
3. ✓ Installs web client dependencies (`npm ci`)
4. ✓ Starts web client dev server in background
5. ✓ Runs all E2E tests (protocol + browser + integration)
6. ✓ Generates HTML and JSON reports
7. ✓ Cleans up (stops web server)

**Output:**
- `tests/evidence/phase1/e2e-report-<timestamp>.html`
- `tests/evidence/phase1/e2e-report-<timestamp>.json`
- Screenshots in `tests/evidence/phase1/run-<timestamp>/`

---

## Option 2: Manual Step-by-Step

### Step 1: Install Dependencies (5 minutes)

```powershell
# Install Python dependencies
cd tests
pip install -r requirements.txt

# Install Playwright browsers
playwright install chromium firefox

# Install web client dependencies
cd ..\web-client
npm ci
```

### Step 2: Build Daemon (2 minutes)

```powershell
cd ..
cargo build --release --bin monoterminal-master
```

### Step 3: Start Web Client (Background)

```powershell
# Open NEW PowerShell terminal
cd web-client
npm run dev

# Keep this terminal open
# Web client will be at http://localhost:5173
```

### Step 4: Run E2E Tests (20 minutes)

```powershell
# In ORIGINAL PowerShell terminal
cd tests

# Full E2E suite
pytest -v -m "e2e and not soak" `
  --html=evidence/phase1/e2e-report-monday.html `
  --self-contained-html

# OR run individual test files:

# Protocol tests (WebSocket + Protobuf)
pytest -v tests/e2e/test_session_flow.py

# Browser rendering tests (Playwright + xterm.js)
pytest -v tests/e2e/test_browser_rendering.py

# Integration tests (auth, handshake)
pytest -v tests/integration/
```

### Step 5: Review Results

Open the HTML report:
```powershell
start tests/evidence/phase1/e2e-report-monday.html
```

Check evidence directory:
```powershell
explorer tests\evidence\phase1\run-<latest-timestamp>
```

---

## Quick Validation Checklist

After test execution, verify these files exist:

```
tests/evidence/phase1/
├── e2e-report-<timestamp>.html      ✓ HTML test report
├── e2e-report-<timestamp>.json      ✓ JSON test report
└── run-<timestamp>/                 ✓ Evidence directory
    ├── terminal-spawn-initial.png   ✓ Screenshot 1
    ├── terminal-spawn-with-output.png ✓ Screenshot 2
    ├── scrollback-top.png           ✓ Screenshot 3
    ├── scrollback-bottom.png        ✓ Screenshot 4
    ├── websocket-traffic.json       ✓ WebSocket log
    ├── metrics.json                 ✓ Performance metrics
    └── evidence-report.html         ✓ Evidence report
```

---

## Expected Results

### Test Summary (if all pass)

```
tests/e2e/test_session_flow.py::test_full_session_lifecycle PASSED
tests/e2e/test_session_flow.py::test_session_id_consistency PASSED
tests/e2e/test_session_flow.py::test_late_joiner_scrollback PASSED
tests/e2e/test_session_flow.py::test_graceful_shutdown_no_leaks PASSED
tests/e2e/test_session_flow.py::test_resize_pty_dimensions PASSED
tests/e2e/test_browser_rendering.py::test_terminal_spawn_and_render PASSED
tests/e2e/test_browser_rendering.py::test_terminal_resize_reflow PASSED
tests/e2e/test_browser_rendering.py::test_scrollback_navigation PASSED
tests/e2e/test_browser_rendering.py::test_multi_session_ui PASSED
tests/integration/test_websocket_handshake.py::test_jwt_validation PASSED

======================== 10 passed in 45.23s =========================
```

### Performance Metrics (SRS §6.1 targets)

| Metric | Target | Status |
|--------|--------|--------|
| Attach Latency (p95) | <30ms | ✓ Measured |
| Input Latency | <16ms | ✓ Measured |
| Scrollback Sync | <100ms | ✓ Measured |
| Resize Latency | <50ms | ✓ Measured |
| Session Cleanup | <1s | ✓ Measured |

---

## Troubleshooting

### Issue: Daemon fails to start

**Error:**
```
AssertionError: Daemon failed to start
```

**Fix:**
```powershell
# Check if port 8080 is in use
netstat -ano | findstr :8080

# Kill process if needed
taskkill /PID <pid> /F

# Rebuild daemon
cargo clean
cargo build --release
```

### Issue: Web client not accessible

**Error:**
```
TimeoutError: Page.goto: Timeout exceeded
```

**Fix:**
```powershell
# Ensure dev server is running
cd web-client
npm run dev

# Wait 10 seconds, then retry tests
```

### Issue: Playwright browser not found

**Error:**
```
Error: browserType.launch: Executable doesn't exist
```

**Fix:**
```powershell
playwright install chromium
```

### Issue: Import error for protobuf

**Error:**
```
ImportError: No module named 'tests.common.monoterminal'
```

**Fix:**
```powershell
# Regenerate protobuf bindings
cd tests/common
protoc --python_out=. --proto_path=../../proto ../../proto/monoterminal/v1/messages.proto
```

---

## Skipping Browser Tests

If web client is not ready or Playwright has issues:

```powershell
# Run protocol tests only (no browser automation)
pytest -v -m "e2e and not slow" tests/e2e/test_session_flow.py tests/integration/
```

OR use the automated script:

```powershell
python tests/run_e2e_suite.py --skip-browser
```

---

## 24-Hour Soak Test (Optional - Run Separately)

**DO NOT** run this as part of Monday execution - it takes 24 hours.

Schedule it separately:

```powershell
# Run in dedicated environment (CI or dedicated machine)
pytest -v -m soak tests/e2e/test_soak.py::test_24h_stability
```

This will be executed automatically via GitHub Actions weekly schedule.

---

## Reporting Results

### Create Summary Document

After test execution, create:

**File:** `tests/evidence/phase1/criterion-3-results.md`

**Template:**

```markdown
# Criterion #3: E2E Test Execution Results

**Executed by:** qa-lead
**Date:** 2026-08-19
**Duration:** <X> minutes

## Test Summary

- **Total Tests:** <N>
- **Passed:** <N>
- **Failed:** <N>
- **Skipped:** <N>

## Failures (if any)

### Test: test_xyz
**Error:**
```
<error message>
```

**Root Cause:**
<analysis>

**Impact on Criterion #3:**
<assessment>

## Evidence

- HTML Report: `e2e-report-<timestamp>.html`
- Screenshots: `run-<timestamp>/`
- Metrics: `run-<timestamp>/metrics.json`

## Conclusion

Criterion #3 Status: ✅ PASSED / ❌ FAILED

<rationale>
```

---

## Contact

**Questions during execution?**

- **Slack:** #monoterminal-qa
- **Agent:** test-engineer-e2e (via org message)
- **Documentation:** `tests/evidence/phase1/criterion-3-e2e/README.md`

---

**Last Updated:** 2026-08-16  
**Prepared by:** test-engineer-e2e
