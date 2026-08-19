# MONOTERMINAL E2E and Integration Tests

**Test Engineer E2E Owner:** test-engineer-e2e  
**Tasks:** task-14 (E2E), task-16 (Integration)  
**Status:** Infrastructure ready, blocked on task-6 & task-13

---

## Overview

This directory contains the Python + pytest E2E and integration test suite for MONOTERMINAL Phase 1 (Windows + Web MVP). Tests cover the complete session lifecycle, multi-client scenarios, protocol compatibility, and monomind integration as specified in SRS §6.1.

---

## Test Structure

```
tests/
├── README.md                    # This file
├── requirements.txt             # Python dependencies
├── pytest.ini                   # Pytest configuration
├── conftest.py                  # Shared fixtures and setup
├── common/                      # Test utilities
│   ├── __init__.py
│   ├── protocol.py             # Protocol Buffer client utilities
│   └── daemon.py               # Daemon management utilities
├── e2e/                        # End-to-end tests (task-14)
│   └── test_session_flow.py   # Full session lifecycle tests
└── integration/                # Integration tests (task-16)
    ├── test_websocket_handshake.py    # TLS + Ed25519 + JWT
    ├── test_multi_client_attach.py    # Multi-client scenarios
    ├── test_protocol_compatibility.py # Forward/backward compat
    └── test_monomind_integration.py   # Monomind detection/dashboard
```

---

## Quick Start

### 1. Install Dependencies

```powershell
# Create virtual environment
python -m venv .venv
.\.venv\Scripts\Activate.ps1

# Install dependencies
pip install -r tests/requirements.txt
```

### 2. Build Master Daemon

```powershell
# Build master daemon (required for E2E tests)
cargo build --package monoterminal-master
```

### 3. Run Tests

```powershell
# Run all E2E tests
pytest tests/e2e/ -v

# Run all integration tests
pytest tests/integration/ -v

# Run all tests
pytest tests/ -v

# Run specific test
pytest tests/e2e/test_session_flow.py::test_full_session_lifecycle -v

# Run tests in parallel (faster)
pytest tests/ -n auto

# Run with coverage
pytest tests/ --cov=tests/common --cov-report=html
```

---

## Test Categories (Markers)

Tests are marked with pytest markers for selective execution:

| Marker | Description | Command |
|--------|-------------|---------|
| `@pytest.mark.e2e` | End-to-end tests (full workflow) | `pytest -m e2e` |
| `@pytest.mark.integration` | Integration tests (multi-component) | `pytest -m integration` |
| `@pytest.mark.requires_daemon` | Tests that need master daemon | `pytest -m requires_daemon` |
| `@pytest.mark.slow` | Slow tests (>10 seconds) | `pytest -m "not slow"` to skip |
| `@pytest.mark.smoke` | Quick smoke tests | `pytest -m smoke` |
| `@pytest.mark.windows` | Windows-only tests | `pytest -m windows` |

**Example:**
```powershell
# Run only fast E2E tests
pytest -m "e2e and not slow" -v
```

---

## Test Status

### ✅ Ready (Infrastructure Complete)

- [x] Pytest configuration and fixtures
- [x] WebSocket protocol client utilities
- [x] Daemon process management
- [x] Test structure and templates
- [x] E2E test skeletons (5 tests)
- [x] Integration test skeletons (15+ tests)

### ⏳ Blocked (Waiting on Dependencies)

#### task-14: E2E Session Flow Tests
**Blocked by:**
- task-6: wgpu + egui master UI (needed for daemon to run)
- task-13: PWA manifest & service worker

**What's needed:**
- Working master daemon binary (`target/debug/monoterminal-master.exe`)
- WebSocket server listening on configurable port
- Basic session create/attach/detach API
- Protocol Buffer message encoding/decoding

#### task-16: Integration Tests
**Blocked by:**
- task-14: E2E tests (dependency chain)
- task-15: Unit test coverage to 70%

**What's needed:**
- TLS 1.3 support in daemon
- Ed25519 challenge-response auth
- JWT issuance and validation
- Multi-client broadcast logic
- Monomind health check API
- Dashboard endpoint

---

## Test Implementation Checklist

### task-14: E2E Session Flow Tests

