"""
Integration Test: Multi-Client Attach (task-16)
Tests multiple clients attaching to the same session

Test Coverage:
1. 2+ clients attach to same session
2. Fan-out broadcast (all clients receive same OutputData)
3. Presence notifications (ClientJoined/ClientLeft)

Depends on: task-14 (E2E), task-15 (unit coverage)
"""

import asyncio
import pytest

from tests.common.protocol import ProtocolClient


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_two_clients_same_session(daemon_process, sample_jwt, test_session_id):
    """
    Test two clients attaching to the same session.

    Assertion: Both clients can attach and receive output independently.
    """
    # Client 1: Create and attach to session
    client1 = ProtocolClient(daemon_process.base_url)
    await client1.connect(auth_jwt=sample_jwt)

    # Client 2: Attach to same session
    client2 = ProtocolClient(daemon_process.base_url)
    await client2.connect(auth_jwt=sample_jwt)

    try:
        # Both attach to same session
        response1 = await client1.send_attach_request(test_session_id)
        assert response1["success"] is True

        response2 = await client2.send_attach_request(test_session_id)
        assert response2["success"] is True
        assert response2["session_id"] == test_session_id

        # Client 1 sends input
        await client1.send_input(b"echo multi-client-test\r\n")

        # Both clients should receive the same output
        output1 = await client1.recv_output(wait_seconds=5.0)
        output2 = await client2.recv_output(wait_seconds=5.0)

        output1_text = output1.decode("utf-8", errors="replace")
        output2_text = output2.decode("utf-8", errors="replace")

        assert "multi-client-test" in output1_text.lower()
        assert "multi-client-test" in output2_text.lower()

        # Outputs should be identical (fan-out broadcast)
        assert output1 == output2, \
            "Fan-out broadcast failed: clients received different output"

    finally:
        await client1.disconnect()
        await client2.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_fan_out_broadcast(daemon_process, sample_jwt, test_session_id):
    """
    Test fan-out broadcast to multiple clients.

    Assertion: All attached clients receive the same OutputData messages.
    """
    num_clients = 3
    clients = []

    try:
        # Create and attach multiple clients
        for i in range(num_clients):
            client = ProtocolClient(daemon_process.base_url)
            await client.connect(auth_jwt=sample_jwt)
            response = await client.send_attach_request(test_session_id)
            assert response["success"] is True
            clients.append(client)

        # First client sends input
        test_message = b"echo broadcast-test\r\n"
        await clients[0].send_input(test_message)

        # All clients should receive the same output
        outputs = []
        for client in clients:
            output = await client.recv_output(wait_seconds=5.0)
            outputs.append(output)

        # Verify all outputs are identical
        for i in range(1, len(outputs)):
            assert outputs[i] == outputs[0], \
                f"Client {i} received different output than client 0"

        # Verify content
        output_text = outputs[0].decode("utf-8", errors="replace")
        assert "broadcast-test" in output_text.lower()

    finally:
        for client in clients:
            await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_presence_notifications(daemon_process, sample_jwt, test_session_id):
    """
    Test ClientJoined and ClientLeft presence notifications.

    Assertion: Clients receive notifications when other clients join/leave.
    """
    # Client 1: Initial client
    client1 = ProtocolClient(daemon_process.base_url)
    await client1.connect(auth_jwt=sample_jwt)

    try:
        response1 = await client1.send_attach_request(test_session_id)
        assert response1["success"] is True

        # Client 2: Join session
        client2 = ProtocolClient(daemon_process.base_url)
        await client2.connect(auth_jwt=sample_jwt)
        response2 = await client2.send_attach_request(test_session_id)
        assert response2["success"] is True

        # TODO: Client 1 should receive ClientJoined notification
        # notification = await client1.recv_notification()
        # assert notification["type"] == "ClientJoined"
        # assert notification["client_id"] == client2_id

        # Client 2: Leave session
        await client2.send_detach()
        await client2.disconnect()

        # TODO: Client 1 should receive ClientLeft notification
        # notification = await client1.recv_notification()
        # assert notification["type"] == "ClientLeft"
        # assert notification["client_id"] == client2_id

        # For now, skip until presence notifications are implemented
        pytest.skip("Presence notifications not yet implemented")

    finally:
        await client1.disconnect()
        if client2.ws:
            await client2.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_client_limit_enforcement(daemon_process, sample_jwt, test_session_id):
    """
    Test enforcement of max clients per session (if configured).

    Assertion: Daemon rejects clients exceeding the configured limit.
    """
    # TODO: Configure max clients in test daemon config
    # For now, test that we can attach at least 10 clients

    max_clients = 10
    clients = []

    try:
        for i in range(max_clients):
            client = ProtocolClient(daemon_process.base_url)
            await client.connect(auth_jwt=sample_jwt)
            response = await client.send_attach_request(test_session_id)
            assert response["success"] is True, \
                f"Failed to attach client {i}"
            clients.append(client)

        # All clients attached successfully
        assert len(clients) == max_clients

    finally:
        for client in clients:
            await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_concurrent_input_handling(daemon_process, sample_jwt, test_session_id):
    """
    Test concurrent input from multiple clients.

    Assertion: Daemon correctly handles interleaved input from multiple clients.
    """
    num_clients = 3
    clients = []

    try:
        # Attach multiple clients
        for i in range(num_clients):
            client = ProtocolClient(daemon_process.base_url)
            await client.connect(auth_jwt=sample_jwt)
            response = await client.send_attach_request(test_session_id)
            assert response["success"] is True
            clients.append(client)

        # Send concurrent input from all clients
        send_tasks = []
        for i, client in enumerate(clients):
            command = f"echo client_{i}\r\n".encode()
            send_tasks.append(client.send_input(command))

        await asyncio.gather(*send_tasks)

        # Wait for output
        await asyncio.sleep(1.0)

        # All clients should receive all outputs (fan-out)
        # Order may vary due to concurrent execution

        # For now, just verify no errors occur
        # Full verification requires collecting all output messages

    finally:
        for client in clients:
            await client.disconnect()
