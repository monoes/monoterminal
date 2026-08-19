"""
Protocol Buffer utilities for E2E tests
WebSocket client with Protocol Buffer message handling
"""

import asyncio
import json
from typing import Optional, Dict, Any

import websockets
from websockets.client import WebSocketClientProtocol


class ProtocolClient:
    """
    WebSocket client for MONOTERMINAL protocol.
    Handles Protocol Buffer encoding/decoding and WebSocket lifecycle.
    """

    def __init__(self, url: str):
        self.url = url
        self.ws: Optional[WebSocketClientProtocol] = None
        self.sequence_number = 0

    async def connect(self, auth_jwt: Optional[str] = None):
        """
        Connect to the WebSocket server.

        Args:
            auth_jwt: Optional JWT for authentication
        """
        headers = {}
        if auth_jwt:
            headers["Authorization"] = f"Bearer {auth_jwt}"

        self.ws = await websockets.connect(self.url, extra_headers=headers)

    async def disconnect(self):
        """Disconnect from the WebSocket server."""
        if self.ws:
            await self.ws.close()
            self.ws = None

    async def send_attach_request(self, session_id: str) -> Dict[str, Any]:
        """
        Send AttachRequest and wait for AttachResponse.

        Args:
            session_id: Session ID to attach to

        Returns:
            AttachResponse as dict (TODO: Replace with actual protobuf once generated)
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        # TODO: Replace with actual protobuf encoding once protocol crate builds Python bindings
        # For now, use JSON as a placeholder
        request = {
            "type": "AttachRequest",
            "session_id": session_id,
            "client_id": "test-client",
            "scrollback_lines": 10000,
        }

        await self.ws.send(json.dumps(request))

        response_raw = await self.ws.recv()
        response = json.loads(response_raw)

        return response

    async def send_input(self, data: bytes):
        """
        Send InputData message.

        Args:
            data: Raw input bytes to send to PTY
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        # TODO: Replace with actual protobuf encoding
        message = {
            "type": "InputData",
            "data": data.hex(),  # Hex-encode bytes for JSON
            "sequence": self.sequence_number,
        }
        self.sequence_number += 1

        await self.ws.send(json.dumps(message))

    async def recv_output(self, wait_seconds: float = 5.0) -> bytes:
        """
        Receive OutputData message.

        Args:
            wait_seconds: Maximum time to wait for output

        Returns:
            Raw output bytes from PTY
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        try:
            response_raw = await asyncio.wait_for(self.ws.recv(), timeout=wait_seconds)
            response = json.loads(response_raw)

            if response["type"] != "OutputData":
                raise ValueError(f"Expected OutputData, got {response['type']}")

            # Decode hex-encoded bytes
            return bytes.fromhex(response["data"])
        except asyncio.TimeoutError:
            raise TimeoutError(f"No output received within {wait_seconds}s")

    async def send_detach(self):
        """Send DetachRequest to cleanly detach from session."""
        if not self.ws:
            raise RuntimeError("Not connected")

        message = {
            "type": "DetachRequest",
        }

        await self.ws.send(json.dumps(message))

    async def send_resize(self, rows: int, cols: int):
        """
        Send ResizeRequest to change PTY dimensions.

        Args:
            rows: New row count
            cols: New column count
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        message = {
            "type": "ResizeRequest",
            "rows": rows,
            "cols": cols,
        }

        await self.ws.send(json.dumps(message))


# ============================================================================
# Protocol Buffer encoding/decoding utilities
# TODO: Replace these with actual protobuf bindings once generated
# ============================================================================

def create_envelope(message_type: str, payload: Dict[str, Any], sequence_number: int = 0) -> bytes:
    """
    Create a Protocol Buffer Envelope (placeholder implementation).

    Args:
        message_type: Type of message (e.g., "AttachRequest")
        payload: Message payload as dict
        sequence_number: Sequence number for the envelope

    Returns:
        Encoded envelope as bytes
    """
    # TODO: Replace with actual protobuf encoding
    envelope = {
        "sequence_number": sequence_number,
        "message": {
            "type": message_type,
            **payload
        }
    }
    return json.dumps(envelope).encode()


def decode_envelope(data: bytes) -> Dict[str, Any]:
    """
    Decode a Protocol Buffer Envelope (placeholder implementation).

    Args:
        data: Encoded envelope bytes

    Returns:
        Decoded envelope as dict
    """
    # TODO: Replace with actual protobuf decoding
    return json.loads(data)
