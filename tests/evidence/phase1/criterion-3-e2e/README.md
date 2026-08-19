# Criterion #3: E2E Test Infrastructure & Evidence

**Phase 1 Gate Criterion:** E2E functional tests validating full terminal lifecycle

**Status:** ✅ INFRASTRUCTURE READY (awaiting Monday execution by qa-lead)

---

## Test Infrastructure Overview

### Test Layers

```
┌─────────────────────────────────────────────────────────────┐
│  E2E Browser Tests (Playwright)                             │
│  - xterm.js rendering validation                            │
│  - Visual screenshot comparison                             │
│  - Cross-browser compatibility                              │
│  - Mobile viewport testing                                  │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│  E2E Protocol Tests (WebSocket + Protobuf)                  │
│  - Session lifecycle (attach/detach/reattach)               │
│  - Input/output flow                                        │
│  - Resize handling                                          │
│  - Multi-client scenarios                                   │
└─────────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────────┐
│  Integration Tests (Component-level)                        │
│  - WebSocket handshake                                      │
│  - Auth (Ed25519/JWT)                                       │
│  - Protocol compatibility                                   │
└─────────────────────────────────────────────────────────────┘
```

### Test Scenarios Covered

#### 1. Basic Terminal Spawn & Execution ✅
- **File:** `tests/e2e/test_session_flow.py::test_full_session_lifecycle`
- **Validates:**
  - WebSocket connection establishment
  - Session creation
  - Command input: `echo hello`
  - Output verification
  - Session cleanup

#### 2. Terminal Resize & Reflow ✅
- **File:** `tests/e2e/test_browser_rendering.py::test_terminal_resize_reflow`
- **Validates:**
  - PTY resize message sent to backend
  - xterm.js dimension adaptation
  - Content reflow correctness
  - No visual artifacts

#### 3. Scrollback Navigation ✅
- **File:** `tests/e2e/test_session_flow.py::test_late_joiner_scrollback`
- **File:** `tests/e2e/test_browser_rendering.py::test_scrollback_navigation`
- **Validates:**
  - 10k line scrollback limit
  - Late-joiner receives full history
  - Scroll up/down navigation
  - Scrollbar position accuracy

#### 4. Multiple Concurrent Sessions ✅
- **File:** `tests/e2e/test_browser_rendering.py::test_multi_session_ui`
- **File:** `tests/e2e/test_soak.py::test_multi_client_soak`
- **Validates:**
  - Multiple sessions in browser UI
  - Session switching
  - Independent session state
  - No cross-session contamination

#### 5. Session Cleanup on Disconnect ✅
- **File:** `tests/e2e/test_session_flow.py::test_graceful_shutdown_no_leaks`
- **Validates:**
  - No PTY process leaks
  - File descriptor cleanup
  - Memory cleanup
  - WebSocket connection closure

---

## Execution Guide

### Prerequisites

```powershell
# 1. Install Python dependencies
cd tests
pip install -r requirements.txt

# 2. Install Playwright browsers
playwright install chromium firefox webkit

# 3. Build Rust daemon
cd ..
cargo build --release

# 4. Install web client dependencies
cd web-client
npm ci
```

### Run E2E Tests (Monday Execution)

```powershell
# Start web client dev server (Terminal 1)
cd web-client
npm run dev

# Run E2E tests (Terminal 2)
cd tests

# Option 1: Full E2E suite (protocol + browser)
pytest -v -m e2e --html=evidence/phase1/e2e-report.html

# Option 2: Protocol tests only (faster)
pytest -v -m "e2e and not slow" tests/e2e/test_session_flow.py

# Option 3: Browser rendering tests
pytest -v tests/e2e/test_browser_rendering.py

# Option 4: Cross-browser compatibility
pytest -v tests/e2e/test_browser_rendering.py --browser=chromium --browser=firefox --browser=webkit
```

### Run Soak Test (24 Hours)

```powershell
# Run in CI or dedicated environment
pytest -v -m soak tests/e2e/test_soak.py::test_24h_stability
```

---

## Evidence Collection

### Automated Evidence Capture

All E2E tests automatically collect:

1. **Screenshots**
   - Initial terminal state
   - After each major action
   - On test failure (automatic)
   - Saved to: `tests/evidence/phase1/run-<timestamp>/`

