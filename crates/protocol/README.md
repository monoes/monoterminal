# monoterminal-protocol

Protocol Buffer schema and generated types for MONOTERMINAL wire protocol (Phase 1).

## Overview

This crate provides the Protocol Buffer message definitions for communication between MONOTERMINAL clients and the master daemon.

**Protocol Version:** v1 (Phase 1 - Windows + Web MVP)  
**Specification:** See `docs/monoterminal-srs.md` §3.1.1

## Wire Format

```
WebSocket Binary Frame
    │
    └─► Protocol Buffer Envelope (self-delimiting)
        ├─► sequence_number (uint64, monotonic counter)
        └─► message (oneof: 10 message types)
```

## Message Types

### Session Control
- **AttachRequest**: Attach to a session (new or existing)
- **AttachResponse**: Session metadata and scrollback
- **DetachRequest**: Detach from a session
- **ResizeRequest**: Update terminal dimensions

### I/O Streaming
- **InputData**: Raw keyboard input (client → server)
- **OutputData**: PTY output chunk (server → client), with optional zstd compression

### Monomind Dashboard (Phase 1)
- **DashboardRequest**: Query monomind status/agents/memory
- **DashboardResponse**: JSON response from embedded monomind

### Error Handling
- **ErrorResponse**: Error code + message

## Compression

- **Algorithm**: zstd (Zstandard)
- **Trigger**: Chunks >4KB (per SRS §3.1.3)
- **Enum**: `CompressionType::NONE` (0) or `CompressionType::ZSTD` (1)

## Sequence Numbers

Every `Envelope` includes a monotonic `sequence_number` for:
- **Ordering**: Detect out-of-order delivery
- **Deduplication**: Detect retransmitted messages
- **Gap detection**: Request missing messages

## Usage

```rust
use monoterminal_protocol::*;

// Create an envelope with a resize request
let envelope = Envelope {
    sequence_number: 42,
    message: Some(envelope::Message::ResizeRequest(ResizeRequest {
        rows: 40,
        cols: 120,
    })),
};

// Encode to bytes
use prost::Message;
let mut buf = Vec::new();
envelope.encode(&mut buf)?;

// Decode from bytes
let decoded = Envelope::decode(&buf[..])?;
```

## Build

Code generation happens automatically via `build.rs`:

```powershell
cargo build
```

Generated types are written to `src/generated/monoterminal.v1.rs` and re-exported from `lib.rs`.

## Testing

Roundtrip encode/decode tests verify all message types:

```powershell
cargo test
```

## Schema Evolution

**Phase 1 Guarantee**: This schema is additive-only. New fields may be added, but existing field numbers will never change or be removed (Protocol Buffers forward/backward compatibility).

**Future Phases**:
- Phase 2: Add P2P message types (peer discovery, relay)
- Phase 3: Platform-specific metadata (Linux/macOS)
- Phase 4: Advanced features (split panes, Sixel graphics)

## Dependencies

- `prost`: Protocol Buffer encoding/decoding
- `bytes`: Efficient byte buffer operations

## References

- SRS: `docs/monoterminal-srs.md` §3.1.1 (Protocol Buffers Schema)
- SRS: `docs/monoterminal-srs.md` §3.1.2 (WebSocket Framing)
- SRS: `docs/monoterminal-srs.md` §3.1.3 (Compression)
