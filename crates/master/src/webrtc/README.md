# WebRTC P2P Networking Module

**Phase 2 Implementation - ADR-011**

This module implements WebRTC-based peer-to-peer networking for MONOTERMINAL, enabling direct client-to-master connections with NAT traversal.

## Architecture

**Hub-and-Spoke Topology:**
- Clients connect to master via WebRTC
- No client-to-client connections (Phase 2 scope)
- Master acts as hub, clients as spokes

**Dual-Transport Strategy:**
- WebSocket: Baseline (always available)
- WebRTC DataChannel: P2P optimized (lower latency, no server bandwidth)
- Both active concurrently
- Client deduplicates by sequence_number
- Instant fallback if WebRTC fails

## Modules

### `config.rs`
WebRTC configuration with STUN/TURN servers.

```rust
use monoterminal_master::webrtc::WebRtcConfig;

let config = WebRtcConfig {
    stun_servers: StunServerConfig::default(), // Google STUN
    ice_gathering_timeout: Duration::from_secs(10),
    negotiation_timeout: Duration::from_secs(15),
    ..Default::default()
};
```

### `peer_connection.rs`
WebRTC peer connection wrapper (offerer/answerer roles).

```rust
use monoterminal_master::webrtc::PeerConnection;

// Create peer connection (client side)
let (conn, ice_rx, msg_rx) = PeerConnection::new_as_offerer(config).await?;

// Create data channel
conn.create_data_channel("monoterminal").await?;

// Create SDP offer
let sdp = conn.create_offer().await?;
// Send SDP to peer via WebSocket...

// Receive SDP answer
conn.set_remote_answer(answer_sdp).await?;

// Send data
conn.send(b"Hello, peer!").await?;
```

### `handshake.rs`
Ed25519-signed PeerHandshake protocol (challenge-response).

```rust
use monoterminal_master::webrtc::{PeerHandshake, HandshakeVerifier};
use ed25519_dalek::SigningKey;

// Client creates handshake
let handshake = PeerHandshake::new(&signing_key)?;

// Master verifies
let mut verifier = HandshakeVerifier::new();
let response = verifier.verify(&handshake)?;

if response.accepted {
    println!("Nonce: {}", response.nonce);
}
```

### `ice.rs`
ICE candidate gathering with STUN support.

```rust
use monoterminal_master::webrtc::{IceCandidateGatherer, probe_stun_server};

// Gather ICE candidates
let (gatherer, candidates_rx) = IceCandidateGatherer::new(config);
let candidates = gatherer.gather_with_timeout().await?;

// Health check
let healthy = probe_stun_server("stun:stun.l.google.com:19302", Duration::from_secs(5)).await?;
```

### `transport.rs`
Dual-transport abstraction (WebSocket + DataChannel).

```rust
use monoterminal_master::webrtc::DualTransport;

let transport = DualTransport::new(websocket_tx);

// Add WebRTC peer after negotiation
transport.set_webrtc_peer(peer_connection).await;

// Send via both transports (client deduplicates)
transport.send_dual(data).await?;

// Or send via preferred transport only
transport.send_preferred(data).await?;
```

### `mod.rs`
Prometheus metrics for WebRTC monitoring.

```rust
use monoterminal_master::webrtc::WebRtcMetrics;
use prometheus::Registry;

let registry = Registry::new();
let metrics = WebRtcMetrics::new(&registry)?;

// Track connection attempt
metrics.webrtc_attempts_total.inc();
metrics.webrtc_success_total.inc();
metrics.update_success_rate();

// Connection state: 0=disconnected, 1=connecting, 2=connected, 3=failed
metrics.webrtc_connection_state.set(2.0);
```

## Health Endpoints

**`/health`** - JSON health status:
```json
{
  "status": "healthy",
  "checks": {
    "websocket": {"status": "healthy"},
    "stun": {"status": "healthy"},
    "turn": null,
    "directory": null
  },
  "timestamp": "2026-08-19T12:00:00Z"
}
```