2. **WebSocket Traffic Logs**
   - Message direction (sent/received)
   - Message types (AttachRequest, InputData, etc.)
   - Payload sizes
   - Sequence numbers
   - Saved to: `websocket-traffic.json`

3. **Performance Metrics**
   - Latency measurements
   - Memory usage
   - CPU usage
   - File descriptor counts
   - Saved to: `metrics.json`

4. **HTML Reports**
   - Test summary
   - Screenshot gallery
   - Performance charts
   - Saved to: `evidence-report.html`

### Manual Evidence Collection

```python
from tests.common.evidence import EvidenceCollector

# In test function:
async def test_my_scenario(evidence_dir, playwright_page):
    collector = EvidenceCollector(evidence_dir, "my-test")
    
    # Capture screenshot
    await collector.capture_screenshot(playwright_page, "before-action")
    
    # Record metric
    collector.record_metric("latency", 12.5, "ms")
    
    # Record WebSocket message
    collector.record_websocket_message("sent", "AttachRequest", 256)
    
    # Finalize (generates HTML report)
    report_path = collector.finalize()
```

---

## Test Fixtures

### Available Fixtures

| Fixture | Scope | Description |
|---------|-------|-------------|
| `daemon_process` | function | Master daemon instance with random port |
| `playwright_page` | function | Chromium browser page |
| `firefox_browser` | session | Firefox browser instance |
| `webkit_browser` | session | WebKit/Safari browser instance |
| `mobile_page` | function | Mobile viewport browser page |
| `evidence_dir` | function | Timestamped evidence directory |
| `sample_jwt` | function | Sample JWT token for auth |
| `test_session_id` | function | UUID session ID |

### Custom Fixture Example

```python
@pytest.fixture
async def my_custom_fixture(daemon_process, sample_jwt):
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)
    yield client
    await client.disconnect()
```

---

## CI Integration

### GitHub Actions Matrix

**File:** `.github/workflows/e2e-tests.yml`

**Matrix Dimensions:**
- **OS:** Windows (Phase 1), Linux (Phase 3), macOS (Phase 3)
- **Arch:** x86_64 (Phase 1), aarch64 (Phase 3)
- **Browser:** Chromium, Firefox, WebKit (Safari)

**Trigger:**
- Push to `main` or `develop`
- Pull requests
- Daily at 2 AM UTC (scheduled)
- Weekly soak test (Saturday 2 AM)

### CI Workflow Status

Current configuration:
- ✅ Windows x86_64 (Phase 1 - active)
- ⏳ Linux x86_64 (Phase 3 - scheduled, main branch only)
- ⏳ macOS x86_64 (Phase 3 - scheduled, main branch only)
- ⏳ Windows/Linux/macOS aarch64 (Phase 3)

---

## Performance Targets (SRS §6.1)

| Metric | Target | Validation |
|--------|--------|------------|
| **Attach Latency** | <30ms (LAN p95) | Measured in `test_session_flow.py` |
| **Input Latency** | <16ms (60 FPS) | Measured in browser rendering tests |
| **Scrollback Sync** | <100ms | Measured in late-joiner test |
| **Resize Latency** | <50ms | Measured in resize test |
| **Session Cleanup** | <1s | Measured in graceful shutdown test |

### Latency Measurement

```python
from tests.common.evidence import PerformanceMonitor

async def test_latency(evidence_dir, daemon_process):
    collector = EvidenceCollector(evidence_dir, "latency-test")
    perf = PerformanceMonitor(collector)
    
    # Measure attach latency
    client = ProtocolClient(daemon_process.base_url)
    await perf.measure_async("attach", client.send_attach_request("test-session"))
    
    # Get stats
    stats = perf.get_latency_stats()
    assert stats["p95"] < 30.0, f"Attach p95 latency {stats['p95']}ms exceeds 30ms target"
```

---

## Browser Compatibility

### Supported Browsers (SRS §1.2)

| Browser | Minimum Version | Test Status |
|---------|----------------|-------------|
| Chrome | 90+ | ✅ Automated (Chromium) |
| Firefox | 88+ | ✅ Automated |
| Safari | 14+ | ✅ Automated (WebKit) |
| Edge | 90+ | ✅ Covered by Chromium |
| Android Chrome | Latest | ✅ Mobile viewport simulation |
| iOS Safari | 14+ | ✅ Mobile viewport simulation |

