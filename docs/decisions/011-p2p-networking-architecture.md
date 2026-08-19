# ADR-011: P2P Networking Architecture

**Status:** Draft — Pending Phase 1 Gate  
**Date:** 2026-08-17  
**Deciders:** principal-architect, networking-engineer  
**SRS Reference:** §2.3, §7.2 (Phase 2 Networking)  
**Phase:** Phase 2 (P2P + Persistence)

---

## Context

Phase 2 introduces peer-to-peer networking to enable:
- **Direct client-to-master connections** without relay servers (reduces latency, bandwidth costs)
- **NAT traversal** for home networks, cellular, corporate VPNs
- **Mobile browser support** (Android Chrome, iOS Safari via WebRTC browser APIs)
- **Hybrid local/internet discovery** (mDNS for LAN, directory service for WAN)

**Current State (Phase 1):**
- WebSocket-only connections (client → master daemon, always-on TCP)
- No NAT traversal (requires port forwarding or VPN)
- No peer discovery (manual IP:port configuration)

**Phase 2 Requirements (SRS §7.2):**
- 65-80% NAT traversal success rate (measured, not assumed from literature)
- 100 concurrent sessions tested
- Reconnect-after-background <10s on iOS Safari

---

## Decision

Implement **WebRTC-based P2P networking** with the following architecture:

### 1. Transport Strategy: Dual-Stack (WebSocket + WebRTC)

**Both transports active concurrently** (not exclusive):

```
Client ←─────────────────────────────────→ Master
       ↑                                   ↑
       │  WebSocket (TLS 1.3)             │
       │  - Always-on baseline            │
       │  - Signaling channel             │
       │  - Fallback transport            │
       │                                   │
       │  WebRTC DataChannel (DTLS)       │
       │  - P2P optimized path            │
       │  - Lower latency                 │
       │  - No server bandwidth cost      │
       └───────────────────────────────────┘
```

**Rationale:**
- ✅ **Instant fallback:** If DataChannel closes (mobile backgrounding, NAT rebinding), WebSocket is still alive
- ✅ **Zero user impact:** Terminal keeps working regardless of P2P success/failure
- ✅ **Clean migration:** Phase 1 clients (WebSocket-only) continue working unchanged

**Trade-off (Accepted):**
- **Memory overhead:** ~4KB per client (WebSocket + DataChannel state)
- **Bandwidth:** Master broadcasts OutputData to both transports (client deduplicates by sequence_number)
- **Optimization opportunity:** Phase 3 can close WebSocket after DataChannel proven stable (profiling needed)

---

### 2. WebRTC Topology: Hub-and-Spoke (Client-to-Master)

**Phase 2 topology:** Clients connect to master via WebRTC, NOT to each other

```
         Master Daemon
         (Hub)
        /  │  \
       /   │   \
      /    │    \
  Client A │ Client B
           │
        Client C
```

**Why NOT full mesh** (client↔client direct connections):
- ❌ **Security complexity:** Ed25519 peer authentication required for every client-client connection
- ❌ **NAT traversal amplification:** N clients = O(N²) WebRTC negotiations (10 clients = 45 connections)
- ❌ **Phase 1 master architecture:** Master owns session state (PTY, scrollback) — clients can't own sessions
- ❌ **Out of scope for Phase 2 MVP:** Collaboration (multi-client attach) doesn't require client↔client connections

**Mesh topology deferred to Phase 4+** (if collaboration latency becomes bottleneck)

**Phase 2 decision:** Stick with hub-and-spoke, optimize master relay performance

---

### 3. NAT Traversal Strategy: STUN + TURN Fallback

**3-tier strategy (per ADR-003):**

```
Tier 1: STUN Direct Connection (0-10s)
    ├─► Google STUN: stun.l.google.com:19302
    ├─► Backup STUN: stun1.l.google.com:19302
    └─► Target: 70-85% success rate (home WiFi, most cellular)

Tier 2: TURN Relay (10-15s timeout)
    ├─► Self-hosted coturn: turn.monoterminal.io:3478
    ├─► TLS: turn.monoterminal.io:5349
    └─► Target: 98-99% success rate (symmetric NATs, corporate VPNs)

Tier 3: WebSocket Fallback (Always Available)
    ├─► Existing WebSocket connection (already established)
    └─► Target: 100% (no P2P, but terminal keeps working)
```

**TURN Server Configuration:**
- **Software:** coturn (open source, RFC 5766 compliant)
- **Deployment:** DigitalOcean/AWS VPS ($5-15/month, 100GB bandwidth)
- **Credentials:** Time-limited REST API (15-minute TTL, RFC 7635)
- **Security:** Master generates HMAC-SHA256 credentials per client (binds to Ed25519 peer_id)

