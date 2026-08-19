# MONOTERMINAL Protocol Python Bindings

Python Protocol Buffer bindings for E2E testing of the MONOTERMINAL wire protocol.

## Overview

This directory contains:
- **Protobuf bindings**: Auto-generated from `proto/monoterminal/v1/messages.proto`
- **Protocol client**: WebSocket client with protobuf encoding/decoding
- **Helper functions**: Simplified API for common protocol operations

## Generated Files

```
tests/common/
├── monoterminal/
│   ├── __init__.py
│   └── v1/
│       ├── __init__.py
│       └── messages_pb2.py    # AUTO-GENERATED - DO NOT EDIT
├── protocol.py                 # ProtocolClient and helpers
├── daemon.py                   # Daemon process management
└── README.md                   # This file
```

## Requirements

### Python Version
- **Python 3.8+** (tested with Python 3.10)

### Dependencies
```bash
pip install protobuf>=4.25.0 websockets zstandard
```

Or install from project requirements:
```bash
pip install -r requirements-test.txt
```

## Regenerating Bindings

**When to regenerate:**
- After any changes to `proto/monoterminal/v1/messages.proto`
- After upgrading protoc version
- When CI reports protobuf version mismatch

**How to regenerate:**

### Windows
```powershell
# Install protoc (if not already installed)
$url = "https://github.com/protocolbuffers/protobuf/releases/download/v28.3/protoc-28.3-win64.zip"
$output = "$env:TEMP\protoc.zip"
Invoke-WebRequest -Uri $url -OutFile $output
Expand-Archive -Path $output -DestinationPath "$env:USERPROFILE\.protoc" -Force

# Generate bindings
& "$env:USERPROFILE\.protoc\bin\protoc.exe" `
    --python_out=tests/common/ `
    --proto_path=proto/ `
    proto/monoterminal/v1/messages.proto
```

### Linux/macOS
```bash
# Install protoc via package manager
# Ubuntu/Debian:
sudo apt install protobuf-compiler

# macOS:
brew install protobuf

# Generate bindings
protoc \
    --python_out=tests/common/ \
    --proto_path=proto/ \
    proto/monoterminal/v1/messages.proto
```

**Verify generation:**
```bash
ls tests/common/monoterminal/v1/messages_pb2.py
# Should exist with recent timestamp
```

## Usage

### Import Pattern

```python
from tests.common.protocol import ProtocolClient
from tests.common.monoterminal.v1 import (
    Envelope,
    AttachRequest,
    AttachResponse,
    InputData,
    OutputData,
    # ... other message types
)
```

### Basic Client Usage

```python
import asyncio
from tests.common.protocol import ProtocolClient

async def example():
    # Connect to daemon
    client = ProtocolClient("ws://localhost:8080/ws")
    await client.connect(auth_jwt="your-jwt-token")
    
    # Attach to session
    response = await client.send_attach_request(
        session_id="",  # Empty = new session
        rows=24,
        cols=80,
    )
    print(f"Attached to session: {response.session_id}")
    
    # Send input
    await client.send_input(b"echo hello\r\n")
    
    # Receive output
    output = await client.recv_output(wait_seconds=5.0)
    print(f"Output: {output.decode('utf-8')}")
    
    # Detach and disconnect
    await client.send_detach(response.session_id)
    await client.disconnect()

asyncio.run(example())
```

### Direct Protobuf Encoding

```python
from tests.common.protocol import encode_attach_request, decode_envelope
from tests.common.monoterminal.v1 import Envelope

# Encode a message
encoded = encode_attach_request(
    session_id="test-session",
    jwt="your-jwt-token",
    rows=24,
    cols=80,
    sequence_number=1,
)

# Decode a message
envelope = decode_envelope(encoded)
if envelope.HasField("attach_request"):
    req = envelope.attach_request
    print(f"Session ID: {req.session_id}")
```

### Accessing Message Fields

Protocol Buffer messages use attribute access:

```python
# AttachResponse
response = await client.send_attach_request("session-123")
print(response.session_id)           # string
print(response.metadata.shell_type)  # nested message
print(response.metadata.rows)        # uint32
print(len(response.scrollback))      # repeated field (list)

# Scrollback iteration
for line in response.scrollback:
    print(line.line_number, line.data.decode('utf-8'))
```

### Compression Handling

The `ProtocolClient` automatically decompresses ZSTD-compressed output:

```python
# recv_output() handles decompression transparently
output = await client.recv_output()
# Always returns uncompressed bytes
```

## Testing

### Run E2E Tests
```bash
cd tests
pytest -v e2e/test_session_flow.py
```

### Run Single Test
```bash
pytest -v e2e/test_session_flow.py::test_full_session_lifecycle
```

### With Coverage
```bash
pytest --cov=tests/common --cov-report=html e2e/
```

## Protocol Schema Version

**Current version:** `monoterminal.v1` (schema v1.0 baseline)

**Schema file:** `proto/monoterminal/v1/messages.proto`

**Compatibility:**
- Forward compatible: Old clients can ignore new fields
- Backward compatible: New clients handle missing fields gracefully
- Breaking changes require new package version (e.g., `monoterminal.v2`)

See `docs/PROTOCOL_VERSIONS.md` for full compatibility matrix.

## Troubleshooting

### Import Error: No module named 'google.protobuf'
```bash
pip install protobuf>=4.25.0
```

### Import Error: No module named 'tests.common.monoterminal'
Regenerate bindings (see "Regenerating Bindings" above).

### RuntimeError: Protobuf version mismatch
```bash
# Upgrade protobuf package
pip install --upgrade protobuf

# Regenerate bindings with matching protoc version
protoc --version  # Check version
# Download matching protobuf Python package
```

### AssertionError in tests
Check that:
1. Daemon is running (`tests/common/daemon.py`)
2. JWT token is valid (not expired)
3. Session ID exists (or use empty string for new session)
4. WebSocket URL is correct (default: `ws://localhost:8080/ws`)

## References

- **SRS §3.1.1**: Protocol message definitions
- **ADR-004**: Protocol schema evolution policy
- **Proto schema**: `proto/monoterminal/v1/messages.proto`
- **Protocol changelog**: `docs/PROTOCOL_CHANGELOG.md`
- **Version matrix**: `docs/PROTOCOL_VERSIONS.md`

## Development

### Adding New Helper Functions

Add to `protocol.py`:

```python
def encode_my_new_request(...) -> bytes:
    """Helper to create MyNewRequest envelope."""
    request = MyNewRequest(...)
    return create_envelope("my_new_request", request, sequence_number)
```

### Testing Protocol Changes

1. Update `proto/monoterminal/v1/messages.proto`
2. Regenerate bindings (see above)
3. Update `protocol.py` helpers if needed
4. Update E2E tests in `tests/e2e/`
5. Run full test suite

## CI Integration

The CI pipeline automatically:
1. Installs protoc (pinned to v28.3)
2. Regenerates bindings on every build
3. Fails if bindings are out of sync with schema
4. Runs E2E tests with real protocol encoding

**Manual check:**
```bash
# Verify bindings are up to date
protoc --python_out=tests/common/ --proto_path=proto/ proto/monoterminal/v1/messages.proto
git diff tests/common/monoterminal/
# Should show no changes if bindings are current
```
