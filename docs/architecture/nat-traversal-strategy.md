# NAT Traversal Strategy - Phase 2 P2P Networking

**Author:** networking-engineer  
**Date:** 2026-08-15  
**Status:** Ready for Monday AM Review  
**References:** ADR-003, SRS §7.2, docs/research/phase2-p2p-webrtc-research.md §4

---

## Executive Summary

This document defines MONOTERMINAL's NAT traversal strategy for Phase 2 P2P WebRTC networking, addressing the challenge that both master daemon and web clients may operate behind NAT/firewalls.

**Key Decisions:**
- ✅ **Full ICE** (not ice-lite) - Master may be behind NAT
- ✅ **Trickle ICE** - 2-3s faster connection vs. batch ICE
- ✅ **Three-tier fallback** - STUN → TURN → HTTPS relay (100% success guarantee)
- ✅ **15s total timeout** - 10s STUN + 5s TURN before WebSocket fallback

**Target Success Rate:** 65-80% NAT traversal (SRS §7.2 acceptance criteria) - measured directly against MONOTERMINAL traffic in Phase 2.

---

## 1. ice-lite vs Full ICE Decision

### 1.1 What is ice-lite?

**ice-lite** (RFC 5245 §2.3):
- **Server-only optimization:** Server does NOT gather ICE candidates
- Server only responds to client's candidate checks
- **Use case:** Server with public IP, no NAT (e.g., Google Meet servers on GCP)
- **Benefit:** Reduces server complexity, faster connection setup

**Full ICE:**
- Both client AND server gather ICE candidates (host, srflx, relay)
- Full connectivity check matrix (client candidates × server candidates)
- Works when BOTH sides are behind NAT

### 1.2 MONOTERMINAL's Deployment Reality

**Master Daemon Deployment Scenarios:**

| Scenario | Master Location | Master NAT Status | ice-lite OK? |
|----------|----------------|-------------------|--------------|
| **Home user** | Windows desktop behind WiFi router | ✅ Behind NAT | ❌ NO |
| **Small office** | Workstation behind corporate firewall | ✅ Behind NAT | ❌ NO |
| **Cloud deployment** | AWS/GCP instance with public IP | ❌ No NAT | ✅ YES |
| **Developer laptop** | MacBook on coffee shop WiFi | ✅ Behind NAT | ❌ NO |

**Analysis:**
- **Primary use case (Phase 1-2):** Home/small-office users - Master behind NAT
- **Minority use case (Phase 3+):** Cloud deployment - Master with public IP

### 1.3 Decision: Full ICE Required

**Rationale:**
1. **Master behind NAT is common:** Home WiFi router, corporate firewall
2. **ice-lite fails for NATted master:** Cannot traverse NAT from master side
3. **Full ICE handles all scenarios:** Works whether master is NATted or not
4. **Performance cost acceptable:** <1s additional ICE gathering time
5. **Rust-webrtc supports Full ICE natively:** No custom implementation needed

**Example Failure Case (ice-lite):**
```
Client (behind NAT A) ← wants to connect → Master (behind NAT B, ice-lite)

ice-lite Master:
- Does NOT gather srflx candidate (no STUN query)
- Only responds to client's candidate checks
- Cannot provide relay path if client check fails

Result: Connection fails if client's NAT is symmetric (10-20% of cases)

Full ICE Master:
- Gathers srflx candidate via STUN
- Gathers relay candidate via TURN
- Provides multiple paths for client to test

Result: Connection succeeds via TURN relay (98-99% success)
```

**Conclusion:** Full ICE is required for MONOTERMINAL's deployment model.

---

## 2. Trickle ICE vs Batch ICE

### 2.1 Comparison

| Mode | Description | Connection Time | Complexity |
|------|-------------|-----------------|------------|
| **Batch ICE** | Wait for all candidates, send in one message | 5-8 seconds | Low (simple) |
| **Trickle ICE** | Send candidates as discovered (incremental) | 2-3 seconds | Medium (state machine) |

**Why Trickle ICE is Faster:**
```
Batch ICE Timeline:
0s: Start ICE gathering
2s: host candidates ready (immediate)
3s: srflx candidates ready (STUN query)
5s: relay candidates ready (TURN allocation)
5s: Send all candidates to peer → connectivity checks begin
8s: Connection established

Trickle ICE Timeline:
0s: Start ICE gathering
0.1s: host candidates ready → send immediately → checks begin
3s: srflx candidates ready → send immediately
3.5s: Connection established via srflx (relay not needed)
```