**Expected Success Rates (to be measured in Phase 2):**

| Network Type | STUN Direct | TURN Fallback | Total Success |
|--------------|-------------|---------------|---------------|
| Home WiFi | 85-95% | 98-99% | 98-99% |
| Cellular (4G/5G) | 60-75% | 98-99% | 98-99% |
| Corporate VPN | 40-55% | 98-99% | 98-99% |
| WebSocket Fallback | — | — | 100% (no P2P) |

**SRS §7.2 acceptance:** 65-80% NAT traversal (measured directly, not assumed)

**Measurement plan:**
- Instrument WebRTC negotiation success/failure in production
- Log network type (WiFi/cellular), NAT type (cone/symmetric), success tier (STUN/TURN/fallback)
- Report metrics via master daemon telemetry API

---

### 4. Discovery Strategy: Hybrid (mDNS + Directory Service)

**4.1 Local Discovery (mDNS/Bonjour)**

**Use case:** Same LAN (home network, office WiFi)

```
Client                           Master Daemon
  │                                    │
  ├─► mDNS Query: _monoterminal._tcp ──►
  │                                    │ Advertise: monoterminal-alice.local:9443
  │◄──────── mDNS Response ────────────┤
  │                                    │
  │   Connect via WebSocket/WebRTC    │
  └───────────────────────────────────►│
```

**mDNS Service Advertisement:**
```rust
use mdns_sd::{ServiceDaemon, ServiceInfo};

let service_info = ServiceInfo::new(
    "_monoterminal._tcp.local.",
    "monoterminal-alice",
    "alice-desktop.local.",
    "192.168.1.100",
    9443,
    &[
        ("version", "1.0"),
        ("peer_id", "ed25519:abcd1234..."),
        ("protocol", "ws+wss+webrtc"),
    ],
)?;

let mdns = ServiceDaemon::new()?;
mdns.register(service_info)?;
```

**Latency:** 1-5 seconds (mDNS query + response)

**Security:** Verify Ed25519 peer_id during WebSocket TLS handshake (prevent rogue mDNS responders)

**Limitations:**
- ❌ **LAN-only:** Doesn't traverse router NAT (no internet discovery)
- ❌ **Corporate networks:** Often block mDNS (firewall policy)

---

**4.2 Internet Discovery (Directory Service)**

**Use case:** Cross-internet (different networks, mobile roaming)

```
Client A                        Directory Service                 Master Daemon
  │                                     │                                │
  │  POST /register ───────────────────►│◄────── POST /register ─────────│
  │  {peer_id, endpoints[], ttl}        │     {peer_id, endpoints[], ttl}│
  │                                     │                                │
  │  GET /peers/{peer_id} ─────────────►│                                │
  │◄────── {endpoints[], verified} ─────┤                                │
  │                                     │                                │
  │  WebRTC Offer via WebSocket ───────────────────────────────────────►│
  └─────────────────────────────────────────────────────────────────────┘
```

**Directory Service Design:**

**Endpoints:**
- `POST /api/v1/peers/register` — Master/client registers endpoints
- `GET /api/v1/peers/{peer_id}` — Lookup peer by Ed25519 public key
- `DELETE /api/v1/peers/{peer_id}` — Deregister (graceful shutdown)

**Registration Payload:**
```json
{
  "peer_id": "ed25519:abcd1234...",
  "endpoints": [
    {"type": "websocket", "url": "wss://203.0.113.45:9443", "verified": true},
    {"type": "webrtc", "ice_servers": ["stun:stun.l.google.com:19302"]}
  ],
  "ttl_seconds": 3600,
  "signature": "ed25519_signature_over_payload"
}
```

**Security:**
- ✅ **Ed25519 signature:** All registrations signed by peer's private key (prevents spoofing)
- ✅ **Endpoint verification:** Directory pings WebSocket endpoint before marking `verified: true`
- ✅ **TTL expiry:** Entries auto-expire after 1 hour (prevents stale registrations)

**Directory Service Deployment:**
- **Phase 2 MVP:** Simple HTTP API (Axum + SQLite, deployed on same VPS as TURN server)
- **Phase 3+:** Upgrade to distributed system (Consul, etcd) if scalability needed

**Fallback if directory unavailable:**
- Client falls back to manual IP:port configuration (environment variable or config file)
- Master continues working (local-only connections via LAN IP)

---

**4.3 Discovery Priority Order**

