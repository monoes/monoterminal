# Phase 2 Implementation Roadmap

**Version:** 1.0  
**Date:** 2026-08-17  
**Status:** Ready for Implementation (Pending Phase 1 Gate Passage)  
**Author:** principal-architect

---

## Executive Summary

This document consolidates the Phase 2 architectural design across four foundational ADRs:
- **ADR-011:** P2P Networking Architecture
- **ADR-012:** Persistence Layer Design
- **ADR-013:** Multi-Session Architecture
- **ADR-014:** Collaboration Primitives

**Phase 2 Goal (SRS §7.2):** Turn the Windows+Web pair into a real product — P2P, persistence, multi-session, collaboration.

**Effort:** 3 months, 1.5 engineers (12 weeks total)

**Acceptance Criteria:**
- ✅ 100 concurrent sessions tested
- ✅ 65-80% NAT traversal success (measured, not assumed)
- ✅ Reconnect-after-background <10s on iOS Safari
- ✅ 75% test coverage

---

## Architecture Overview

### System Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    MONOTERMINAL Phase 2                         │
│                                                                 │
│  ┌──────────────┐                           ┌──────────────┐   │
│  │  Web Client  │◄──────WebSocket──────────►│    Master    │   │
│  │  (Browser)   │                           │    Daemon    │   │
│  │              │◄────WebRTC DataChannel───►│   (Rust)     │   │
│  └──────────────┘                           └──────┬───────┘   │
│         │                                           │           │
│         │                                    ┌──────▼───────┐   │
│         │                                    │   SQLite     │   │
│         │                                    │  Persistence │   │
│         │                                    └──────────────┘   │
│         │                                                       │
│  ┌──────▼──────────────────────────────────────────────┐       │
│  │           Discovery Services                        │       │
│  │  • mDNS (LAN): _monoterminal._tcp.local            │       │
│  │  • Directory Service: HTTP API (peer registration) │       │
│  └─────────────────────────────────────────────────────┘       │
│                                                                 │
│  ┌─────────────────────────────────────────────────────┐       │
│  │           NAT Traversal Infrastructure              │       │
│  │  • STUN: stun.l.google.com:19302 (public)         │       │
│  │  • TURN: coturn self-hosted (VPS, $5-15/month)     │       │
│  └─────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Week-by-Week Implementation Plan

### Weeks 1-2: WebRTC Foundation (ADR-011)

**Deliverables:**
- [ ] WebRTC DataChannel integration (webrtc-rs crate)
- [ ] PeerHandshake protocol (Ed25519 challenge-response, per ADR-004 §4.3)
- [ ] ICE candidate gathering (STUN client)
- [ ] Dual-transport management (WebSocket + DataChannel active concurrently)
- [ ] Unit tests: version negotiation, signature verification

**Owner:** networking-engineer  
**Dependencies:** None (starts immediately after Phase 1 gate)

**Key Decisions:**
- Hub-and-spoke topology (client-to-master, not mesh)
- Both transports active (instant fallback, ~4KB overhead per client)
- 15-second WebRTC timeout (10s STUN, 5s TURN)

---

### Weeks 3-4: NAT Traversal & Fallback (ADR-011)

**Deliverables:**
- [ ] TURN server deployment (coturn on DigitalOcean/AWS VPS)
- [ ] TURN credential generation (REST API, HMAC-SHA256, 15-minute TTL)
- [ ] ICE negotiation timeout handling
- [ ] WebSocket fallback on WebRTC failure
- [ ] Integration tests: symmetric NAT, corporate VPN scenarios

**Owner:** networking-engineer + devops-lead  
**Dependencies:** Week 1-2 WebRTC foundation

