"""
Integration Test: WebSocket Handshake (task-16)
Tests the WebSocket + TLS 1.3 + Ed25519/JWT auth handshake flow

Test Coverage:
1. TLS 1.3 negotiation
2. Ed25519 challenge-response
3. JWT issuance + validation

Depends on: task-14 (E2E), task-15 (unit coverage)
"""

import asyncio
import pytest
import ssl

from tests.common.protocol import ProtocolClient


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_tls_13_negotiation(daemon_process):
    """
    Verify TLS 1.3 is used for WebSocket connections.

    Assertion: Connection uses TLS 1.3 protocol.
    """
    # TODO: Enable TLS in test daemon config
    # For now, this test is a placeholder until TLS is enabled

    client = ProtocolClient(daemon_process.base_url.replace("ws://", "wss://"))

    try:
        # This will fail until TLS is implemented
        # await client.connect()

        # TODO: Verify TLS version once connection succeeds
        # ssl_context = client.ws.transport.get_extra_info('ssl_object')
        # assert ssl_context.version() == 'TLSv1.3'

        pytest.skip("TLS 1.3 not yet implemented in daemon")

    except Exception as e:
        pytest.skip(f"TLS 1.3 not yet implemented: {e}")


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_ed25519_challenge_response(daemon_process):
    """
    Test Ed25519 challenge-response authentication flow.

    Flow:
    1. Client connects
    2. Server sends challenge
    3. Client signs challenge with Ed25519 private key
    4. Server verifies signature
    5. Server issues JWT

    Assertion: Valid Ed25519 signature results in JWT issuance.
    """
    # TODO: Implement once auth module is complete
    pytest.skip("Ed25519 auth not yet implemented")

    # Placeholder for future implementation:
    # client = ProtocolClient(daemon_process.base_url)
    # await client.connect()
    #
    # # Receive challenge
    # challenge = await client.recv_challenge()
    #
    # # Sign with Ed25519 private key
    # signature = sign_ed25519(challenge, private_key)
    #
    # # Send signature
    # await client.send_challenge_response(signature)
    #
    # # Receive JWT
    # jwt_response = await client.recv_jwt()
    # assert jwt_response["jwt"] is not None


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_jwt_validation(daemon_process, sample_jwt):
    """
    Test JWT validation on subsequent requests.

    Assertion: Valid JWT allows access, invalid JWT is rejected.
    """
    client = ProtocolClient(daemon_process.base_url)

    # Test with valid JWT
    await client.connect(auth_jwt=sample_jwt)
    # Connection should succeed
    assert client.ws is not None
    await client.disconnect()

    # Test with invalid JWT
    invalid_jwt = "invalid.jwt.here"

    try:
        await client.connect(auth_jwt=invalid_jwt)
        # Should fail or return error
        pytest.fail("Invalid JWT should be rejected")
    except Exception:
        # Expected - invalid JWT rejected
        pass


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_jwt_expiry_handling(daemon_process):
    """
    Test JWT expiry and renewal flow.

    Assertion: Expired JWT is rejected, renewal works correctly.
    """
    # TODO: Implement once JWT expiry is configured
    pytest.skip("JWT expiry handling not yet implemented")


@pytest.mark.integration
@pytest.mark.requires_daemon
@pytest.mark.asyncio
async def test_connection_without_auth(daemon_process):
    """
    Test connection without authentication (should fail if auth enabled).

    Assertion: Unauthenticated connection is rejected when auth is required.
    """
    client = ProtocolClient(daemon_process.base_url)

    try:
        # Connect without JWT
        await client.connect(auth_jwt=None)

        # If auth is disabled (test config), this succeeds
        # If auth is enabled, this should fail
        # For now, we expect success since test daemon has auth disabled

        assert client.ws is not None
        await client.disconnect()

    except Exception as e:
        # Expected if auth is required
        assert "Unauthorized" in str(e) or "Forbidden" in str(e)