**Savings:** 2-5 seconds (critical for UX, especially mobile)

### 2.2 Decision: Use Trickle ICE

**Rationale:**
- ✅ **Speed:** 2-3s connection vs 5-8s (40% faster)
- ✅ **Mobile-critical:** iOS Safari has 30s background timeout (every second counts)
- ✅ **Standard:** All browsers support trickle ICE (Chrome, Firefox, Safari)
- ✅ **rust-webrtc support:** `on_ice_candidate` callback fires per candidate
- ⚠️ **Complexity:** Slightly more complex state management (acceptable trade-off)

**Implementation:**
```rust
// Master (rust-webrtc)
pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
    let signaling = signaling.clone();
    Box::pin(async move {
        if let Some(c) = candidate {
            // Send candidate immediately (trickle)
            signaling.send_ice_candidate(c).await?;
        }
        Ok(())
    })
}));
```

```typescript
// Web client
pc.addEventListener('icecandidate', (event) => {
  if (event.candidate) {
    // Send candidate immediately (trickle)
    sendMessage({ ice_candidate: {
      candidate: event.candidate.candidate,
      sdpMid: event.candidate.sdpMid,
      sdpMLineIndex: event.candidate.sdpMLineIndex
    }});
  }
});
```

---

## 3. Three-Tier Fallback Chain

### 3.1 Architecture

```
┌──────────────────────────────────────────┐
│ Tier 1: STUN Direct (10s timeout)       │  Priority: 1
│   - Public STUN (stun.l.google.com)     │  Success: 60-95%
│   - UDP hole-punching                    │  Cost: Free
│   - Works: Full Cone, Restricted NAT    │
└──────────────────────────────────────────┘
              │ FAILS
              ▼
┌──────────────────────────────────────────┐
│ Tier 2: TURN Relay (5s timeout)         │  Priority: 2
│   - Self-hosted coturn                   │  Success: 98-99%
│   - Relayed UDP path                     │  Cost: $5-15/mo VPS
│   - Works: Symmetric NAT, blocked UDP   │
└──────────────────────────────────────────┘
              │ FAILS (coturn unavailable)
              ▼
┌──────────────────────────────────────────┐
│ Tier 3: HTTPS Relay (no timeout)        │  Priority: 3
│   - Existing WebSocket connection        │  Success: 100%
│   - Phase 1 fallback                     │  Cost: Free
│   - Works: Always (TLS 1.3)              │
└──────────────────────────────────────────┘
```

### 3.2 Tier 1: STUN Direct Connection

**Public STUN Servers:**
```rust
RTCIceServer {
    urls: vec![
        "stun:stun.l.google.com:19302".to_string(),
        "stun:stun1.l.google.com:19302".to_string(), // Backup
    ],
    ..Default::default()
}
```

**How STUN Works:**
1. Client sends STUN Binding Request to public STUN server
2. STUN server responds with client's public IP:port (srflx candidate)
3. Client sends this srflx candidate to master via signaling
4. Master also gathers its own srflx candidate via STUN
5. Both sides attempt direct P2P connection using srflx candidates
6. If NATs allow (symmetric NAT hole-punching), direct UDP connection succeeds

**Success Rates (Literature - to be measured directly):**

| NAT Type | STUN Success | Typical Network |
|----------|--------------|-----------------|
| **Full Cone NAT** | 95% | Home router (port-forwarding enabled) |
| **Restricted Cone NAT** | 85% | Home router (default config) |
| **Port-Restricted Cone** | 70% | Small office router |
| **Symmetric NAT** | 10-20% | Carrier-grade NAT (CGN), corporate VPN |

**Timeout:** 10 seconds
- Long enough for ICE gathering on slow networks (cellular 3G)
- Short enough to avoid poor UX if STUN fails

### 3.3 Tier 2: TURN Relay

**Self-Hosted coturn Configuration:**
```yaml
Server: turn.monoterminal.dev
Ports: 3478 (UDP/TCP), 5349 (TLS)
VPS: DigitalOcean Droplet $6/month (1 vCPU, 1GB RAM, 1TB bandwidth)
```

