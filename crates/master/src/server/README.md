# WebSocket Server with TLS 1.3

## Overview

This module implements the WebSocket server with TLS 1.3 support for MONOTERMINAL, as specified in:
- **SRS §3.1.2**: WebSocket Framing
- **SRS §3.2.1**: Transport Security (TLS 1.3 only)
- **SRS §3.1.4**: Output Buffering & Flow Control

## Architecture

### Components

1. **`mod.rs`**: Main server entry point
   - TCP listener on `127.0.0.1:5000` (Phase 1 local only)
   - Connection limiting (1000 concurrent max)
   - Rate limiting (100 connections/minute)

2. **`tls.rs`**: TLS 1.3 configuration
   - rustls-based TLS acceptor
   - TLS 1.3 only (rejects TLS 1.2)
   - Cipher suites: TLS_AES_256_GCM_SHA384, TLS_AES_128_GCM_SHA256, TLS_CHACHA20_POLY1305_SHA256
   - Self-signed certificates for development (TOFU model)

3. **`handler.rs`**: Protocol message handler (task-3)
   - Protocol Buffer decode/encode
   - Message routing (AttachRequest, InputData, ResizeRequest, etc.)
   - Error handling with typed ErrorResponse

4. **`connection.rs`**: Connection state management
   - Per-client output buffer (1 MB bounded queue)
   - Backpressure detection
   - Lagging client handling

5. **`error.rs`**: Error types

## Setup

### Generate TLS Certificate (Development)

```powershell
# Run from repository root
.\scripts\gen-tls-cert.ps1
```

This creates:
- `certs/server.crt` - Self-signed certificate
- `certs/server.key` - Private key (RSA 2048-bit)

Valid for 365 days, CN=localhost, SAN: localhost, 127.0.0.1

### Run Server

```bash
cargo run --bin monoterminal
```

The server will:
1. Listen on `127.0.0.1:5000`
2. Accept TLS 1.3 connections only
3. Upgrade to WebSocket
4. Process Protocol Buffer messages

## Protocol Flow

```
Client                          Server
  │                               │
  │  ──── TCP Connect ────►       │
  │  ──── TLS 1.3 Handshake ──►   │
  │  ◄─── TLS 1.3 Server Hello──  │
  │                               │
  │  ──── WebSocket Upgrade ──►   │
  │  ◄─── 101 Switching Protocols │
  │                               │
  │  ──── Binary Frame ────►      │  (Protocol Buffer Envelope)
  │     (Envelope.AttachRequest)  │
  │                               │
  │  ◄─── Binary Frame ────       │  (Protocol Buffer Envelope)
  │     (Envelope.AttachResponse) │
```

## Message Types (task-3)

Implemented in `handler.rs`:

### Client → Server

- **AttachRequest**: Attach to session (create if empty session_id)
- **InputData**: Keyboard input to PTY
- **ResizeRequest**: Resize terminal dimensions
- **DetachRequest**: Detach from session
- **DashboardRequest**: Get monomind dashboard data

### Server → Client

- **AttachResponse**: Session metadata + scrollback
- **OutputData**: PTY output chunks (with compression support)
- **ErrorResponse**: Error with typed ErrorCode
- **DashboardResponse**: Monomind status

## Implemented Features

✅ **task-2: WebSocket Server with TLS 1.3**
- TLS 1.3 only (rustls)
- WebSocket upgrade (tokio-tungstenite)
- Connection limiting (1000 max)
- Local bind (127.0.0.1:5000)

✅ **task-3: Protocol Runtime Integration**
- Protocol Buffer decode/encode (prost)
- Message routing
- Error handling
- Response generation

✅ **Connection Management**
- Per-client bounded queue (1 MB)
- Lagging detection
- Disconnect after 30s lagging

## TODO: Future Work

### Compression (SRS §3.1.3)

```rust
// TODO: Implement zstd compression
// Triggers:
// 1. Output chunk > 4KB
// 2. Client advertises compression support
// 3. Client buffer > 50% full

use zstd;

fn compress_output(data: &[u8]) -> Result<Vec<u8>> {
    zstd::encode_all(data, 3) // Level 3 per SRS
}
```

### Backpressure (SRS §3.1.4)

```rust
// TODO: Implement full backpressure handling
// 1. Enable compression if buffer > 50% full
// 2. Send LAGGING warning if > 80% full for > 5s
// 3. Drop oldest output if buffer full (lossy)
// 4. Disconnect if lagging > 30s (IMPLEMENTED)
```

### Rate Limiting (SRS §3.2.4)

```rust
// TODO: Implement rate limiting with tower
use tower::limit::RateLimit;

// 100 connections/minute
// 1000 auth attempts/hour
// 10000 input messages/minute per client
```

### Fuzzing (SRS §6.1)

```bash
# TODO: Set up cargo-fuzz for protocol parser
cargo fuzz init
cargo fuzz add protobuf_parser
cargo fuzz run protobuf_parser
```

### Benchmarking

```bash
# TODO: Benchmark protocol overhead with criterion
# Target: 10-20 byte fixed overhead
# Target: 500-1000 MB/s throughput

cargo bench --bench protocol_overhead
```

## Testing

```bash
# Unit tests
cargo test --lib

# Integration tests (requires ConPTY implementation)
cargo test --test integration

# Run with logging
RUST_LOG=debug cargo test
```

## Dependencies

- `tokio`: Async runtime
- `tokio-tungstenite`: WebSocket
- `rustls`: TLS 1.3
- `tokio-rustls`: Tokio integration
- `rustls-pemfile`: Certificate loading
- `prost`: Protocol Buffers
- `futures-util`: Stream utilities

## References

- SRS: `docs/monoterminal-srs.md`
- Protocol Schema: `proto/monoterminal/v1/messages.proto`
- Architecture: `docs/phase1-backend-implementation-plan.md`
