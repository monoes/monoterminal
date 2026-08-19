"""
Protocol Buffer utilities for E2E tests
WebSocket client with Protocol Buffer message handling (real protobuf encoding)
"""

import asyncio
from typing import Optional, List

import websockets
from websockets.client import WebSocketClientProtocol

from tests.common.monoterminal.v1 import (
    Envelope,
    AttachRequest,
    AttachResponse,
    InputData,
    OutputData,
    ResizeRequest,
    DetachRequest,
    ErrorResponse,
    SessionMetadata,
    Line,
    CompressionType,
    ErrorCode,
)


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
        """Connect to the WebSocket server with optional JWT authentication."""
        headers = {}
        if auth_jwt:
            headers["Authorization"] = f"Bearer {auth_jwt}"

        self.ws = await websockets.connect(self.url, extra_headers=headers)

    async def disconnect(self):
        """Disconnect from the WebSocket server."""
        if self.ws:
            await self.ws.close()
            self.ws = None

    async def send_attach_request(
        self, session_id: str, rows: int = 24, cols: int = 80, jwt: str = ""
    ) -> AttachResponse:
        """
        Send AttachRequest and wait for AttachResponse.

        Args:
            session_id: Session ID to attach to (empty for new session)
            rows: Terminal rows (default 24)
            cols: Terminal columns (default 80)
            jwt: JWT authentication string

        Returns:
            AttachResponse protobuf message
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        # Create AttachRequest message
        request = AttachRequest(
            session_id=session_id,
            auth_token=jwt,  # proto field name
            rows=rows,
            cols=cols,
            last_seen_sequence=0,  # Full scrollback
        )

        # Wrap in Envelope
        envelope = Envelope(
            sequence_number=self.sequence_number, attach_request=request
        )
        self.sequence_number += 1

        # Send protobuf-encoded message
        await self.ws.send(envelope.SerializeToString())

        # Receive response
        response_bytes = await self.ws.recv()
        response_envelope = Envelope()
        response_envelope.ParseFromString(response_bytes)

        # Extract AttachResponse
        if response_envelope.HasField("attach_response"):
            return response_envelope.attach_response
        elif response_envelope.HasField("error_response"):
            raise RuntimeError(
                f"Attach failed: {response_envelope.error_response.message}"
            )
        else:
            raise ValueError(f"Unexpected response type: {response_envelope}")

    async def send_input(self, data: bytes, jwt: str = ""):
        """Send InputData message with raw bytes to the PTY."""
        if not self.ws:
            raise RuntimeError("Not connected")

        # Create InputData message
        message = InputData(data=data, auth_token=jwt)  # proto field name

        # Wrap in Envelope
        envelope = Envelope(sequence_number=self.sequence_number, input_data=message)
        self.sequence_number += 1

        await self.ws.send(envelope.SerializeToString())

    async def recv_output(self, wait_seconds: float = 5.0) -> bytes:
        """
        Receive OutputData message.

        Returns:
            Raw output bytes from PTY (decompressed if ZSTD)
        """
        if not self.ws:
            raise RuntimeError("Not connected")

        try:
            response_bytes = await asyncio.wait_for(
                self.ws.recv(), timeout=wait_seconds
            )
            envelope = Envelope()
            envelope.ParseFromString(response_bytes)

            if envelope.HasField("output_data"):
                output = envelope.output_data

                # Handle compression
                if output.compression == CompressionType.ZSTD:
                    import zstandard as zstd

                    dctx = zstd.ZstdDecompressor()
                    return dctx.decompress(output.data)
                else:
                    return output.data
            elif envelope.HasField("error_response"):
                raise RuntimeError(f"Error: {envelope.error_response.message}")
            else:
                raise ValueError(f"Expected OutputData, got {envelope}")
        except asyncio.TimeoutError:
            raise TimeoutError(f"No output received within {wait_seconds}s")

    async def send_detach(self, session_id: str):
        """Send DetachRequest to cleanly detach from session."""
        if not self.ws:
            raise RuntimeError("Not connected")

        message = DetachRequest(session_id=session_id)
        envelope = Envelope(sequence_number=self.sequence_number, detach_request=message)
        self.sequence_number += 1

        await self.ws.send(envelope.SerializeToString())

    async def send_resize(self, rows: int, cols: int, jwt: str = ""):
        """Send ResizeRequest to change PTY dimensions."""
        if not self.ws:
            raise RuntimeError("Not connected")

        message = ResizeRequest(rows=rows, cols=cols, auth_token=jwt)  # proto field name
        envelope = Envelope(sequence_number=self.sequence_number, resize_request=message)
        self.sequence_number += 1

        await self.ws.send(envelope.SerializeToString())


# ============================================================================
# Protocol Buffer encoding/decoding helper functions
# ============================================================================


def create_envelope(
    message_type: str, message_obj, sequence_number: int = 0
) -> bytes:
    """
    Create a Protocol Buffer Envelope with the specified message.

    Args:
        message_type: Type of message (e.g., "attach_request", "input_data")
        message_obj: Protobuf message object
        sequence_number: Sequence number for the envelope

    Returns:
        Encoded envelope as bytes
    """
    envelope = Envelope(sequence_number=sequence_number)
    setattr(envelope, message_type, message_obj)
    return envelope.SerializeToString()


def decode_envelope(data: bytes) -> Envelope:
    """
    Decode a Protocol Buffer Envelope.

    Args:
        data: Encoded envelope bytes

    Returns:
        Decoded Envelope protobuf message
    """
    envelope = Envelope()
    envelope.ParseFromString(data)
    return envelope


def encode_attach_request(
    session_id: str,
    jwt: str = "",
    rows: int = 24,
    cols: int = 80,
    sequence_number: int = 0,
) -> bytes:
    """Helper to create an AttachRequest envelope."""
    request = AttachRequest(
        session_id=session_id,
        auth_token=jwt,  # proto field name
        rows=rows,
        cols=cols,
        last_seen_sequence=0,
    )
    return create_envelope("attach_request", request, sequence_number)


def encode_input_data(data: bytes, jwt: str = "", sequence_number: int = 0) -> bytes:
    """Helper to create an InputData envelope."""
    input_msg = InputData(data=data, auth_token=jwt)  # proto field name
    return create_envelope("input_data", input_msg, sequence_number)


def encode_resize_request(
    rows: int, cols: int, jwt: str = "", sequence_number: int = 0
) -> bytes:
    """Helper to create a ResizeRequest envelope."""
    request = ResizeRequest(rows=rows, cols=cols, auth_token=jwt)  # proto field name
    return create_envelope("resize_request", request, sequence_number)


def encode_detach_request(session_id: str, sequence_number: int = 0) -> bytes:
    """Helper to create a DetachRequest envelope."""
    request = DetachRequest(session_id=session_id)
    return create_envelope("detach_request", request, sequence_number)