**How TURN Works:**
1. Client sends TURN Allocate Request to coturn server (includes HMAC credentials)
2. coturn validates credentials (timestamp + HMAC)
3. coturn allocates relay address (e.g., `192.168.1.100:54321`)
4. Client uses relay address as ICE relay candidate
5. Master connects to client via coturn relay (UDP forwarding)
6. All P2P traffic flows through coturn (encrypted by DTLS)

**When TURN Activates:**
- STUN direct connection fails (symmetric NAT, blocked UDP)
- ICE selects relay candidate as best path
- Automatic fallback (no user action needed)

**Success Rate:** 98-99%
- Only fails if coturn server unreachable (server down, network blocking TURN ports)

**Cost:** ~$0.05/GB bandwidth
- Example: 10 users × 8 hours × 50 KB/s = ~14 GB/day = ~420 GB/month = ~$21/month
- Acceptable for Phase 2 MVP (optimize in Phase 3 with regional TURN pools)

**Timeout:** 5 seconds
- TURN should be faster than STUN (dedicated relay, no NAT hole-punching attempts)
- If TURN fails in 5s, likely server unavailable → fall back to HTTPS

### 3.4 Tier 3: HTTPS Relay (WebSocket Fallback)

**Guaranteed Fallback:**
- Phase 1 WebSocket connection ALREADY established (TLS 1.3)
- If WebRTC negotiation fails completely (15s timeout total), client continues using WebSocket
- No user-visible interruption (terminal keeps working)

**Telemetry:**
- Client logs `P2P_STATE_FAILED`
- Master marks connection as "WebSocket fallback"
- Monitored for Phase 2 acceptance (goal: <5% fallback rate)

**Why This Matters:**
- 100% uptime guarantee (always works, even if STUN + TURN both fail)
- User never sees connection error
- Phase 2 P2P is additive enhancement, not breaking change

---

## 4. Timeout Strategy

### 4.1 Timeout Breakdown

| Phase | Timeout | Rationale |
|-------|---------|-----------|
| **ICE Gathering** | 10s | Wait for all candidate types (host, srflx, relay) |
| **STUN Direct** | 10s | ICE connectivity checks (candidate pair testing) |
| **TURN Relay** | 5s | Faster than STUN (dedicated relay, no hole-punching) |
| **Total ICE** | 15s | Sum of STUN + TURN attempts before WebSocket fallback |
| **DataChannel Idle** | 60s | No activity → send ping to keep connection alive |

### 4.2 ICE State Transitions

```
ICEGatheringState:
  new → gathering (0-2s) → complete

ICEConnectionState:
  new → checking (0-10s STUN) → connected
                           ↓
                      failed (10s timeout) → checking (0-5s TURN) → connected
                                                                 ↓
                                                            failed (5s timeout) → closed
                                                                 ↓
                                                      [WebSocket fallback]
```

### 4.3 Mobile Considerations

**iOS Safari Background Timeout:**
- Safari suspends network after ~30s in background
- WebRTC connection also suspended
- On foreground resume: Detect DataChannel closed → reconnect

**Reconnection Target:** <10s p95 (SRS §7.2 acceptance criteria)

**Strategy:**
1. Detect app returned to foreground (`visibilitychange` event)
2. Check DataChannel state → if closed, initiate new WebRTC negotiation
3. Reuse existing PeerHandshake (if nonce valid <60s) OR perform new handshake
4. Fast-path: Try STUN first (most likely to succeed on mobile)
5. Measure: foreground → DataChannel open latency

---

## 5. NAT Type Detection

### 5.1 RFC 3489 NAT Classification

| NAT Type | Behavior | STUN Success | Example Network |
|----------|----------|--------------|-----------------|
| **Full Cone** | Same public IP:port for all destinations | 95% | Port-forwarded home router |
| **Restricted Cone** | Same public port, different IP → filtered | 85% | Default home router |
| **Port-Restricted Cone** | Same public port, different IP:port → filtered | 70% | Small office firewall |
| **Symmetric** | Different public port per destination | 10-20% | CGN, corporate VPN, cellular |

### 5.2 Detection Algorithm

**Simplified NAT Detection (via STUN):**
1. Query STUN server A → get public IP:port pair A
2. Query STUN server B → get public IP:port pair B
3. Compare:
   - If `A == B` → Full Cone or Restricted Cone NAT
   - If `A != B` (different port) → Symmetric NAT