**Client discovery flow:**

```rust
pub async fn discover_master(peer_id: &Ed25519PublicKey) -> Result<MasterEndpoint> {
    // Step 1: Try mDNS (parallel with directory, race them)
    let mdns_future = discover_via_mdns(peer_id);
    let directory_future = discover_via_directory(peer_id);
    
    // Step 2: Race mDNS vs Directory (first to respond wins)
    let endpoint = tokio::select! {
        Ok(ep) = mdns_future => {
            tracing::info!("Discovered master via mDNS: {}", ep.url);
            ep
        }
        Ok(ep) = directory_future => {
            tracing::info!("Discovered master via directory service: {}", ep.url);
            ep
        }
        else => {
            // Step 3: Fallback to manual configuration
            return get_manual_endpoint_from_config();
        }
    };
    
    // Step 4: Verify peer_id via TLS certificate (prevent MITM)
    verify_peer_identity(&endpoint, peer_id).await?;
    
    Ok(endpoint)
}
```

**Priority:**
1. **mDNS + Directory (parallel race)** — whichever responds first
2. **Manual configuration** — environment variable `MONOTERMINAL_MASTER_URL`
3. **Error:** No master found (show helpful message: "Configure master URL or ensure discovery services are reachable")

---

### 5. Connection Lifecycle

**5.1 Initial Handshake (WebSocket Baseline)**

```
Client                                Master
  │                                     │
  │  TLS 1.3 Handshake ────────────────►│ (Certificate verification)
  │◄────────────────────────────────────┤ (Ed25519 peer_id validated)
  │                                     │
  │  AttachRequest ────────────────────►│ (protocol_version=2, client_id, JWT)
  │◄────────── AttachResponse ──────────┤ (session_id, scrollback)
  │                                     │
  │  ✅ WebSocket connection established│
  │  ✅ Terminal I/O flowing             │
```

**At this point:** Client has working terminal (Phase 1 baseline)

---

**5.2 WebRTC Negotiation (P2P Upgrade)**

```
Client                                Master
  │  ✅ WebSocket connected              │
  │                                     │
  │  PeerHandshake ────────────────────►│ (protocol_version, peer_id, signature)
  │◄────── PeerHandshakeResponse ───────┤ (challenge, nonce)
  │                                     │
  │  WebRTCOffer ──────────────────────►│ (SDP, peer_id, nonce)
  │◄────────── WebRTCAnswer ────────────┤ (SDP, TURN credentials)
  │                                     │
  │  ICECandidate ←────────────────────►│ (Trickle ICE)
  │                                     │
  │  [STUN/TURN negotiation 0-15s]     │
  │                                     │
  │  ✅ DataChannel OPEN ────────────────│
  │  ✅ P2P connection established       │
  │                                     │
  │  (WebSocket remains open as fallback)│
```

**Timeout handling:**
- **15-second total timeout:** 10s STUN + 5s TURN
- If timeout expires → log failure, continue on WebSocket
- User sees no disruption (terminal already working via WebSocket)

**Sequence number continuity:**
- Master broadcasts same `sequence_number` to both WebSocket + DataChannel
- Client deduplicates by sequence (skips duplicates)
- Seamless failover if DataChannel closes mid-session

---

**5.3 Reconnection Strategy**

**Mobile backgrounding (iOS Safari, Android Chrome):**

```
Client                                Master
  │  ✅ Active session                  │
  │                                     │
  │  [User switches to another app]    │
  │  WebSocket: ping timeout (30s)     │
  │  DataChannel: DTLS timeout (60s)   │
  │                                     │
  │  [User returns to browser]         │
  │                                     │
  │  WebSocket: reconnect ─────────────►│
  │◄────── Resume from last_sequence ───┤
  │                                     │
  │  WebRTC: re-negotiate ─────────────►│ (if <10s since disconnect)
  │◄────── DataChannel re-opens ────────┤
```

**Reconnection timing (SRS §7.2 acceptance):**
- **<10s total:** WebSocket reconnect + WebRTC re-negotiation
- **WebSocket alone:** <3s (TCP + TLS handshake, resume from sequence)
- **WebRTC optional:** If >10s backgrounded, skip WebRTC (WebSocket-only mode)

**Scrollback continuity:**
- Client sends `last_seen_sequence` in AttachRequest
- Master resends missed OutputData chunks (from persistent storage or in-memory buffer)
- Client applies delta, no screen flicker

---

### 6. Bandwidth & Latency Optimization

**6.1 Compression (zstd)**

