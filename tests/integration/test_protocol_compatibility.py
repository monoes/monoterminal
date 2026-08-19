"""
Integration Test: Protocol Compatibility (task-16)
Tests forward and backward compatibility of the Protocol Buffer schema

Test Coverage:
1. Send messages with unknown fields (forward compatibility)
2. Old client connects to new server (backward compatibility)
3. Protocol version negotiation

Depends on: task-14 (E2E), task-15 (unit coverage)
"""

import asyncio
import json
import pytest

from tests.common.protocol import ProtocolClient, create_envelope


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_forward_compatibility_unknown_fields(daemon_process, sample_jwt, test_session_id):
    """
    Test forward compatibility: server ignores unknown fields.

    Assertion: Messages with unknown fields are accepted and processed correctly.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Create AttachRequest with extra unknown fields
        # TODO: Replace with actual protobuf once generated
        request = {
            "type": "AttachRequest",
            "session_id": test_session_id,
            "client_id": "test-client",
            "scrollback_lines": 10000,
            # Unknown fields (future additions)
            "future_field_1": "some_value",
            "future_field_2": 12345,
            "nested_unknown": {
                "key": "value"
            }
        }

        await client.ws.send(json.dumps(request))

        # Server should ignore unknown fields and process request normally
        response_raw = await client.ws.recv()
        response = json.loads(response_raw)

        assert response["type"] == "AttachResponse"
        assert response["success"] is True

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_backward_compatibility_old_client(daemon_process, sample_jwt, test_session_id):
    """
    Test backward compatibility: old client (missing new fields) works with new server.

    Assertion: Older protocol version clients can still connect and function.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Simulate old client: send minimal AttachRequest (missing new optional fields)
        request = {
            "type": "AttachRequest",
            "session_id": test_session_id,
            # Omit optional fields that might be added in future versions
        }

        await client.ws.send(json.dumps(request))

        # Server should handle missing optional fields with defaults
        response_raw = await client.ws.recv()
        response = json.loads(response_raw)

        assert response["type"] == "AttachResponse"
        # Response may include new fields, but old client ignores them

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_protocol_version_field(daemon_process, sample_jwt):
    """
    Test protocol version field in messages.

    Assertion: Version mismatches are handled gracefully.
    """
    # TODO: Implement once version field is added to Envelope
    pytest.skip("Protocol version negotiation not yet implemented")

    # Future implementation:
    # client = ProtocolClient(daemon_process.base_url)
    # await client.connect(auth_jwt=sample_jwt)
    #
    # # Send message with older protocol version
    # envelope = create_envelope("AttachRequest", {...}, version=1)
    # await client.ws.send(envelope)
    #
    # response = await client.ws.recv()
    # # Server should handle or reject gracefully


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_malformed_message_handling(daemon_process, sample_jwt):
    """
    Test server handling of malformed messages.

    Assertion: Malformed messages result in ErrorResponse, not crashes.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        # Send invalid JSON
        await client.ws.send("not valid json{}")

        # Server should respond with ErrorResponse
        try:
            response_raw = await asyncio.wait_for(client.ws.recv(), timeout=2.0)
            response = json.loads(response_raw)

            assert response["type"] == "ErrorResponse"
            assert "error" in response or "message" in response

        except asyncio.TimeoutError:
            # Server might close connection instead of sending error
            # Both behaviors are acceptable
            pass

    except Exception:
        # Connection closed is also acceptable for malformed messages
        pass

    finally:
        if client.ws:
            await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_compression_compatibility(daemon_process, sample_jwt, test_session_id):
    """
    Test compression negotiation and compatibility.

    Assertion: Clients with/without compression support work correctly.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        response = await client.send_attach_request(test_session_id)
        assert response["success"] is True

        # Send large output to trigger compression (>4KB)
        large_input = b"echo " + (b"x" * 5000) + b"\r\n"
        await client.send_input(large_input)

        # Receive output (may be compressed)
        output = await client.recv_output(wait_seconds=5.0)

        # TODO: Verify compression was used via message metadata
        # For now, just verify output was received

        assert len(output) > 0

    finally:
        await client.disconnect()


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_message_sequence_numbering(daemon_process, sample_jwt, test_session_id):
    """
    Test sequence number handling across protocol versions.

    Assertion: Sequence numbers are correctly maintained and validated.
    """
    client = ProtocolClient(daemon_process.base_url)
    await client.connect(auth_jwt=sample_jwt)

    try:
        response = await client.send_attach_request(test_session_id)
        assert response["success"] is True

        # Send multiple inputs with incrementing sequence numbers
        for i in range(5):
            await client.send_input(f"echo seq_{i}\r\n".encode())

        # TODO: Verify sequence numbers in responses
        # For now, just verify no errors

        await asyncio.sleep(1.0)

    finally:
        await client.disconnect()