**Implementation:**
```rust
async fn detect_nat_type(stun_servers: &[String]) -> Result<NATType, Error> {
    let mut candidates = vec![];
    
    for server in stun_servers {
        let candidate = query_stun(server).await?;
        candidates.push(candidate);
    }
    
    // Compare srflx candidates
    if candidates[0].port == candidates[1].port {
        Ok(NATType::ConeNAT) // Full/Restricted/Port-Restricted (need more tests to distinguish)
    } else {
        Ok(NATType::SymmetricNAT)
    }
}
```

### 5.3 Telemetry Usage

**Log NAT type for each connection:**
- Track correlation: NAT type → STUN success/failure
- Validate literature success rates against real MONOTERMINAL traffic
- Identify networks needing TURN optimization

**Example Telemetry:**
```json
{
  "nat_type": "SymmetricNAT",
  "ice_state": "failed",
  "fallback": "TURN",
  "connection_time_ms": 8500,
  "network": "Cellular_Verizon"
}
```

---

## 6. Browser Compatibility

### 6.1 WebRTC Support Matrix

| Browser | WebRTC Support | Trickle ICE | DTLS 1.2+ | Notes |
|---------|---------------|-------------|-----------|-------|
| **Chrome 100+** | ✅ Full | ✅ Yes | ✅ Yes | Reference implementation |
| **Firefox 100+** | ✅ Full | ✅ Yes | ✅ Yes | Strong support |
| **Safari 15+** | ✅ Full | ✅ Yes | ✅ Yes | iOS Safari tested |
| **Edge 100+** | ✅ Full | ✅ Yes | ✅ Yes | Chromium-based |

**Phase 2 Target:** Chrome, Firefox, Safari 15+

### 6.2 Known Browser Issues

**Safari (iOS/macOS):**
- Background suspension (30s timeout) → Handled by reconnection logic
- Stricter privacy (mDNS candidate obfuscation) → No impact (STUN/TURN still work)

**Firefox:**
- Requires `iceServers` config (no default STUN) → Explicitly configured

**Chrome:**
- No known issues

---

## 7. Testing Strategy

### 7.1 NAT Simulation (Unit Tests)

**Docker-based NAT Simulation:**
```bash
# Simulate symmetric NAT
docker run --sysctl net.ipv4.ip_forward=1 \
           --privileged \
           monoterminal/nat-sim:symmetric

# Run test
cargo test test_symmetric_nat_traversal
```

### 7.2 Real-World Testing (Phase 2 Acceptance)

**Test Matrix:**

| Network Type | Client NAT | Master NAT | Expected Result |
|--------------|------------|------------|-----------------|
| Home WiFi | Restricted Cone | Restricted Cone | ✅ STUN direct (85%+) |
| Cellular 4G | Symmetric | Restricted Cone | ✅ TURN relay (98%+) |
| Corporate VPN | Symmetric | Symmetric | ✅ TURN relay (98%+) |
| Coffee shop WiFi | Port-Restricted | Restricted Cone | ✅ STUN direct (70%+) |
| Tethered mobile | Symmetric | Symmetric | ✅ TURN relay (98%+) |

**Measurement:**
- Run 100+ connections per network type
- Measure: STUN success %, TURN usage %, WebSocket fallback %
- Goal: 65-80% overall NAT traversal success (SRS §7.2)

### 7.3 Failure Injection Tests

**Test coturn Unavailability:**
1. Stop coturn service
2. Attempt WebRTC connection
3. Verify: Client falls back to WebSocket within 15s
4. Verify: Terminal continues working (no user-visible error)

**Test STUN Blocked:**
1. Firewall block UDP 3478 (STUN port)
2. Attempt WebRTC connection
3. Verify: Client falls back to TURN
4. Measure: Connection time (should be <8s)

---

## 8. Performance Targets

### 8.1 Latency Budget (SRS §2.2)

| Operation | Target | Measurement |
|-----------|--------|-------------|
| ICE gathering (all candidates) | <2s | p95 |
| STUN direct connection | <3s | p95 from offer → DataChannel open |
| TURN relay connection | <8s | p95 from offer → DataChannel open |
| WebSocket fallback | <15s | p95 from offer → fallback detected |
| Mobile reconnection | <10s | p95 foreground → DataChannel open |

### 8.2 Success Rate Targets