**Per ADR-004 §2.4:**
- Compress OutputData chunks >4KB (zstd level 1: ~60% ratio, <5ms latency)
- Skip compression for small messages (<4KB: InputData, PresenceUpdate)
- Compression negotiated via Envelope.compression field

**Bandwidth savings example:**
```
Uncompressed: 4KB OutputData → 4096 bytes on wire
Compressed:   4KB OutputData → 800-1200 bytes (3-5× reduction)
```

**Impact on P2P:**
- TURN relay bandwidth: 4KB → 1KB (reduce TURN server costs by 75%)
- Mobile data usage: 10MB scrollback fetch → 2-3MB (better mobile experience)

---

**6.2 Backpressure Handling**

**Problem:** Client slow consumer (mobile CPU throttling, poor network)

**Solution:** Per-client write buffer with flow control

```rust
pub struct ClientConnection {
    websocket: WebSocketSender,
    datachannel: Option<DataChannelSender>,
    write_buffer: VecDeque<Envelope>,
    max_buffer_size: usize, // 1MB limit
}

impl ClientConnection {
    pub async fn send_output(&mut self, envelope: Envelope) -> Result<()> {
        // Check backpressure
        if self.write_buffer.len() > self.max_buffer_size {
            tracing::warn!("Client {} backpressure exceeded, dropping oldest frames", self.client_id);
            self.write_buffer.pop_front(); // Drop oldest (terminal scrollback, not critical)
        }
        
        // Queue for async send
        self.write_buffer.push_back(envelope);
        
        // Flush to transport
        self.flush_write_buffer().await?;
        Ok(())
    }
}
```

**Backpressure threshold:** 1MB (per ADR-004, triggers compression if >50% full)

**Consequence:** Slow clients may lose old scrollback chunks (acceptable for Phase 2 MVP)

**Phase 3 optimization:** Add explicit flow control protocol (PAUSE/RESUME messages)

---

### 7. Security Considerations

**7.1 Ed25519 Peer Authentication**

**Per ADR-004 §4.3 and protocol-phase2-design.md:**

All P2P connections require Ed25519 signature verification:

```rust
// PeerHandshake challenge-response
fn verify_peer_handshake(msg: &PeerHandshake) -> Result<()> {
    // Step 1: Verify timestamp (prevent replay attacks)
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    if (now as i64 - msg.timestamp_ms as i64).abs() > 30_000 {
        return Err(Error::ChallengeExpired);
    }
    
    // Step 2: Verify Ed25519 signature
    let peer_pubkey = Ed25519PublicKey::from_hex(&msg.peer_id)?;
    let payload = format!(
        "MONOTERMINAL-P2P-HANDSHAKE:{}:{}:{}",
        msg.protocol_version,
        msg.peer_id,
        msg.timestamp_ms
    );
    
    peer_pubkey.verify(payload.as_bytes(), &msg.signature)?;
    
    Ok(())
}
```

**Why necessary:**
- WebRTC DTLS provides encryption but NOT peer identity verification
- STUN/TURN can leak IP addresses to malicious signaling servers
- Ed25519 signature binds protocol_version to cryptographic identity

---

**7.2 TURN Credential Scoping**

**Time-limited credentials (15-minute TTL):**

```rust
fn generate_turn_credentials(peer_id: &Ed25519PublicKey) -> TURNCredentials {
    let expiry = SystemTime::now() + Duration::from_secs(900); // 15 minutes
    let username = format!("{}:{}", expiry.as_secs(), peer_id.to_hex());
    let credential = hmac_sha256(
        &TURN_SECRET,
        username.as_bytes()
    );
    
    TURNCredentials {
        urls: vec!["turn:turn.monoterminal.io:3478".into()],
        username,
        credential: base64::encode(credential),
        expires_at_ms: expiry.as_millis(),
    }
}
```

