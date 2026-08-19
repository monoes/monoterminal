# ADR-003: WebRTC Over libp2p for P2P Transport

**Status:** Accepted  
**Date:** 2026-08-13  
**Deciders:** principal-architect, networking-engineer  
**SRS Reference:** §8.2.1  
**Phase:** Phase 2 (not Phase 1)

---

## Context

MONOTERMINAL Phase 2 requires P2P networking for:
- Direct client-to-master connections (bypass relay servers)
- NAT traversal (home networks, cellular, corporate VPNs)
- Mobile browser support (Android Chrome, iOS Safari)

We evaluated WebRTC vs. libp2p for the P2P transport layer.

---

## Decision

**Use WebRTC exclusively for P2P transport** (Phase 2+).

Phase 1 uses direct WebSocket (no P2P).

---

## Alternatives Considered

### Option A: rust-libp2p

**Pros:**
- Decentralized by design (Kademlia DHT)
- Proven in IPFS, Polkadot
- Active Rust ecosystem

**Cons:**
- No native mobile client to weigh (MONOTERMINAL ships no mobile binary per ADR-002)
- js-libp2p available but immature
- Mobile updates lag (rust-libp2p mobile support is not first-class)
- Real NAT traversal: ~70% ± 7% (4.4M-attempt study, no TCP-vs-QUIC difference)
- Requires custom directory service (no built-in discovery)

**Verdict:** ❌ Rejected

---

### Option B: WebRTC ✅

**Pros:**
- **Built into every browser** (Chrome, Firefox, Safari) via `RTCPeerConnection`
- Zero SDK integration work (no rust-webrtc crate on mobile)
- Proven at scale (Google Meet, Zoom, Discord)
- Cellular tested (works on 4G/5G carrier NAT)
- Google maintains WebRTC across all browsers (no SDK for us to track)

**Cons:**
- Less flexible than libp2p (no built-in DHT)
- Must build own directory service (or use STUN/TURN only)
- No rigorous NAT traversal percentage (measure directly in Phase 2)

**Verdict:** ✅ **CHOSEN**

---

## NAT Traversal Strategy

### STUN Servers (free, public)

- `stun:stun.l.google.com:19302` (primary)
- `stun1.l.google.com:19302` (backup)

### TURN Relay (self-hosted)

- **Software:** coturn (open source, C)
- **Cost:** $5-15/month VPS (100GB bandwidth)
- **Config:** listening-port=3478, tls-listening-port=5349

### Fallback Strategy

1. Attempt STUN direct connection (timeout: 10s)
2. If fails, use TURN relay
3. If TURN unavailable, fall back to HTTPS relay (master acts as WebSocket relay server)

### Expected Success Rates

| Network Type | STUN Direct | TURN Fallback |
|--------------|-------------|---------------|
| WiFi (home) | 85-95% | 98-99% |
| Cellular (4G/5G) | 60-75% | 98-99% |
| Corporate VPN | 40-55% | 98-99% |

**Note:** Real success rates must be measured in Phase 2 against actual MONOTERMINAL traffic (SRS §7.2 acceptance criteria).

---

## Discovery Strategy

**Hybrid Model:**

| Method | Scope | Latency | Reliability |
|--------|-------|---------|-------------|
| **mDNS/Bonjour** | LAN only | 1-5s | HIGH |
| **Directory Service** | Internet | <100ms | MEDIUM |
| **Kademlia DHT** | Internet | 2-10s | HIGH (future) |

**Flow:**

```
App Launch
    │
    ├─► mDNS Query (parallel)
    ├─► Directory Query (parallel)
    └─► DHT Query (parallel, future)
         │
         ▼
    Merge Results → Dedupe by Peer ID → Sort (mDNS first, Directory, DHT)
```

---

## Consequences

### Positive

- No mobile SDK integration (browser-native)
- Proven at scale (Google Meet, Zoom)
- Simpler integration vs. libp2p
- Smaller binary (no rust-libp2p crate)

### Negative

- Must build own directory service (no built-in DHT)
- Less flexible than libp2p (no custom transports)
- STUN/TURN infrastructure required (coturn self-hosted)

### Neutral

- NAT traversal success rate: measure directly in Phase 2, don't assume literature figures

---

## Implementation Notes

### Rust Master

- Use `webrtc-rs` crate (Rust implementation of WebRTC)
- `RTCPeerConnection` for DataChannel
- DTLS-SRTP for encryption (automatic)

### Web Client

- Use native `RTCPeerConnection` browser API
- js-libp2p for signaling only (optional)
- No SDK required

### Signaling

- WebSocket signaling server (master or separate service)
- SDP Offer/Answer exchange
- ICE candidate trickle

---

## References

- SRS §8.2.1 (WebRTC over libp2p)
- SRS §2.3 (P2P Networking Architecture)
- libp2p NAT traversal study: 4.4M attempts, ~70% ± 7% success
- WebRTC spec: RFC 8825 (ICE), RFC 8445 (STUN), RFC 5766 (TURN)

---

## Follow-up Actions

1. ⏳ Phase 1: Skip (direct WebSocket only)
2. ⏳ Phase 2: Implement WebRTC DataChannel (rust-webrtc)
3. ⏳ Phase 2: Build directory service for peer discovery
4. ⏳ Phase 2: Self-host coturn TURN relay
5. ⏳ Phase 2: Measure real NAT traversal success rates
