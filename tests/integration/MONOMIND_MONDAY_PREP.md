# Monomind Integration Test Preparation - MONDAY READY

**Date:** 2026-08-15  
**Owner:** test-engineer-e2e  
**Backend Ready:** Monday (task-7 completion)  

---

## Pre-Flight Checklist (Sunday Night / Monday Morning)

### 1. Backend Verification

**Coordinate with monomind-integration-engineer:**

- [ ] Confirm backend handlers are deployed and live
- [ ] Get API endpoint URLs:
  - [ ] Monomind detection: Where in attach response?
  - [ ] Health check endpoint: `/api/monomind/health`?
  - [ ] Dashboard endpoint: `/api/monomind/dashboard`?
  - [ ] Upgrade check endpoint: `/api/monomind/upgrade`?
  - [ ] Org status endpoint: `/api/monomind/org/status`?

- [ ] Get Protocol Buffer message specs:
  - [ ] `MonomindDetectionResponse` fields
  - [ ] `DashboardResponse` structure
  - [ ] `HealthCheckResponse` fields
  - [ ] `UpgradeCheckResponse` fields

- [ ] Verify detection.rs behavior:
  - [ ] How is working directory passed in session creation?
  - [ ] Where does suggestion flag appear (attach response)?
  - [ ] Dismiss marker file name: `.monomind-suggestion-dismissed`?

### 2. Test Implementation Tasks

**For each test, remove skip and implement assertions:**

#### Test 1: `test_monomind_detection_no_project`
```python
# Remove line 47: pytest.skip("Monomind detection not yet implemented")

# Uncomment/implement line 44:
assert response.get("monomind_suggestion") is True
# OR (if using protobuf):
# assert response.monomind_detection.suggestion_shown is True
```

#### Test 2: `test_monomind_detection_with_project`
```python
# Remove line 77: pytest.skip(...)

# Uncomment/implement line 75:
assert response.get("monomind_suggestion") is False
```

#### Test 3: `test_suggestion_dismiss_marker`
```python
# Remove line 111: pytest.skip(...)

# Implement lines 100, 109:
assert response1.get("monomind_suggestion") is True
assert response2.get("monomind_suggestion") is False
```

#### Test 4: `test_monomind_health_check`
```python
# Remove line 127: pytest.skip(...)

# Implement lines 130-138:
client = ProtocolClient(daemon_process.base_url)
await client.connect(auth_jwt=sample_jwt)

# Use actual API endpoint from backend spec
health_response = await client.send_dashboard_request("health_check")
# OR direct HTTP request to /api/monomind/health

assert health_response["type"] == "DashboardResponse"
assert "health" in health_response
assert health_response["health"]["status"] in ["healthy", "degraded", "unhealthy"]
```

#### Test 5: `test_monomind_dashboard_session_status`
```python
# Remove line 151: pytest.skip(...)

# Implement lines 154-165 (see commented code)
```

#### Test 6: `test_monomind_upgrade_check`
```python
# Remove line 178: pytest.skip(...)

# Implement lines 181-190 (see commented code)
```

#### Test 7: `test_monomind_org_status`
```python
# Remove line 202: pytest.skip(...)

# Implement org status query and assertions
```

### 3. Protocol Client Updates

**May need to add methods to `tests/common/protocol.py`:**

```python
class ProtocolClient:
    async def send_dashboard_request(self, request_type: str) -> dict:
        """Send dashboard API request via WebSocket or HTTP"""
        # Implementation depends on backend API design
        pass
    
    async def send_health_check_request(self) -> dict:
        """Request monomind health check"""
        pass
    
    async def send_upgrade_check_request(self) -> dict:
        """Request monomind upgrade check"""
        pass
```

### 4. Backend API Spec Questions

**Questions to ask monomind-integration-engineer:**

1. **Detection Flow:**
   - How is session working directory specified in create/attach request?
   - Where does `monomind_suggestion` flag appear in response?
   - Is it in AttachResponse or a separate message type?

2. **Dashboard API:**
   - WebSocket messages or HTTP REST endpoints?
   - Authentication: same JWT or separate?
   - Response format: JSON or Protocol Buffers?

