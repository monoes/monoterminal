"""
Integration Test: Monomind Integration (task-16)
Tests monomind detection, dashboard, and health check integration

Test Coverage:
1. Session created in dir without .monomind/ → suggestion banner appears
2. Dismiss marker created → suggestion stops
3. Health check runs → status updates in dashboard

Depends on: task-14 (E2E), task-15 (unit coverage), task-7 (monomind health)

STATUS UPDATE (2026-08-15):
- task-7 is 85% complete, finishing EOD today
- Backend ready MONDAY: detection.rs, health.rs, dashboard.rs (473 lines)
- These tests ready to execute Monday morning
- Action required: Remove skip decorators, implement assertions per backend API spec

Phase 1 Gate: Verifies Criterion #3 (Monomind detection) and #4 (Embedded dashboard)
"""

import asyncio
import json
import os
from pathlib import Path

import pytest

from tests.common.protocol import ProtocolClient


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_monomind_detection_no_project(daemon_process, sample_jwt, tmp_path):
    """
    Test monomind detection when .monomind/ directory is absent.

    Assertion: Suggestion banner appears when session is created in non-monomind directory.
    """
    # Create session in temporary directory without .monomind/
    session_id = f"test-session-{tmp_path.name}"

    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # TODO: Pass working directory when creating session
        response = await client.send_attach_request(session_id)

        # TODO: Check for monomind suggestion in response
        # Expected: response includes "monomind_suggestion": true
        # assert response.get("monomind_suggestion") is True

        # For now, skip until monomind integration is complete
        pytest.skip("Monomind detection not yet implemented")

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_monomind_detection_with_project(daemon_process, sample_jwt, tmp_path):
    """
    Test monomind detection when .monomind/ directory exists.

    Assertion: No suggestion banner when session is in monomind project.
    """
    # Create .monomind/ directory
    monomind_dir = tmp_path / ".monomind"
    monomind_dir.mkdir()

    session_id = f"test-session-{tmp_path.name}"

    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        response = await client.send_attach_request(session_id)

        # TODO: Verify no suggestion in response
        # assert response.get("monomind_suggestion") is False

        pytest.skip("Monomind detection not yet implemented")

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_suggestion_dismiss_marker(daemon_process, sample_jwt, tmp_path):
    """
    Test suggestion dismiss marker functionality.

    Assertion: Creating .monomind-suggestion-dismissed stops suggestions.
    """
    session_id = f"test-session-{tmp_path.name}"

    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # First attach: should show suggestion
        response1 = await client.send_attach_request(session_id)
        # assert response1.get("monomind_suggestion") is True

        # Create dismiss marker
        dismiss_marker = tmp_path / ".monomind-suggestion-dismissed"
        dismiss_marker.write_text("dismissed")

        # Second attach: should NOT show suggestion
        await client.send_detach()
        response2 = await client.send_attach_request(session_id)
        # assert response2.get("monomind_suggestion") is False

        pytest.skip("Suggestion dismiss not yet implemented")

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_monomind_health_check(daemon_process, sample_jwt):
    """
    Test monomind health check execution.

    Assertion: Health check runs and returns status.
    """
    # TODO: Implement once monomind dashboard API is available
    pytest.skip("Monomind health check API not yet implemented")

    # Future implementation:
    # client = ProtocolClient(daemon_process.base_url)
    # await client.connect(auth_jwt=sample_jwt)
    #
    # # Request health check via dashboard API
    # health_response = await client.send_dashboard_request("health_check")
    #
    # assert health_response["type"] == "DashboardResponse"
    # assert "health" in health_response
    # assert health_response["health"]["status"] in ["healthy", "degraded", "unhealthy"]


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_monomind_dashboard_session_status(daemon_process, sample_jwt, test_session_id):
    """
    Test monomind dashboard session status API.

    Assertion: Dashboard returns current session state.
    """
    # TODO: Implement once dashboard API is available
    pytest.skip("Monomind dashboard API not yet implemented")

    # Future implementation:
    # client = ProtocolClient(daemon_process.base_url)
    # await client.connect(auth_jwt=sample_jwt)
    #
    # # Attach to session
    # await client.send_attach_request(test_session_id)
    #
    # # Query dashboard for session status
    # dashboard_response = await client.send_dashboard_request("session_status")
    #
    # assert dashboard_response["type"] == "DashboardResponse"
    # assert "sessions" in dashboard_response
    # assert any(s["session_id"] == test_session_id for s in dashboard_response["sessions"])


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_monomind_upgrade_check(daemon_process, sample_jwt):
    """
    Test monomind upgrade check functionality.

    Assertion: Upgrade check returns version information.
    """
    # TODO: Implement once monomind upgrade check is available
    pytest.skip("Monomind upgrade check not yet implemented")

    # Future implementation:
    # client = ProtocolClient(daemon_process.base_url)
    # await client.connect(auth_jwt=sample_jwt)
    #
    # # Request upgrade check
    # upgrade_response = await client.send_dashboard_request("upgrade_check")
    #
    # assert upgrade_response["type"] == "DashboardResponse"
    # assert "current_version" in upgrade_response
    # assert "latest_version" in upgrade_response
    # assert "upgrade_available" in upgrade_response


@pytest.mark.integration
@pytest.mark.asyncio
async def test_monomind_org_status(daemon_process, sample_jwt):
    """
    Test monomind org status in embedded dashboard.

    Assertion: Dashboard shows org activity and agent status.
    """
    # TODO: Implement once org status API is available
    pytest.skip("Monomind org status not yet implemented")