**`/metrics`** - Prometheus metrics:
```
# HELP webrtc_success_rate WebRTC connection success rate (0-1)
# TYPE webrtc_success_rate gauge
webrtc_success_rate 0.85

# HELP webrtc_attempts_total Total WebRTC connection attempts
# TYPE webrtc_attempts_total counter
webrtc_attempts_total 100

# HELP stun_health_status STUN server health (0=unknown, 1=healthy, 2=unhealthy)
# TYPE stun_health_status gauge
stun_health_status 1
```

## Connection Lifecycle

1. **WebSocket Baseline** (always established first)
   ```
   Client → TLS handshake → Master
   Client → AttachRequest → Master
   ✅ Terminal working via WebSocket
   ```

2. **WebRTC Upgrade** (P2P optimization)
   ```
   Client → PeerHandshake → Master
   Client ← PeerHandshakeResponse ← Master (nonce)
   Client → WebRTCOffer → Master
   Client ← WebRTCAnswer ← Master (TURN credentials)
   Client ↔ ICECandidate ↔ Master (trickle)
   ✅ DataChannel open, dual-transport active
   ```

3. **Failover** (WebRTC timeout/failure)
   ```
   DataChannel closes → continue on WebSocket
   ✅ Zero user impact
   ```

## NAT Traversal Strategy

**Tier 1: STUN Direct (0-10s)**
- Google STUN: stun.l.google.com:19302
- Target: 70-85% success (home WiFi, cellular)

**Tier 2: TURN Relay (10-15s)** *(Week 3-4)*
- Self-hosted coturn: turn.monoterminal.io:3478
- Target: 98-99% success (symmetric NATs, VPNs)

**Tier 3: WebSocket Fallback (Always)**
- Existing WebSocket connection
- Target: 100% (no P2P, terminal keeps working)

## Testing

Run WebRTC tests:
```bash
cargo test --lib webrtc
```

Run specific test:
```bash
cargo test --lib webrtc::handshake::tests::test_handshake_create_and_verify
```

Integration tests:
```bash
cargo test --lib webrtc::tests::integration_tests
```

## Dependencies

- `webrtc = "0.9"` - WebRTC implementation
- `prometheus = "0.13"` - Metrics
- `mdns-sd = "0.10"` - mDNS discovery (Week 5-6)
- `ed25519-dalek = "2"` - Ed25519 signatures
- `tokio` - Async runtime

## Timeline

**Week 1-2: WebRTC Foundation** ✅ (Day 1 complete)
- WebRTC DataChannel integration
- PeerHandshake protocol
- ICE gathering (STUN only)
- Dual-transport management
- Health endpoints + metrics

**Week 3-4: NAT Traversal**
- TURN server deployment
- TURN credential generation
- Failover testing

**Week 5-6: Discovery Services**
- mDNS service advertisement
- Directory service API

**Week 7-8: Optimization**
- zstd compression
- Backpressure handling
- Performance benchmarks

## Security

**Ed25519 Peer Authentication:**
- All handshakes signed with Ed25519
- 30-second timestamp window (replay attack prevention)
- Protocol version binding

**TURN Credentials:**
- 15-minute TTL (time-limited)
- HMAC-SHA256 signed
- Scoped to peer_id

**TLS 1.3:**
- WebSocket baseline always encrypted
- WebRTC DTLS for DataChannel

## References

- **ADR-011:** P2P Networking Architecture
- **SRS §2.3:** P2P Networking
- **SRS §7.2:** Phase 2 Acceptance Criteria
- **webrtc-rs:** https://github.com/webrtc-rs/webrtc

## Support

Questions? Contact:
- **networking-engineer** (WebRTC implementation)
- **principal-architect** (architecture decisions)
- **rust-engineer-protocol** (protocol schema)
