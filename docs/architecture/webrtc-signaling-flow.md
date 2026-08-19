# WebRTC Signaling Flow - Phase 2 P2P Networking

**Author:** networking-engineer  
**Date:** 2026-08-15  
**Status:** Draft (Phase 2 Planning - Mon-Tue Deliverable)  
**References:** ADR-003, SRS §4, docs/protocol-phase2-design.md §4

---

## Executive Summary

This document specifies the complete WebRTC signaling flow for MONOTERMINAL Phase 2 P2P networking, including:
- SDP offer/answer exchange over existing WebSocket connection
- ICE candidate gathering and trickle ICE
- STUN→TURN→HTTPS three-tier fallback chain
- Dual-transport state management (WebSocket + DataChannel)
- Security: peer authentication via Ed25519, TURN credential scoping

**Key Decision:** Signaling reuses Phase 1 WebSocket connection (no separate signaling server).

---

## 1. High-Level Signaling Flow

```mermaid
sequenceDiagram
    participant C as Web Client
    participant WS as WebSocket (TLS 1.3)
    participant M as Master Daemon
    participant STUN as STUN Server
    participant TURN as TURN Relay (coturn)

    Note over C,M: Phase 1 Connection (Already Established)
    C->>WS: AttachRequest (protocol_version=2, auth_token)
    WS->>M: Forward AttachRequest
    M->>M: Validate JWT, check peer_id
    M->>WS: AttachResponse (scrollback, session_metadata)
    WS->>C: Forward AttachResponse
    
    Note over C,M: Phase 2 WebRTC Negotiation Begins
    
    rect rgb(200, 220, 255)
        Note over C,M: Step 1: Client Initiates WebRTC Offer
        C->>C: Create RTCPeerConnection
        C->>C: createOffer() → SDP offer
        C->>C: setLocalDescription(offer)
        C->>WS: WebRTCOffer {session_id, sdp, peer_id}
        WS->>M: Forward WebRTCOffer
    end
    
    rect rgb(220, 255, 220)
        Note over M: Step 2: Master Generates Answer + TURN Credentials
        M->>M: Validate peer_id matches JWT sub
        M->>M: Create RTCPeerConnection
        M->>M: setRemoteDescription(offer)
        M->>M: createAnswer() → SDP answer
        M->>M: Generate TURN credentials (15-min TTL)
        M->>WS: WebRTCAnswer {sdp, turn_credentials}
        WS->>C: Forward WebRTCAnswer
        C->>C: setRemoteDescription(answer)
    end
    
    rect rgb(255, 240, 220)
        Note over C,M: Step 3: ICE Candidate Exchange (Trickle ICE)
        par Client ICE Gathering
            C->>STUN: STUN Binding Request
            STUN-->>C: Public IP:port (srflx candidate)
            C->>WS: ICECandidate {candidate, sdp_mid, mline_index}
            WS->>M: Forward ICECandidate
            M->>M: addIceCandidate(client_candidate)
        and Master ICE Gathering
            M->>STUN: STUN Binding Request
            STUN-->>M: Public IP:port (srflx candidate)
            M->>WS: ICECandidate {candidate, sdp_mid, mline_index}
            WS->>C: Forward ICECandidate
            C->>C: addIceCandidate(master_candidate)
        end
    end
    
    rect rgb(255, 220, 220)
        Note over C,M: Step 4: ICE Connectivity Checks
        C<-->>M: STUN Binding Requests (candidate pairs)
        alt STUN Direct Success (60-95%)
            C->>M: Direct P2P Connection Established
            M->>C: ACK
            Note over C,M: Proceed to Step 5
        else STUN Fails (Symmetric NAT)
            C->>TURN: Allocate Request (TURN credentials)
            TURN-->>C: Relayed Address
            C->>TURN: Data → TURN → M (relayed path)
            M->>TURN: Data → TURN → C (relayed path)
            Note over C,M: TURN Relay Active (Step 5)
        end
    end
    
    rect rgb(220, 255, 240)
        Note over C,M: Step 5: DataChannel Establishment
        C->>M: DataChannel Open (DTLS handshake)
        M->>C: DataChannel ACK
        C->>M: First Protobuf Envelope over DataChannel
        M->>C: OutputData over DataChannel
        Note over C: WebSocket kept alive as fallback
    end
```

---

## 2. Detailed Step-by-Step Flow

### Step 1: Client Initiates WebRTC Offer

**Client Actions:**
1. User navigates to MONOTERMINAL web client, authenticates (JWT)
2. Client sends `AttachRequest` with `protocol_version=2` over WebSocket
3. Master responds with `AttachResponse` (session metadata, scrollback)
4. Client detects Phase 2 support → initiates WebRTC negotiation
5. Client creates `RTCPeerConnection` with ICE server config:
   ```javascript
   const pc = new RTCPeerConnection({
     iceServers: [
       { urls: 'stun:stun.l.google.com:19302' },  // Public STUN
       // TURN credentials provided in WebRTCAnswer
     ]
   });
   ```