- [x] Test infrastructure setup
- [x] `test_full_session_lifecycle` - Complete 10-step workflow
- [x] `test_session_id_consistency` - Session ID across attach/detach
- [x] `test_late_joiner_scrollback` - 10k line history
- [x] `test_graceful_shutdown_no_leaks` - Process cleanup
- [x] `test_resize_pty_dimensions` - PTY resize
- [ ] **Execute tests** (blocked on task-6, task-13)
- [ ] **Fix any failures**
- [ ] **Verify all assertions pass**

### task-16: Integration Tests

#### WebSocket Handshake
- [x] `test_tls_13_negotiation`
- [x] `test_ed25519_challenge_response`
- [x] `test_jwt_validation`
- [x] `test_jwt_expiry_handling`
- [x] `test_connection_without_auth`

#### Multi-Client Attach
- [x] `test_two_clients_same_session`
- [x] `test_fan_out_broadcast`
- [x] `test_presence_notifications`
- [x] `test_client_limit_enforcement`
- [x] `test_concurrent_input_handling`

#### Protocol Compatibility
- [x] `test_forward_compatibility_unknown_fields`
- [x] `test_backward_compatibility_old_client`
- [x] `test_protocol_version_field`
- [x] `test_malformed_message_handling`
- [x] `test_compression_compatibility`
- [x] `test_message_sequence_numbering`

#### Monomind Integration
- [x] `test_monomind_detection_no_project`
- [x] `test_monomind_detection_with_project`
- [x] `test_suggestion_dismiss_marker`
- [x] `test_monomind_health_check`
- [x] `test_monomind_dashboard_session_status`
- [x] `test_monomind_upgrade_check`
- [x] `test_monomind_org_status`

- [ ] **Execute all integration tests** (blocked on task-14, task-15)
- [ ] **Replace JSON placeholders with actual Protocol Buffers**
- [ ] **Fix any failures**
- [ ] **Verify 25% of test pyramid coverage**

---

## Current Limitations & TODOs

### Protocol Buffers
Currently using JSON as a placeholder. Need to:
- [ ] Generate Python bindings from `.proto` files
- [ ] Replace `ProtocolClient` JSON encoding with protobuf
- [ ] Update `create_envelope()` and `decode_envelope()` functions

### Authentication
Currently using placeholder JWT:
- [ ] Implement Ed25519 key generation in tests
- [ ] Implement challenge-response signing
- [ ] Update `sample_jwt` fixture with real JWT

### Daemon Process Management
Currently hardcoded port 8080:
- [ ] Parse actual port from daemon stdout
- [ ] Implement health check polling
- [ ] Add retry logic for daemon startup

### Monomind Integration
Tests are stubbed out:
- [ ] Wait for task-7 (monomind health check) completion
- [ ] Update tests with actual dashboard API endpoints
- [ ] Verify detection logic with real `.monomind/` directory

---

## CI Integration

Tests will be integrated into `.github/workflows/test.yml`:

```yaml
e2e:
  name: E2E Tests
  runs-on: windows-2022
  steps:
    - uses: actions/checkout@v4
    
    - name: Setup Python
      uses: actions/setup-python@v4
      with:
        python-version: '3.11'
    
    - name: Install dependencies
      run: pip install -r tests/requirements.txt
    
    - name: Build master daemon
      run: cargo build --package monoterminal-master
    
    - name: Run E2E tests
      run: pytest tests/ -v --junit-xml=test-results.xml
    
    - name: Upload test results
      if: always()
      uses: actions/upload-artifact@v3
      with:
        name: test-results
        path: test-results.xml
```

---

## Contributing

When adding new tests:

1. **Follow naming convention:** `test_<feature>_<scenario>`
2. **Add appropriate markers:** `@pytest.mark.e2e`, `@pytest.mark.integration`
3. **Use fixtures:** Leverage `daemon_process`, `sample_jwt`, `test_session_id`
4. **Document assertions:** Clearly state what the test verifies
5. **Handle cleanup:** Use `try/finally` to ensure resources are released

---

## SRS References

- **§6.1:** Testing Pyramid (70% unit, 25% integration, 5% E2E)
- **§7.1:** Phase 1 Acceptance Criteria (70% coverage, zero crashes)
- **§3.1.1:** Protocol Buffer schema (message types)
- **§4.2.1:** WebSocket + TLS 1.3 transport
- **§4.2.2:** Ed25519 + JWT authentication

---

## Contact

**Questions?** Contact `test-engineer-e2e` in the monoterminal-dev org.

**Issues?** Create a task via `org_task` or message `qa-lead`.