**Security properties:**
- ✅ Single-use per WebRTC negotiation (nonce consumed after WebRTCOffer validation)
- ✅ Scoped to requesting peer (username contains peer_id)
- ✅ Time-limited (can't be reused after 15 minutes)
- ✅ HMAC secret known only to master + TURN server (shared via environment variable)

---

### 8. Phase 2 Implementation Roadmap

**Week 1-2: WebRTC Foundation**
- [ ] WebRTC DataChannel integration (webrtc-rs crate)
- [ ] PeerHandshake protocol (Ed25519 challenge-response)
- [ ] ICE candidate gathering (STUN client)
- [ ] Dual-transport management (WebSocket + DataChannel)
- [ ] Unit tests: version negotiation, signature verification

**Week 3-4: NAT Traversal**
- [ ] TURN server deployment (coturn on VPS)
- [ ] TURN credential generation (REST API, HMAC-SHA256)
- [ ] ICE negotiation timeout handling (10s STUN, 5s TURN)
- [ ] WebSocket fallback on WebRTC failure
- [ ] Integration tests: symmetric NAT, corporate VPN scenarios

**Week 5-6: Discovery Services**
- [ ] mDNS service advertisement (mdns-sd crate)
- [ ] Directory service API (Axum + SQLite)
- [ ] Ed25519 signature verification (directory registration)
- [ ] Discovery priority order (mDNS race vs directory)
- [ ] End-to-end test: client discovers master via mDNS

**Week 7-8: Optimization & Telemetry**
- [ ] zstd compression (OutputData >4KB)
- [ ] Backpressure handling (1MB write buffer)
- [ ] NAT traversal success rate telemetry
- [ ] Reconnection strategy (mobile backgrounding <10s)
- [ ] Performance benchmarks: latency p95, bandwidth reduction

**Week 9-10: Testing & Documentation**
- [ ] Stress test: 100 concurrent WebRTC connections
- [ ] Mobile testing: iOS Safari backgrounding, Android Chrome
- [ ] NAT traversal validation (home WiFi, cellular, VPN)
- [ ] Update SRS with measured NAT success rates
- [ ] Deployment guide: TURN server setup, directory service

**Estimated effort:** 3 months, 1.5 engineers (SRS §7.2)

---

## Alternatives Considered

### Option A: libp2p Instead of WebRTC

**Rejected per ADR-003:**
- ❌ No native browser support (requires js-libp2p, immature)
- ❌ NAT traversal ~70% ± 7% (no better than WebRTC)
- ❌ Custom directory service still required (no built-in DHT benefit)

**WebRTC wins:** Browser-native, proven at scale (Google Meet, Zoom)

---

### Option B: Full Mesh Topology (Client↔Client)

**Rejected (see §2):**
- ❌ O(N²) WebRTC negotiations (10 clients = 45 connections)
- ❌ Security complexity (every client must authenticate every other client)
- ❌ Master owns session state (clients can't own PTY/scrollback)

**Hub-and-spoke wins:** Simpler security, matches Phase 1 architecture

---

### Option C: Kademlia DHT for Discovery

**Deferred to Phase 4+:**
- ✅ Fully decentralized (no directory service needed)
- ❌ 2-10 second discovery latency (slower than directory <100ms)
- ❌ Complex implementation (libp2p-kad crate, peer routing)
- ❌ Overkill for Phase 2 MVP (directory service simpler)

**Decision:** Start with directory service, upgrade to DHT if needed

---

## Consequences

### Positive
- ✅ Instant fallback to WebSocket (no user-visible failures)
- ✅ Browser-native WebRTC (zero mobile SDK integration)
- ✅ Proven NAT traversal (STUN + TURN = 98-99% success)
- ✅ Hybrid discovery (mDNS for LAN, directory for internet)

### Negative
- ⚠️ Dual-transport memory overhead (~4KB per client)
- ⚠️ TURN server operational cost ($5-15/month VPS)
- ⚠️ Directory service is single point of failure (fallback: manual config)

### Neutral
- WebRTC complexity contained to Phase 2 (Phase 1 WebSocket-only remains simple)
- NAT success rates must be measured (can't assume literature figures per SRS §7.2)

---

## References

- **ADR-003:** WebRTC Over libp2p (WebRTC selection rationale)
- **ADR-004:** Protocol Schema Evolution (PeerHandshake, version negotiation)
- **SRS §2.3:** P2P Networking Architecture
- **SRS §7.2:** Phase 2 Acceptance Criteria (65-80% NAT, 100 sessions, <10s reconnect)
- **protocol-phase2-design.md:** WebRTC signaling flow, PeerHandshake schema
- **webrtc-rs:** https://github.com/webrtc-rs/webrtc
- **RFC 5766:** TURN - Relay Extensions to STUN
- **RFC 8825:** WebRTC Overview

---

## Follow-up Actions

1. ⏳ **Pending Phase 1 gate passage** (Friday 5/7 threshold)
2. ⏳ **Approve ADR-011** (eng-director, networking-engineer review)
3. ⏳ **Deploy TURN server** (devops-lead, VPS setup)
4. ⏳ **Implement WebRTC DataChannel** (networking-engineer, Week 1-2)
5. ⏳ **Build directory service** (rust-engineer-protocol, Week 5-6)

---

**Next:** ADR-012 (Persistence Layer Design)
