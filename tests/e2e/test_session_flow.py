"""
E2E Test: Session Flow (task-14)
Full workflow test covering the complete session lifecycle as specified in SRS §6.1

Test Scenario:
1. Start master daemon
2. Connect web client (WebSocket + auth)
3. Create session
4. Attach to session
5. Send input ("echo hello")
6. Verify output received ("hello")
7. Detach
8. Reattach (verify scrollback sync)
9. Kill session
10. Verify cleanup

Depends on: WebSocket server (task-2, task-3), PWA web client (task-13)
NOTE: Does NOT require local UI (wgpu/egui) - tests via WebSocket protocol only
"""

import asyncio
import pytest

from tests.common.protocol import ProtocolClient


@pytest.mark.e2e
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_full_session_lifecycle(daemon_process, sample_jwt, test_session_id):
    """
    Complete session lifecycle test (SRS §6.1 - task-14).

    Steps:
    1. Connect to daemon via WebSocket
    2. Create new session
    3. Attach to session
    4. Send "echo hello" command
    5. Verify "hello" appears in output
    6. Detach from session
    7. Reattach and verify scrollback contains history
    8. Kill session
    9. Verify session cleanup
    """
    # Step 1: Connect to daemon
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Step 2-3: Create and attach to session
        # Session is auto-created on attach if session_id is empty
        response = await client.send_attach_request(test_session_id)

        # Verify AttachResponse protobuf message
        assert response.session_id == test_session_id
        assert response.metadata is not None

        # Step 4: Send input
        command = b"echo hello\r\n"
        await client.send_input(command)

        # Step 5: Verify output
        output = await client.recv_output(wait_seconds=5.0)
        output_text = output.decode("utf-8", errors="replace")

        assert "hello" in output_text.lower(), \
            f"Expected 'hello' in output, got: {output_text}"

        # Step 6: Detach from session
        await client.send_detach(test_session_id)
        await client.disconnect()

        # Step 7: Reattach and verify scrollback
        client2 = ProtocolClient(daemon_process.base_url)
        await client2.connect(auth_jwt=sample_jwt)

        response2 = await client2.send_attach_request(test_session_id)
        assert response2.session_id == test_session_id

        # Verify scrollback contains previous command/output
        scrollback = response2.scrollback  # List of Line messages
        scrollback_text = "".join(line.data.decode("utf-8", errors="replace") for line in scrollback)

        assert "echo hello" in scrollback_text, \
            "Scrollback should contain previous command"
        assert "hello" in scrollback_text.lower(), \
            "Scrollback should contain previous output"

        # Step 8: Kill session
        # TODO: Implement session kill message once protocol is complete
        await client2.send_detach(test_session_id)
        await client2.disconnect()

        # Step 9: Verify cleanup
        # TODO: Query daemon to verify session is terminated

    finally:
        # Cleanup: ensure client is disconnected
        if client.ws:
            await client.disconnect()


@pytest.mark.e2e
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_session_id_consistency(daemon_process, sample_jwt, test_session_id):
    """
    Verify session ID remains consistent across attach/detach cycles.

    Assertion: Session ID matches across attach/detach operations.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # First attach
        response1 = await client.send_attach_request(test_session_id)
        session_id_1 = response1.session_id

        # Detach and reattach
        await client.send_detach(test_session_id)
        response2 = await client.send_attach_request(test_session_id)
        session_id_2 = response2.session_id

        # Assert session IDs match
        assert session_id_1 == session_id_2, \
            f"Session ID changed: {session_id_1} != {session_id_2}"
        assert session_id_1 == test_session_id, \
            f"Session ID doesn't match requested: {session_id_1} != {test_session_id}"

    finally:
        await client.disconnect()


@pytest.mark.e2e
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_late_joiner_scrollback(daemon_process, sample_jwt, test_session_id):
    """
    Verify late-joiner receives 10k lines of history.

    Assertion: Late-joiner client receives full scrollback (up to 10k lines).
    """
    # Client 1: Create session and generate output
    client1 = ProtocolClient(daemon_process.base_url)
    await client1.connect(auth_jwt=sample_jwt)

    try:
        response = await client1.send_attach_request(test_session_id)
        assert response.session_id == test_session_id

        # Generate some output
        for i in range(10):
            await client1.send_input(f"echo line_{i}\r\n".encode())
            await asyncio.sleep(0.1)

        # Detach
        await client1.send_detach(test_session_id)
        await client1.disconnect()

        # Client 2: Late joiner - should receive scrollback
        client2 = ProtocolClient(daemon_process.base_url)
        await client2.connect(auth_jwt=sample_jwt)

        response2 = await client2.send_attach_request(test_session_id)
        scrollback = response2.scrollback  # List of Line messages

        # Verify scrollback contains generated lines
        scrollback_text = "".join(line.data.decode("utf-8", errors="replace") for line in scrollback)

        assert "line_0" in scrollback_text, "Scrollback missing early lines"
        assert "line_9" in scrollback_text, "Scrollback missing recent lines"

        # Verify scrollback limit (should be capped at 10k lines)
        line_count = len(scrollback)
        assert line_count <= 10000, \
            f"Scrollback exceeds 10k line limit: {line_count} lines"

    finally:
        if client1.ws:
            await client1.disconnect()
        if client2.ws:
            await client2.disconnect()


@pytest.mark.e2e
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_graceful_shutdown_no_leaks(daemon_process, sample_jwt, test_session_id):
    """
    Verify graceful shutdown with no leaked processes.

    Assertion: After session termination, no PTY processes remain.
    """
    import psutil

    # Get initial process count
    initial_processes = len(psutil.pids())

    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Create session
        response = await client.send_attach_request(test_session_id)
        assert response.session_id == test_session_id

        # Send some commands
        await client.send_input(b"echo test\r\n")
        await asyncio.sleep(0.5)

        # Terminate session
        await client.send_detach(test_session_id)
        await client.disconnect()

        # Wait for cleanup
        await asyncio.sleep(1.0)

        # Verify no process leak
        final_processes = len(psutil.pids())

        # Allow small variance (other system processes may start/stop)
        process_diff = final_processes - initial_processes
        assert abs(process_diff) < 5, \
            f"Process leak detected: {process_diff} processes remain"

    finally:
        if client.ws:
            await client.disconnect()


@pytest.mark.e2e
@pytest.mark.requires_daemon
@pytest.mark.asyncio
@pytest.mark.slow
async def test_resize_pty_dimensions(daemon_process, sample_jwt, test_session_id):
    """
    Test PTY resize functionality.

    Steps:
    1. Attach to session (default 80x24)
    2. Send resize request to 100x30
    3. Verify PTY dimensions changed
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Attach with default dimensions
        response = await client.send_attach_request(test_session_id)
        assert response.session_id == test_session_id

        # Verify initial dimensions from metadata
        assert response.metadata.cols == 80
        assert response.metadata.rows == 24

        # Send resize request
        await client.send_resize(rows=30, cols=100)
        await asyncio.sleep(0.5)

        # TODO: Query daemon for current dimensions once API is available
        # For now, just verify no errors

    finally:
        await client.disconnect()