### Cross-Browser Test Execution

```powershell
# Run same test across all browsers
pytest -v tests/e2e/test_browser_rendering.py \
  --browser=chromium \
  --browser=firefox \
  --browser=webkit
```

---

## Soak Test (24 Hours)

### Test Parameters

- **Duration:** 24 hours (86,400 seconds)
- **Sample Interval:** 60 seconds (1,440 data points)
- **Scenarios:**
  - Continuous session (persistent WebSocket)
  - Periodic reconnections (every hour)
  - Periodic input (every 5 minutes)

### Success Criteria

- ✅ No daemon crashes
- ✅ Memory growth < 100 MB
- ✅ No file descriptor leaks (+20 FD tolerance)
- ✅ Session survives reconnections
- ✅ No WebSocket connection accumulation

### Soak Test Output

- **Metrics CSV:** `soak-test-metrics-<timestamp>.csv` (1,440 rows)
- **HTML Report:** `soak-test-report-<timestamp>.html`
- **Evidence:** Screenshots at T+0h, T+12h, T+24h

---

## Troubleshooting

### Common Issues

#### 1. Daemon fails to start
```
AssertionError: Daemon failed to start
```

**Fix:**
```powershell
# Check if port is already in use
netstat -ano | findstr :8080

# Build daemon in debug mode for better errors
cargo build --bin monoterminal-master
```

#### 2. Web client not responding
```
TimeoutError: Page.goto: Timeout 30000ms exceeded
```

**Fix:**
```powershell
# Ensure dev server is running
cd web-client
npm run dev

# Wait for server to start
Start-Sleep -Seconds 10
```

#### 3. Playwright browser not installed
```
Error: browserType.launch: Executable doesn't exist
```

**Fix:**
```powershell
playwright install chromium
```

#### 4. WebSocket connection refused
```
ConnectionRefusedError: [Errno 111] Connection refused
```

**Fix:**
```powershell
# Check daemon config (tests/conftest.py)
# Ensure listen_address is "127.0.0.1:0" (random port)
```

---

## Monday Execution Checklist (qa-lead)

### Pre-Execution

- [ ] Verify Python 3.11+ installed
- [ ] Verify Rust toolchain installed
- [ ] Install dependencies: `pip install -r tests/requirements.txt`
- [ ] Install Playwright browsers: `playwright install chromium firefox`
- [ ] Build daemon: `cargo build --release`
- [ ] Install web client deps: `cd web-client && npm ci`

### Execution

- [ ] Start web client: `cd web-client && npm run dev`
- [ ] Run E2E suite: `pytest -v -m e2e --html=evidence/phase1/e2e-report.html`
- [ ] Verify all tests pass (or document failures)
- [ ] Collect evidence artifacts from `tests/evidence/phase1/`

### Post-Execution

- [ ] Review HTML reports
- [ ] Archive screenshots
- [ ] Document any failures in `tests/evidence/phase1/criterion-3-results.md`
- [ ] Update Phase 1 gate tracking

---

## Evidence Archive Structure

```
tests/evidence/phase1/
├── run-20260816-090000/          # Timestamped run directory
│   ├── terminal-spawn-initial.png
│   ├── terminal-spawn-with-output.png
│   ├── terminal-before-resize.png
│   ├── terminal-after-resize.png
│   ├── scrollback-top.png
│   ├── scrollback-bottom.png
│   ├── multi-session-1.png
│   ├── multi-session-2.png
│   ├── websocket-traffic.json
│   ├── metrics.json
│   ├── test-log.txt
│   └── evidence-report.html
├── e2e-report.html               # pytest-html report
├── e2e-report.json               # pytest-json-report
└── criterion-3-results.md        # qa-lead summary (created Monday)
```

---

## References

- **SRS §6.1:** Testing & Quality Strategy
- **SRS §5.3:** Performance metrics and targets
- **Protocol Schema:** `proto/monoterminal/v1/messages.proto`
- **Test Plan:** `docs/e2e-monday-execution-plan.md`

---

**Infrastructure Prepared By:** test-engineer-e2e  
**Date:** 2026-08-16  
**Status:** Ready for Monday execution by qa-lead (task-8)