3. **Health Check:**
   - Synchronous or async health check?
   - Does it run automatically or only on request?
   - What health metrics are returned?

4. **Upgrade Check:**
   - Does it call out to GitHub/npm registry?
   - Cached results or live check?
   - One-click upgrade flow or just notification?

5. **Org Status:**
   - What org information is exposed?
   - Agent count, run status, memory usage?
   - Real-time updates or snapshot?

---

## Test Execution Plan (Monday)

**Time estimate: 4-6 hours (with potential fixes)**

### Phase 1: Smoke Test (30 minutes)
```bash
# Verify daemon is running with Monomind support
cargo run --package monoterminal-master

# Check logs for Monomind initialization
# Expected: "Monomind bridge initialized"

# Quick connectivity test
pytest tests/integration/test_monomind_integration.py::test_monomind_health_check -v -s
```

### Phase 2: Detection Tests (1-2 hours)
```bash
pytest tests/integration/test_monomind_integration.py::test_monomind_detection_no_project -v -s
pytest tests/integration/test_monomind_integration.py::test_monomind_detection_with_project -v -s
pytest tests/integration/test_monomind_integration.py::test_suggestion_dismiss_marker -v -s
```

**Expected issues:**
- Working directory not passed correctly → Fix session creation
- Suggestion flag in wrong place → Update assertion
- Dismiss marker not detected → Check file path

### Phase 3: Dashboard Tests (1-2 hours)
```bash
pytest tests/integration/test_monomind_integration.py::test_monomind_health_check -v -s
pytest tests/integration/test_monomind_integration.py::test_monomind_dashboard_session_status -v -s
pytest tests/integration/test_monomind_integration.py::test_monomind_upgrade_check -v -s
pytest tests/integration/test_monomind_integration.py::test_monomind_org_status -v -s
```

**Expected issues:**
- Endpoint URLs wrong → Update from backend spec
- Protocol Buffer deserialization → Fix message types
- Authentication issues → Verify JWT is passed

### Phase 4: Evidence Collection (1 hour)
- [ ] Screenshot: Suggestion banner in web client
- [ ] Screenshot: Embedded dashboard (session status)
- [ ] Screenshot: Health check results
- [ ] Video: Suggestion dismiss workflow (3 minutes)
- [ ] Test report: pytest HTML output

### Phase 5: Gate Verification (30 minutes)

**Criterion #3: Monomind Suggestion**
- [ ] Suggestion fires when session created in non-monomind directory ✅
- [ ] Dismiss marker stops suggestion ✅
- [ ] Screenshot evidence collected ✅
- [ ] Video evidence collected ✅

**Criterion #4: Embedded Dashboard**
- [ ] Dashboard accessible via web client (no separate service) ✅
- [ ] Shows session count and status ✅
- [ ] Health check runs without errors ✅
- [ ] Screenshot evidence collected ✅
- [ ] Verify NO separate port/service for monomind ✅

---

## Escalation Plan

**If tests fail on Monday:**

1. **Protocol mismatch:** File issue with rust-backend-lead + rust-engineer-protocol
2. **Detection not working:** Coordinate with monomind-integration-engineer (same-day fix)
3. **Dashboard API issues:** rust-backend-lead (backend WebSocket handler)
4. **Frontend issues:** frontend-lead (if web client doesn't render dashboard)

**Communication:**
- Post status updates in org chat every 2 hours
- Escalate blockers immediately (don't wait for EOD)
- Goal: All 7 tests passing by EOD Monday

---

## Success Criteria

**Monday EOD:**
- ✅ All 7 Monomind tests passing
- ✅ Criterion #3 verified with evidence
- ✅ Criterion #4 verified with evidence
- ✅ Phase 1 gate: 2 of 7 criteria complete
- ✅ Test report published to docs/phase1-evidence/

**Confidence:** HIGH (backend 85% complete, tests are well-structured)

---

## Notes

- Keep this checklist updated during Monday execution
- Document any API spec deviations discovered
- Update tests/README.md with final Monomind test status
- Prepare summary report for qa-lead EOD Monday