6. Client creates DataChannel: `pc.createDataChannel('monoterminal')`
7. Client generates SDP offer: `pc.createOffer()`
8. Client sets local description: `pc.setLocalDescription(offer)`
9. Client sends `WebRTCOffer` message over WebSocket:
   ```protobuf
   WebRTCOffer {
     session_id: "abc123",
     client_id: "client-uuid",
     sdp: "<SDP offer string>",
     peer_id: "ed25519_pubkey_hex"
   }
   ```

**Timing:** ~50-100ms (SDP generation is fast)

---

### Step 2: Master Generates Answer + TURN Credentials

**Master Actions:**
1. Receive `WebRTCOffer` via WebSocket
2. **Security Check:** Validate `peer_id` matches JWT `sub` claim from `AttachRequest`
   - If mismatch → send `ErrorResponse(AUTH_FAILED)`
   - If valid → proceed
3. Create `RTCPeerConnection` (rust-webrtc):
   ```rust
   let config = RTCConfiguration {
       ice_servers: vec![
           RTCIceServer {
               urls: vec!["stun:stun.l.google.com:19302".to_string()],
               ..Default::default()
           },
       ],
       ..Default::default()
   };
   let pc = api.new_peer_connection(config).await?;
   ```
4. Set remote description from client's offer:
   ```rust
   let offer = RTCSessionDescription::offer(offer_sdp)?;
   pc.set_remote_description(offer).await?;
   ```
5. Generate SDP answer:
   ```rust
   let answer = pc.create_answer(None).await?;
   pc.set_local_description(answer.clone()).await?;
   ```
6. **Generate TURN credentials** (15-minute TTL):
   ```rust
   let turn_username = format!("{}:{}", expiry_timestamp, peer_id);
   let turn_credential = hmac_sha256(TURN_SECRET, &turn_username);
   ```
7. Send `WebRTCAnswer` over WebSocket:
   ```protobuf
   WebRTCAnswer {
     sdp: "<SDP answer string>",
     turn: TURNCredentials {
       urls: ["turn:turn.monoterminal.dev:3478", "turns:turn.monoterminal.dev:5349"],
       username: "1723680000:ed25519_pubkey",
       credential: "hmac_output_base64",
       expires_at_ms: 1723680900000
     },
     offer_timestamp_ms: <echo from offer>
   }
   ```

**Timing:** ~50-150ms (includes HMAC computation)

---

### Step 3: ICE Candidate Exchange (Trickle ICE)

**Why Trickle ICE?**
- **Fast:** Start connectivity checks before all candidates gathered (~2-3s vs ~5-8s)
- **Standard:** Supported by all browsers (Chrome, Firefox, Safari)
- **Complexity:** Slightly more complex state machine (acceptable trade-off)

**Client ICE Gathering:**
1. Client's `RTCPeerConnection` starts gathering ICE candidates:
   - **host:** Local IP addresses (192.168.x.x, 10.x.x.x)
   - **srflx:** STUN-discovered public IP:port (via `stun.l.google.com`)
   - **relay:** TURN relay address (via coturn, if STUN fails)
2. For each candidate discovered:
   ```javascript
   pc.addEventListener('icecandidate', (event) => {
     if (event.candidate) {
       sendMessage({
         ice_candidate: {
           session_id: sessionId,
           client_id: clientId,
           candidate: event.candidate.candidate,
           sdp_mid: event.candidate.sdpMid,
           sdp_mline_index: event.candidate.sdpMLineIndex
         }
       });
     }
   });
   ```
3. Client sends `ICECandidate` messages over WebSocket as candidates arrive

**Master ICE Gathering:**
1. Master's `RTCPeerConnection` gathers candidates (same process)
2. Master sends `ICECandidate` messages to client over WebSocket
3. Client adds master's candidates:
   ```javascript
   pc.addIceCandidate(new RTCIceCandidate({
     candidate: iceMsg.candidate,
     sdpMid: iceMsg.sdp_mid,
     sdpMLineIndex: iceMsg.sdp_mline_index
   }));
   ```

**Candidate Types (Typical Order):**
1. **host** candidates (immediate, ~0ms)
2. **srflx** candidates (STUN, ~100-500ms)
3. **relay** candidates (TURN, ~500-1000ms)

**Timing:** 2-5 seconds for full gathering (STUN + TURN)

---

### Step 4: ICE Connectivity Checks (Three-Tier Fallback)