**Key Decisions:**
- 3-tier strategy: STUN (0-10s) → TURN (10-15s) → WebSocket (always available)
- Expected success: 98-99% total (65-80% STUN direct, rest via TURN/fallback)
- Measure real NAT traversal rates (don't assume literature figures)

---

### Weeks 5-6: Discovery Services (ADR-011)

**Deliverables:**
- [ ] mDNS service advertisement (mdns-sd crate)
- [ ] Directory service API (Axum + SQLite, 3 endpoints: register, lookup, deregister)
- [ ] Ed25519 signature verification (directory registration)
- [ ] Discovery priority order (mDNS race vs directory)
- [ ] End-to-end test: client discovers master via mDNS

**Owner:** rust-engineer-protocol  
**Dependencies:** Weeks 1-4 (can start in parallel after Week 2)

**Key Decisions:**
- Hybrid model: mDNS (LAN, 1-5s) + Directory (internet, <100ms)
- Parallel race: whichever responds first wins
- Fallback: manual configuration (environment variable)

---

### Weeks 2-3: Persistence Layer (ADR-012)

**Deliverables:**
- [ ] SQLite schema (sessions, scrollback, configuration, audit_logs tables)
- [ ] Schema migration system (version tracking, idempotent scripts)
- [ ] Scrollback two-tier storage (hot: 10k lines RAM, cold: disk + zstd)
- [ ] Connection pooling (r2d2, max 20 connections)
- [ ] Write batching (100 lines/batch, 100ms flush interval)

**Owner:** rust-engineer-storage  
**Dependencies:** None (can start in parallel with WebRTC)

**Key Decisions:**
- WAL mode (readers don't block writers)
- zstd compression: 60-80% reduction (4KB → 800B-1200B)
- Dual-mode paths: Service (%ProgramData%), Console (%LOCALAPPDATA%)

---

### Weeks 4-5: Multi-Session Architecture (ADR-013)

**Deliverables:**
- [ ] SessionManager with state machine (CREATE → RUNNING → DETACHED → TERMINATED)
- [ ] Session routing table (session_id → Session handle)
- [ ] Client multiplexing (one WebSocket → many sessions)
- [ ] Session discovery (ListSessionsRequest API)
- [ ] DETACHED TTL cleanup (24-hour default, configurable)

**Owner:** rust-backend-lead  
**Dependencies:** Week 2-3 persistence layer

**Key Decisions:**
- DETACHED state: sessions persist when all clients leave (24h TTL)
- Resource quotas: max 100 sessions, max 10/user
- Scrollback hot/cold tiering: keep 10k lines in RAM per session

---

### Weeks 6-7: Collaboration Primitives (ADR-014)

**Deliverables:**
- [ ] Multi-client attach (N clients on one PTY)
- [ ] Presence tracking (ClientPresence, heartbeat 30s, eviction 2min)
- [ ] RBAC implementation (owner/editor/viewer roles, ACL HashMap)
- [ ] Input queueing (FIFO, sequential execution)
- [ ] JWT integration (extract user_id from "sub" claim)

**Owner:** rust-backend-lead + security-engineer  
**Dependencies:** Week 4-5 multi-session architecture

**Key Decisions:**
- Broadcast model (all clients see same output, not independent cursors)
- Heartbeat: 30s interval, 2-minute timeout
- Input conflict resolution: Queue-only (no locking in Phase 2)

---

### Weeks 7-8: Optimization & Telemetry (ADR-011, ADR-012)

**Deliverables:**
- [ ] zstd compression (OutputData >4KB)
- [ ] Backpressure handling (1MB write buffer, drop oldest)
- [ ] NAT traversal success rate telemetry
- [ ] Reconnection strategy (mobile backgrounding <10s)
- [ ] Performance benchmarks: latency p95, bandwidth reduction

**Owner:** performance-engineer  
**Dependencies:** Weeks 1-7 (integration complete)

**Targets:**
- Fetch 1000 scrollback lines: <100ms p95
- Write 100 lines: <50ms p95 (batched)
- DB size: <1GB for 100 sessions × 100k lines

---

### Weeks 9-10: Testing & Documentation (All ADRs)

**Deliverables:**
- [ ] Stress test: 100 concurrent WebRTC connections
- [ ] Mobile testing: iOS Safari backgrounding, Android Chrome
- [ ] NAT traversal validation (home WiFi, cellular, VPN)
- [ ] RBAC security audit (permission enforcement)
- [ ] Update SRS with measured NAT success rates
- [ ] Deployment guide: TURN server setup, directory service

**Owner:** qa-lead + test-engineer-e2e  
**Dependencies:** Weeks 1-8 (all features complete)

**Acceptance tests:**
- 100 concurrent sessions: RAM <500 MB, CPU <50%
- NAT traversal: 65-80% measured success
- iOS Safari reconnect: <10s after backgrounding

---

## Critical Path Analysis

**Critical path (longest dependency chain):**

```
Week 1-2: WebRTC Foundation
    ↓
Week 3-4: NAT Traversal
    ↓
Week 7-8: Optimization & Telemetry
    ↓
Week 9-10: Testing & Documentation
```

**Duration:** 10 weeks on critical path

**Parallelizable work:**
- Persistence (Weeks 2-3) runs parallel to WebRTC (Weeks 1-2)
- Discovery Services (Weeks 5-6) runs parallel to Multi-Session (Weeks 4-5)
- Collaboration (Weeks 6-7) runs parallel to Optimization (Weeks 7-8)

**Total calendar time:** 10-12 weeks (2.5-3 months, aligns with SRS §7.2 estimate)

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **NAT traversal <65%** | Medium | High | TURN relay fallback ensures 98%+ total success |
| **SQLite performance bottleneck** | Low | Medium | WAL mode + write batching tested to 100 sessions |
| **iOS Safari backgrounding >10s** | Medium | Medium | WebSocket-only mode acceptable fallback |
| **TURN server cost overrun** | Low | Low | $5-15/month VPS sufficient for MVP, scalable later |
| **RBAC implementation complexity** | Low | Medium | Simple ACL HashMap, defer fine-grained permissions to Phase 3 |
| **WebRTC browser compatibility** | Low | High | Chrome/Firefox/Safari all support WebRTC (proven at scale) |

---

## Technology Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| **P2P Transport** | WebRTC (webrtc-rs) | Browser-native, proven NAT traversal |
| **NAT Traversal** | STUN (Google) + TURN (coturn) | Free STUN, self-hosted TURN ($5-15/mo) |
| **Discovery** | mDNS (mdns-sd) + HTTP API (Axum) | LAN + internet coverage |
| **Persistence** | SQLite (rusqlite) | Embedded, WAL mode, 100+ sessions proven |
| **Compression** | zstd (level 1) | 60-80% reduction, <5ms latency |
| **Connection Pool** | r2d2-sqlite | Thread-safe, max 20 connections |
| **Auth** | JWT (jsonwebtoken) + Ed25519 | Existing Phase 1 infrastructure |

---

## Integration Points with Phase 1

**Preserved:**
- WebSocket baseline (Phase 1 clients continue working)
- Ed25519 + JWT auth (ADR-007, ADR-008)
- Protocol schema (Envelope, fields 1-17)
- Daemon lifecycle (ADR-005: dual-mode Service/Console)

**Extended:**
- Protocol schema: fields 18-30 (Phase 2 messages, per ADR-004 §2.5)
- Session lifecycle: ADD DETACHED state (ADR-013)
- Daemon paths: ADD SQLite database (`data/monoterminal.db`)

**No breaking changes:** Phase 1 clients and Phase 2 master are fully compatible (version negotiation via `protocol_version` field).

---

## Phase 2 → Phase 3 Transition

**Deferred to Phase 3:**
- Linux + macOS master support (SRS §7.3)
- Input locking (explicit "request lock" mechanism)
- Directory service distributed upgrade (Consul/etcd)
- Cross-session bandwidth quotas (per-user limits)

**Upgrade path:**
- Phase 2 SQLite schema supports Phase 3 additive changes (ALTER TABLE ADD COLUMN)
- Protocol fields 31-39 reserved for Phase 3 extensions
- No breaking changes required (backward compatible)

---

## Success Metrics

**Technical metrics (SRS §7.2 acceptance):**
- ✅ 100 concurrent sessions tested
- ✅ 65-80% NAT traversal (measured in production)
- ✅ <10s reconnect after mobile backgrounding
- ✅ 75% test coverage

**User metrics (Phase 2+ evaluation):**
- Session persistence adoption: >50% of users create multi-session workflows
- Collaboration adoption: >20% of sessions have multi-client attach
- P2P success rate: >70% direct connections (rest via TURN/WebSocket)

**Infrastructure metrics:**
- TURN server bandwidth: <100 GB/month (within $5-15 VPS budget)
- Directory service uptime: >99.5%
- SQLite database size: <1 GB per 100 sessions

---

## References

**ADRs:**
- ADR-003: WebRTC Over libp2p (WebRTC selection)
- ADR-004: Protocol Schema Evolution (fields 18-30, version negotiation)
- ADR-005: Daemon Lifecycle (dual-mode paths)
- ADR-007/008: JWT + Ed25519 Auth
- ADR-011: P2P Networking Architecture
- ADR-012: Persistence Layer Design
- ADR-013: Multi-Session Architecture
- ADR-014: Collaboration Primitives

**SRS:**
- §2.1.3: Session Lifecycle State Machine
- §2.1.5: RBAC Roles (owner, editor, viewer)
- §4.1: SQLite Persistence
- §7.2: Phase 2 Acceptance Criteria

**Design Documents:**
- protocol-phase2-design.md (rust-engineer-protocol, 2026-08-15)
- webrtc-signaling-flow.md
- nat-traversal-strategy.md

---

## Appendix: Phase 2 Protocol Message Summary

**New message types (fields 18-30, per ADR-004 §2.5):**

| Field | Message Type | Purpose |
|-------|-------------|---------|
| 18 | ScrollbackFetchRequest | Paginate old scrollback (>100 lines) |
| 19 | ScrollbackFetchResponse | Return scrollback page (zstd compressed) |
| 20 | ListSessionsRequest | Discover active sessions |
| 21 | ListSessionsResponse | Return session summaries |
| 22 | ClientHeartbeat | Keep-alive (30s interval) |
| 23 | PresenceUpdate | Broadcast client join/leave/focus |
| 24 | InputFocusUpdate | Client active/idle state change |
| 25 | WebRTCOffer | SDP offer (WebRTC negotiation) |
| 26 | WebRTCAnswer | SDP answer + TURN credentials |
| 27 | ICECandidate | Trickle ICE candidate |
| 28 | P2PConnectionStatus | WebRTC state (negotiating/connected/failed) |
| 29 | PeerHandshake | Ed25519 challenge-response (P2P auth) |
| 30 | PeerHandshakeResponse | Challenge + nonce |

**Total:** 13 new message types (17 Phase 1 + 13 Phase 2 = 30 total)

---

**Status:** Ready for Implementation (Pending Phase 1 Gate Passage Friday 1 PM)

**Next:** Await eng-director gate decision (5/7 threshold)