| Metric | Phase 2 Target | Phase 3 Target |
|--------|---------------|----------------|
| Overall NAT traversal | 65-80% | 85-95% |
| STUN direct success | 60-75% | 75-85% |
| TURN fallback usage | 20-35% | 10-20% |
| WebSocket fallback | <5% | <2% |

---

## 9. Implementation Checklist

### 9.1 Master Daemon (Rust)

- [ ] Configure Full ICE (not ice-lite)
- [ ] Enable trickle ICE (`on_ice_candidate` callback)
- [ ] Add STUN servers: `stun.l.google.com:19302`, `stun1.l.google.com:19302`
- [ ] Add TURN server: `turn(s)://turn.monoterminal.dev:3478/5349`
- [ ] Implement 15s total ICE timeout
- [ ] Implement NAT type detection (log for telemetry)
- [ ] Handle ICE state transitions: `new → checking → connected → failed → WebSocket fallback`

### 9.2 Web Client (TypeScript)

- [ ] Enable trickle ICE (`addEventListener('icecandidate')`)
- [ ] Configure ICE servers (STUN + TURN from WebRTCAnswer)
- [ ] Implement 15s ICE timeout → WebSocket fallback
- [ ] Log NAT type detection results
- [ ] Implement mobile reconnection (iOS Safari foreground resume)
- [ ] Measure connection time telemetry (offer → DataChannel open)

### 9.3 coturn TURN Relay

- [ ] Deploy coturn on DigitalOcean VPS ($6/month)
- [ ] Configure ports: 3478 (UDP/TCP), 5349 (TLS)
- [ ] Install Let's Encrypt TLS certificate
- [ ] Set bandwidth quotas: `max-bps=1000000` (1 Mbps/allocation)
- [ ] Enable verbose logging
- [ ] Configure firewall (ports 3478, 5349, 49152-65535)

### 9.4 Monitoring & Telemetry

- [ ] Log ICE state transitions per connection
- [ ] Log NAT type detection per connection
- [ ] Track STUN success rate (daily aggregate)
- [ ] Track TURN usage rate (daily aggregate)
- [ ] Track WebSocket fallback rate (daily aggregate)
- [ ] Alert: WebSocket fallback >10% (indicates TURN issues)
- [ ] Alert: coturn offline (health check every 60s)

---

## 10. Phase 2 Acceptance Criteria

**From SRS §7.2:**
> **Acceptance Criterion:** 65-80% NAT traversal success rate, measured directly against MONOTERMINAL traffic (not literature estimates).

**Validation Plan:**
1. Deploy Phase 2 to test users (10+ diverse networks)
2. Collect 1000+ connection attempts over 2 weeks
3. Measure:
   - STUN direct success %
   - TURN fallback usage %
   - WebSocket fallback %
   - Connection time distribution (p50, p95, p99)
4. Pass criteria: 65-80% STUN+TURN success, <5% WebSocket fallback

---

## 11. Future Optimizations (Phase 3+)

**Not Required for Phase 2:**

1. **Regional TURN Pools:**
   - Deploy coturn in US-East, US-West, EU, Asia
   - Client selects nearest TURN server (latency optimization)
   - Cost: 4× VPS cost (~$24/month)

2. **TURN Load Balancing:**
   - Round-robin across multiple coturn instances
   - Handle coturn failure gracefully (try next server)

3. **ICE Candidate Prioritization:**
   - Prefer relay candidates on known-symmetric networks (e.g., Verizon cellular)
   - Skip STUN attempt if NAT type detected as symmetric → save 10s

4. **Adaptive Timeouts:**
   - Cellular network: 15s STUN timeout (slower)
   - WiFi network: 5s STUN timeout (faster)

---

## 12. References

- RFC 5245: Interactive Connectivity Establishment (ICE)
- RFC 3489: STUN (Classic)
- RFC 5389: STUN (Updated)
- RFC 5766: TURN
- ADR-003: WebRTC over libp2p (architecture decision)
- SRS §7.2: Phase 2 Acceptance Criteria (NAT traversal)

---

## 13. Approval

**Document Status:** Ready for Monday AM Review

**Review Required:**
- [ ] principal-architect: Approve NAT traversal strategy
- [ ] rust-backend-lead: Validate Full ICE implementation plan
- [ ] devops-lead: Confirm coturn deployment plan

**Target Approval:** Monday 2026-08-17 noon (before security doc finalization)

---

**END OF DOCUMENT**