**ICE Pairing Algorithm:**
- WebRTC automatically pairs candidates: client_candidate × master_candidate
- Tests each pair with STUN Binding Requests
- Prioritizes: host-to-host > host-to-srflx > srflx-to-srflx > relay paths

**Tier 1: STUN Direct Connection (Timeout: 10s)**

```
Client (behind NAT A) <--STUN Binding Request--> Master (behind NAT B)
                              ↓
                    NAT A & NAT B map ports
                              ↓
                  Direct UDP hole-punching succeeds
                              ↓
                    P2P connection established
```

**Success Rate:** 60-95% (depends on NAT type)
- **Full Cone NAT:** ~95%
- **Restricted Cone NAT:** ~85%
- **Port-Restricted Cone NAT:** ~70%
- **Symmetric NAT:** ~10-20% (usually fails)

**Tier 2: TURN Relay (Timeout: 5s)**

If STUN direct fails (symmetric NAT, firewall blocking UDP):

```
Client <---> TURN Relay (coturn) <---> Master
          (port 3478/5349)
```

1. Client sends `TURN Allocate Request` to coturn server
2. coturn validates credentials (HMAC-SHA256 from `WebRTCAnswer.turn`)
3. coturn allocates relay address, returns to client
4. Client uses relay candidate for ICE checks
5. Master connects to client via relay address

**Success Rate:** 98-99% (only fails if coturn unreachable)

**Cost:** Bandwidth relayed through VPS (~$0.05/GB)

**Tier 3: HTTPS Relay (No Timeout - Always Succeeds)**

If TURN relay unavailable (coturn down, network blocking TURN ports):

```
Client <--WebSocket (TLS 1.3)--> Master
     (Phase 1 connection still alive)
```

1. ICE negotiation fails completely (15s timeout total)
2. Client logs `P2P_STATE_FAILED`, continues using WebSocket
3. **No user-visible interruption:** Terminal data flows over WebSocket
4. Master marks connection as "WebSocket fallback" in telemetry

**Success Rate:** 100% (WebSocket already established)

---

### Step 5: DataChannel Establishment

**DTLS Handshake:**
1. ICE selects best candidate pair (e.g., STUN direct or TURN relay)
2. WebRTC performs DTLS 1.2 handshake over selected UDP path
3. DTLS provides:
   - **Encryption:** AES-128-GCM or AES-256-GCM
   - **Authentication:** Certificate fingerprint in SDP (self-signed OK)
   - **Integrity:** HMAC-SHA256

**DataChannel Open:**
1. Client's `pc.createDataChannel('monoterminal')` triggers SCTP association
2. SCTP INIT/INIT-ACK/COOKIE-ECHO handshake over DTLS tunnel
3. DataChannel fires `onopen` event on both sides:
   ```javascript
   dataChannel.addEventListener('open', () => {
     console.log('DataChannel opened, switching to P2P');
     sendProtobufMessage({ input_data: { data: "ls\n" } });
   });
   ```

**Protocol Switching:**
1. Client sends first Protobuf `Envelope` over DataChannel
2. Master receives, validates `sequence_number`, responds with `OutputData`
3. Client confirms P2P working → **optionally** close WebSocket (or keep as fallback)

**Timing:** 100-300ms (DTLS + SCTP handshake)

---

## 3. State Machine

### Client State Transitions

```
[INITIAL]
   │
   ├─> (Phase 1 only) ──> [WEBSOCKET_ONLY]
   │
   ├─> (Phase 2 capable) ──> [NEGOTIATING]
   │                            │
   │                            ├─> (ICE success) ──> [P2P_CONNECTED]
   │                            │                         │
   │                            │                         ├─> (DataChannel close) ──> [RECONNECTING]
   │                            │                         │                              │
   │                            │                         │                              ├─> (WebRTC retry) ──> [NEGOTIATING]
   │                            │                         │                              └─> (Retry failed) ──> [WEBSOCKET_FALLBACK]
   │                            │                         │
   │                            │                         └─> (Manual disconnect) ──> [DISCONNECTED]
   │                            │
   │                            └─> (ICE timeout 15s) ──> [WEBSOCKET_FALLBACK]
```

**State Descriptions:**

- **WEBSOCKET_ONLY:** Phase 1 client, no WebRTC capability
- **NEGOTIATING:** WebRTC offer sent, waiting for ICE/DTLS handshake
- **P2P_CONNECTED:** DataChannel open, terminal data flows over P2P
- **RECONNECTING:** DataChannel closed unexpectedly, attempting WebRTC reconnect
- **WEBSOCKET_FALLBACK:** WebRTC failed, using WebSocket (Phase 1 mode)
- **DISCONNECTED:** User closed tab or lost network

### Master State (Per Client)

```
[CLIENT_CONNECTED_WS]
   │
   ├─> (WebRTCOffer received) ──> [P2P_NEGOTIATING]
   │                                  │
   │                                  ├─> (DataChannel open) ──> [P2P_ACTIVE]
   │                                  │                             │
   │                                  │                             └─> (DataChannel close) ──> [CLIENT_CONNECTED_WS]
   │                                  │
   │                                  └─> (ICE timeout) ──> [CLIENT_CONNECTED_WS]
```

**Broadcast Strategy:**
- Master sends `OutputData` to **all** connected clients (both WebSocket and P2P)
- Deduplication by `sequence_number` on client side

---

## 4. Dual-Transport Management

### Why Keep WebSocket Alive?

**Option A: Close WebSocket After P2P Success**
- **Pros:** Saves 1 TCP connection per client
- **Cons:** No fallback if DataChannel closes mid-session

**Option B: Keep WebSocket Alive as Fallback** ✅ **RECOMMENDED**
- **Pros:** Instant fallback if P2P fails (mobile backgrounding, network change)
- **Cons:** 1 extra TCP connection (~4KB memory per client)

**Decision:** Keep WebSocket alive for Phase 2 MVP, optimize in Phase 3.

### Message Routing Rules

**Client → Master:**
- If `P2P_CONNECTED`: Send over DataChannel
- If DataChannel send fails: Retry over WebSocket, transition to `RECONNECTING`

**Master → Client:**
- Broadcast `OutputData` to **both** WebSocket and DataChannel (if open)
- Client deduplicates by `sequence_number`

**Signaling Messages (Always WebSocket):**
- `WebRTCOffer`, `WebRTCAnswer`, `ICECandidate` always go over WebSocket
- Signaling never uses DataChannel (chicken-and-egg problem)

---

## 5. Timeouts & Retry Strategy

| Phase | Timeout | Retry | Fallback |
|-------|---------|-------|----------|
| STUN Direct | 10s | None (ICE handles retries) | TURN |
| TURN Relay | 5s | None | WebSocket |
| ICE Total | 15s | None | WebSocket |
| DataChannel Idle | 60s | Send ping every 30s | Reconnect |
| WebSocket Ping | 30s | 3 retries | Close connection |

**Mobile Considerations (iOS Safari):**
- Background timeout: ~30s before network suspended
- On foreground resume: Detect DataChannel closed → reconnect WebRTC
- Target: <10s reconnection (SRS §7.2 acceptance criteria)

---

## 6. Security Considerations (Summary)

*Full details in `docs/security/webrtc-p2p-security.md` (separate deliverable)*

1. **Peer Authentication:** `WebRTCOffer.peer_id` must match JWT `sub` claim
2. **TURN Credentials:** Time-limited (15 min), HMAC-scoped to peer_id
3. **DTLS Encryption:** AES-GCM over DataChannel (automatic)
4. **Signaling Tampering:** WebSocket already TLS 1.3 encrypted
5. **ICE Candidate Injection:** Mitigated by DTLS fingerprint validation

---

## 7. Telemetry & Monitoring

**Track for Phase 2 Acceptance:**

| Metric | Target | Measurement |
|--------|--------|-------------|
| NAT traversal success rate | 65-80% | Direct measurement (not literature) |
| STUN success rate | 60-95% | ICE state transitions |
| TURN usage rate | 5-35% | TURN allocate requests |
| WebSocket fallback rate | 0-5% | `P2P_STATE_FAILED` count |
| Reconnection time (iOS) | <10s p95 | foreground → DataChannel open |

**Logging:**
- NAT type detection (Full Cone, Symmetric, etc.)
- ICE candidate types selected (host, srflx, relay)
- Fallback chain usage (STUN → TURN → WebSocket)
- Latency: Offer → Answer → DataChannel open

---

## 8. Implementation Checklist

**Monday Deliverables:**
- [x] Signaling flow diagram (this document)
- [ ] Security considerations doc (separate: `webrtc-p2p-security.md`)
- [ ] NAT traversal strategy finalization (ice-lite decision)

**Tuesday Deliverables:**
- [ ] Protocol integration spec (DataChannel framing)
- [ ] Review with rust-engineer-protocol (message boundaries)
- [ ] Final approval from principal-architect

**Phase 2 Implementation (starts Fri 2026-08-19):**
- [ ] rust-webrtc integration (networking-engineer)
- [ ] Signaling relay in WebSocket handler (rust-engineer-protocol)
- [ ] coturn TURN relay deployment (networking-engineer)
- [ ] Web client WebRTC negotiation (frontend-engineer)

---

**Next Steps:**
1. Monday 09:00 sync with rust-engineer-protocol (protocol boundaries)
2. Draft security considerations document (Monday PM)
3. Finalize NAT strategy (ice-lite vs full ICE decision)

---

**Status:** Draft complete, ready for Monday review.
