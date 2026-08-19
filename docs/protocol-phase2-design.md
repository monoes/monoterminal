# Phase 2 Protocol Schema Design

**Author:** rust-engineer-protocol  
**Date:** 2026-08-15  
**Status:** Draft — Ready for Review  
**Target:** Phase 2 P2P Implementation (starts 2026-08-18)

---

## Executive Summary

This document defines Protocol Buffers schema extensions for Phase 2 multi-client collaboration, presence indicators, and WebRTC P2P signaling. All extensions maintain backward compatibility with Phase 1 clients.

**Key Decisions:**
- ✅ Add 13 new message types to Envelope (fields 18-30)
- ✅ Version negotiation via `protocol_version` field in AttachRequest
- ✅ Scrollback pagination with cursor-based streaming
- ✅ 30-second heartbeat interval with 2-minute stale client timeout
- ✅ WebRTC signaling over existing WebSocket connection
- ✅ Ed25519 peer authentication handshake before WebRTC negotiation
- ✅ Full ICE (not ice-lite) - master may be behind NAT
- ✅ Transparent WebSocket fallback if WebRTC fails

---

## 1. Multi-Client Session Attach

### 1.1 Protocol Flow

```
Client → Master: AttachRequest (protocol_version=2, session_id="xyz")
Master → Client: AttachResponse (session_id, metadata, scrollback_cursor)
Client → Master: ScrollbackFetchRequest (cursor, limit=1000)
Master → Client: ScrollbackFetchResponse (lines, next_cursor, has_more)
Master → All Clients: PresenceUpdate (new client joined)
```

### 1.2 Schema Design

```protobuf
// ============================================================================
// Phase 2: Multi-Client Attach & Session Discovery
// ============================================================================

// Added to AttachRequest (Phase 2 fields)
message AttachRequest {
  string session_id = 1;
  optional string auth_token = 2;
  uint32 rows = 3;
  uint32 cols = 4;
  uint64 last_seen_sequence = 5;
  
  // Phase 2 additions
  optional uint32 protocol_version = 6;  // 1 = Phase 1, 2 = Phase 2
  optional ClientInfo client_info = 7;   // Device metadata
  optional bool request_presence = 8;     // Subscribe to presence updates (default: true)
}

message ClientInfo {
  string client_id = 1;           // UUID (generated client-side, stable per device)
  string device_name = 2;         // "Alice's iPhone 14", "Bob's Desktop"
  ClientType client_type = 3;     // WEB, MOBILE, DESKTOP
  string user_agent = 4;          // Browser/app user agent
  map<string, string> capabilities = 5; // {"webrtc": "true", "compression": "zstd"}
}

enum ClientType {
  CLIENT_TYPE_UNKNOWN = 0;
  CLIENT_TYPE_WEB = 1;
  CLIENT_TYPE_MOBILE = 2;
  CLIENT_TYPE_DESKTOP = 3;
}

// Enhanced AttachResponse for Phase 2
message AttachResponse {
  string session_id = 1;
  SessionMetadata metadata = 2;
  repeated Line scrollback = 3;  // Phase 1: full scrollback; Phase 2: last 100 lines
  
  // Phase 2 additions
  optional string scrollback_cursor = 4;  // Opaque cursor for pagination
  optional bool has_more_scrollback = 5;  // true if >100 lines available
  repeated ClientPresence attached_clients = 6; // Who else is attached
  optional uint64 total_scrollback_lines = 7; // Total history available
}

// Scrollback pagination (Phase 2 only)
message ScrollbackFetchRequest {
  string session_id = 1;
  string cursor = 2;              // From AttachResponse.scrollback_cursor
  uint32 limit = 3;               // Max lines to return (default: 1000, max: 5000)
  optional CompressionType compression = 4; // Request compressed response
}

message ScrollbackFetchResponse {
  repeated Line lines = 1;
  optional string next_cursor = 2;  // null if no more data
  bool has_more = 3;
  CompressionType compression = 4;  // Actual compression used
  uint32 total_bytes = 5;           // Uncompressed size (for UI progress)
}

// Session discovery (Phase 2)
message ListSessionsRequest {
  optional string auth_token = 1;  // JWT for RBAC filtering
}

message ListSessionsResponse {
  repeated SessionSummary sessions = 1;
}

message SessionSummary {
  string session_id = 1;
  SessionMetadata metadata = 2;
  uint32 attached_clients = 3;     // Number of clients currently attached
  uint64 total_scrollback_lines = 4;
  bool is_active = 5;               // Has received input in last 5 minutes
}
```

### 1.3 Answers to Questions

**Q1: Should scrollback be compressed (zstd) in SessionAttachResponse?**

✅ **YES** — Compression strategy:
- **AttachResponse.scrollback** (last 100 lines): Uncompressed for fast initial render
- **ScrollbackFetchRequest/Response**: Client requests compression via `compression` field
- Master applies zstd if `total_bytes > 4KB` (matches §3.1.3 threshold)
- Compression field in response indicates actual compression used (client capability negotiation)

**Q2: Should we paginate scrollback for large histories (>10k lines)?**

✅ **YES** — Cursor-based pagination:
- **Phase 1 behavior preserved**: AttachResponse includes full scrollback (for backward compatibility)
- **Phase 2 optimization**: AttachResponse returns last 100 lines + opaque cursor
- Client fetches older history via `ScrollbackFetchRequest` (1000 lines per request, max 5000)
- Cursor encodes `(session_id, oldest_line_number, timestamp)` — opaque to client, prevents tampering

**Q3: Do we need a "subscribe to updates" message after attach?**

❌ **NO** — Subscription is implicit:
- Master automatically streams `OutputData` to all attached clients (existing behavior)
- Master streams `PresenceUpdate` to clients with `request_presence=true` (default)
- Client unsubscribes by sending `DetachRequest` (existing message)

---

## 2. Presence Indicators

### 2.1 Protocol Flow

```
Client → Master: ClientHeartbeat (every 30s)
Master → All Clients: PresenceUpdate (on join/leave/heartbeat timeout)
Client → Master: InputFocusUpdate (on browser focus change)
Master → All Clients: PresenceUpdate (client is_active changed)
```

### 2.2 Schema Design

```protobuf
// ============================================================================
// Phase 2: Presence & Collaboration
// ============================================================================

message ClientPresence {
  string client_id = 1;           // Matches ClientInfo.client_id
  string device_name = 2;
  ClientType client_type = 3;
  uint64 last_seen_ms = 4;        // Unix timestamp (milliseconds)
  bool is_active = 5;             // Has input focus (from InputFocusUpdate)
  uint64 joined_at_ms = 6;        // When client attached to session
  optional string user_id = 7;    // From JWT sub claim (for RBAC display)
}

message ClientHeartbeat {
  string session_id = 1;
  string client_id = 2;
  uint64 timestamp_ms = 3;        // Client's current timestamp
}

message PresenceUpdate {
  string session_id = 1;
  repeated ClientPresence clients = 2;  // Current full client list
  PresenceEventType event_type = 3;     // What triggered this update
  optional string affected_client_id = 4; // Which client changed (for UI diff)
}

enum PresenceEventType {
  PRESENCE_EVENT_UNKNOWN = 0;
  PRESENCE_EVENT_CLIENT_JOINED = 1;
  PRESENCE_EVENT_CLIENT_LEFT = 2;
  PRESENCE_EVENT_HEARTBEAT_TIMEOUT = 3;  // Client evicted (no heartbeat for 2min)
  PRESENCE_EVENT_FOCUS_CHANGED = 4;      // Client is_active toggled
}

message InputFocusUpdate {
  string session_id = 1;
  string client_id = 2;
  bool is_active = 3;  // true = has focus, false = blurred
}
```

### 2.3 Answers to Questions

**Q1: Heartbeat interval: 30s? 60s?**

✅ **30 seconds** — Standard for web applications:
- WebSocket ping/pong every 30s (existing connection keep-alive)
- `ClientHeartbeat` message every 30s (presence-specific, includes client_id)
- Tolerant to 1 missed heartbeat (60s grace period before warning)
- 2-minute timeout triggers `HEARTBEAT_TIMEOUT` event and client eviction

**Q2: Should master proactively evict stale clients (no heartbeat for 2 minutes)?**

✅ **YES** — Proactive cleanup prevents:
- Memory leaks from zombie connections
- Incorrect presence indicators ("Alice is here" when she crashed)
- Wasted bandwidth broadcasting to dead connections

Eviction flow:
1. No heartbeat for 90s → Log warning, send ping
2. No heartbeat for 120s → Evict client, broadcast `PresenceUpdate(HEARTBEAT_TIMEOUT)`
3. Client reconnects → Fresh `AttachRequest` (new client_id or reuse stable UUID)

**Q3: Do we need "typing indicator" / input focus tracking?**

✅ **YES** — Essential for collaboration UX:
- `InputFocusUpdate` message tracks browser/app focus state
- `is_active` flag in `ClientPresence` shows who's currently typing
- Use case: Prevent input conflicts in multi-client attach (future Phase 2+)
- Low overhead: Only sent on focus/blur events, not every keystroke

---

## 3. Input Broadcasting & Multi-Client Awareness

### 3.1 Enhanced InputData

```protobuf
message InputData {
  bytes data = 1;
  optional string auth_token = 2;
  
  // Phase 2 additions
  optional string client_id = 3;  // Who sent this input (audit trail)
  optional uint64 timestamp_ms = 4; // Client timestamp (for latency tracking)
}
```

**Broadcasting Strategy:**
- ❌ **Do NOT echo InputData** to other clients (security/privacy risk)
- ✅ **Broadcast OutputData** to all attached clients (existing behavior)
- Each client sees the same terminal output (natural collaboration feedback)
- `client_id` in InputData used for audit logs only (SRS §5 compliance)

### 3.2 Output Deduplication

Phase 2 clients must deduplicate `OutputData` by `sequence` number:
- Master assigns monotonic `sequence` to each output chunk
- Late-joining client receives scrollback + live stream → may see duplicates
- Client discards `OutputData` with `sequence ≤ last_seen_sequence`

---

## 4. WebRTC P2P Signaling

### 4.1 Protocol Flow

```
# Phase 1: Peer Authentication
Client A → Master: PeerHandshake (session_id, client_id, peer_id, timestamp)
Master → Client A: PeerHandshakeResponse (challenge)
Client A → Master: PeerHandshake (challenge_response = Ed25519.sign(challenge))
Master → Client A: PeerHandshakeResponse (accepted=true, nonce)

# Phase 2: WebRTC Signaling
Client A → Master: WebRTCOffer (SDP offer, peer_id, nonce)
Master → Client A: WebRTCAnswer (SDP answer, TURN credentials)
Client A ↔ Master: ICECandidate exchange

# Phase 3: P2P Connection
[WebRTC DataChannel established]
Client A ↔ Master: Direct P2P terminal traffic (Protobuf Envelope over DataChannel)
```

**Authentication Flow Details:**
1. **Initial PeerHandshake**: Client sends peer_id (Ed25519 public key)
2. **Challenge**: Master generates random 32-byte challenge, returns to client
3. **Challenge Response**: Client signs challenge with Ed25519 private key, proves key ownership
4. **Nonce**: Master returns single-use nonce, binds to upcoming WebRTCOffer
5. **WebRTCOffer Validation**: Master verifies nonce matches handshake, peer_id matches JWT sub claim

### 4.2 Schema Design

```protobuf
// ============================================================================
// Phase 2: WebRTC P2P Signaling & Peer Authentication
// ============================================================================

// Peer authentication handshake (before WebRTC negotiation)
message PeerHandshake {
  string session_id = 1;          // Session to establish P2P for
  string client_id = 2;           // From ClientInfo
  string peer_id = 3;             // Client's Ed25519 public key (hex)
  bytes challenge_response = 4;   // Ed25519 signature of challenge (proves key ownership)
  uint64 timestamp_ms = 5;        // Request timestamp (prevents replay attacks)
}

message PeerHandshakeResponse {
  bool accepted = 1;              // Authentication successful
  optional string error_message = 2; // Reason if rejected
  optional bytes challenge = 3;   // Server challenge for client to sign (if initial handshake)
  uint64 nonce = 4;               // Single-use nonce for WebRTC negotiation
}

message WebRTCOffer {
  string session_id = 1;          // Session to establish P2P for
  string client_id = 2;           // From ClientInfo
  string sdp = 3;                 // SDP offer
  string peer_id = 4;             // Client's Ed25519 public key (hex) (must match PeerHandshake)
  uint64 nonce = 5;               // From PeerHandshakeResponse (binds offer to handshake)
}

message WebRTCAnswer {
  string sdp = 1;                 // SDP answer
  optional TURNCredentials turn = 2; // TURN server credentials (if needed)
  uint64 offer_timestamp_ms = 3;  // Echo from offer (for latency calc)
}

message TURNCredentials {
  repeated string urls = 1;       // ["turn:coturn.example.com:3478"]
  string username = 2;            // Time-limited TURN username
  string credential = 3;          // TURN password
  uint64 expires_at_ms = 4;       // Credential TTL (Unix timestamp)
}

message ICECandidate {
  string session_id = 1;
  string client_id = 2;
  string candidate = 3;           // ICE candidate string
  optional string sdp_mid = 4;
  optional uint32 sdp_mline_index = 5;
}

message P2PConnectionStatus {
  string session_id = 1;
  string client_id = 2;
  P2PState state = 3;
  optional string error_message = 4;
}

enum P2PState {
  P2P_STATE_UNKNOWN = 0;
  P2P_STATE_NEGOTIATING = 1;      // Exchanging SDP/ICE
  P2P_STATE_CONNECTED = 2;        // DataChannel open
  P2P_STATE_FAILED = 3;           // NAT traversal failed, fallback to WebSocket
  P2P_STATE_DISCONNECTED = 4;     // Connection lost, attempting reconnect
}
```

### 4.3 Answers to Questions

**Q1: Should signaling go over existing WebSocket connection?**

✅ **YES** — Reuse WebSocket for signaling:
- Simpler architecture (no second connection to manage)
- TLS 1.3 already established (no additional handshake)
- Master already has client authentication context (JWT validated)
- WebRTC DataChannel handles P2P data after negotiation

**Q2: Or separate signaling channel (dedicated port)?**

❌ **NO** — Separate port adds complexity:
- Firewall configuration burden (need to open 2 ports instead of 1)
- Duplicate TLS handshake overhead
- Additional connection state tracking
- No performance benefit (signaling is low-volume)

**Q3: Do we need TURN credentials in WebRTCAnswer?**

✅ **YES** — Essential for NAT traversal:
- 65-80% success with STUN alone (SRS §7.2 acceptance criteria)
- TURN provides fallback for symmetric NATs (remaining 20-35%)
- Master generates time-limited TURN credentials (15-minute TTL)
- `TURNCredentials` message includes coturn server URLs + ephemeral username/password

**Security Note:** TURN credentials are single-use, scoped to requesting client's Ed25519 peer_id.

### 4.4 WebRTC Transport Decisions (Resolved 2026-08-15)

**Q4: ICE Strategy - ice-lite vs Full ICE?**

✅ **Full ICE** — Master daemon may be behind NAT:
- **ice-lite** (RFC 5245 §2.3) assumes server has public IP, only responds to client candidates
- **Problem:** MONOTERMINAL master often behind home NAT (WiFi router)
- **Solution:** Full ICE allows both master AND client to gather candidates
- **Optimization:** Use Trickle ICE (send candidates incrementally vs. waiting for full gathering)

**Q5: DataChannel Protocol - Reuse Protobuf Envelope or raw binary?**

✅ **REUSE Protobuf Envelope** — Same format for both WebSocket and DataChannel:
- Protocol symmetry: Client code doesn't care about transport layer
- Fallback seamless: WebRTC fails → WebSocket resume with zero protocol changes
- Schema evolution unified: Version negotiation works identically on both transports
- Debug tooling reuse: Same Wireshark dissector, same logging
- Trade-off: ~20 bytes overhead per message (<4% for 512B-64KB chunks)

**Q6: Message Framing on DataChannel?**

✅ **Rely on DataChannel message boundaries** (no length prefix):
- DataChannel is message-oriented (RFC 8831) - preserves message boundaries
- Length prefix redundant (4 bytes saved per message, ~20% overhead reduction)
- WebSocket still uses length prefix (stream-oriented, needs framing)
- Code symmetry preserved: Both transports send/receive `Envelope` messages
- SRS §3.1.2 compliance: DataChannel message boundary is the delimiter

**Q7: P2P Fallback Strategy?**

✅ **Automatic transparent fallback to WebSocket:**

**Flow:**
1. Client initiates WebRTC negotiation (sends WebRTCOffer over WebSocket)
2. **Keep WebSocket alive during negotiation** (don't close until DataChannel confirmed open)
3. WebRTC negotiation timeout (15s total: 10s STUN + 5s TURN) → log failure, continue on WebSocket
4. If DataChannel opens successfully → optionally close WebSocket (keep as fallback for reliability)
5. If DataChannel fails mid-session → reconnect WebSocket, resume via AttachRequest

**User experience:** No visible interruption - terminal keeps working regardless of P2P success/failure.

**Q8: TURN Credential TTL?**

✅ **15 minutes** — Aligns with JWT auth_token:
- Long enough for WebRTC negotiation (<30s) + connection lifetime
- Short enough to limit abuse if credentials leak
- coturn REST API authentication (RFC 7635): `username = "timestamp:peer_id"`, `credential = HMAC-SHA256(secret, username)`
- Master validates peer_id before generating credentials

**Q9: PeerHandshake Challenge Format?**

✅ **32-byte pure random** (no embedded metadata):
- Simpler implementation (`rand::thread_rng().fill_bytes(&mut challenge)`)
- session_id/timestamp already in signed message payload
- 256-bit security matches Ed25519 key strength
- No parsing overhead or format validation needed

**Q10: Nonce Lifetime?**

✅ **60-second window** (allows WebRTCOffer retry):
- Tolerates ICE timeout, network glitch, client retry logic
- Master storage: `HashMap<nonce, (peer_id, expires_at)>`
- Background cleanup task: remove expired nonces every 60s
- Single-use within 60s window (nonce deleted on first WebRTCOffer validation)

**Q11: Dual-Transport Strategy?**

✅ **Keep both active** (WebSocket + DataChannel):
- Instant fallback if DataChannel closes (mobile app backgrounding, NAT rebinding)
- Memory overhead: ~4KB per client (acceptable for Phase 2 MVP, 1000 clients = 4MB)
- Optimize in Phase 3 if profiling shows bottleneck
- Master broadcasts OutputData to both transports (client deduplicates by sequence_number)

---

## 5. Schema Evolution & Backward Compatibility

### 5.1 Version Negotiation

```protobuf
message AttachRequest {
  // ...existing fields...
  optional uint32 protocol_version = 6;  // 1 = Phase 1, 2 = Phase 2
}

message AttachResponse {
  // ...existing fields...
  optional uint32 protocol_version = 8;  // Master's max supported version
}
```

**Negotiation Flow:**
1. **Phase 1 Client** → Master: `AttachRequest` (no protocol_version field)
   - Master detects missing field → assumes `protocol_version = 1`
   - Master responds: `AttachResponse` (Phase 1 schema only, no Phase 2 fields)
   - Client receives scrollback directly (no pagination)

2. **Phase 2 Client** → Master: `AttachRequest` (protocol_version=2)
   - Master responds: `AttachResponse` (with Phase 2 fields: cursor, attached_clients)
   - Client uses pagination, presence, WebRTC signaling

3. **Future Phase 3 Client** → Phase 2 Master: `AttachRequest` (protocol_version=3)
   - Master responds: `AttachResponse` (protocol_version=2) — downgrade to common max
   - Client disables Phase 3-only features, uses Phase 2 baseline

### 5.2 Unknown Message Handling

**Phase 1 Client** receives Phase 2 message (e.g., `PresenceUpdate`):
- Protobuf decoder skips unknown `oneof` field (spec-compliant behavior)
- Client logs warning: "Unknown message type 25, ignoring"
- Connection remains stable (no errors)

**Phase 2 Master** serves Phase 1 Client:
- Never sends Phase 2 messages (`PresenceUpdate`, `ScrollbackFetchResponse`) to v1 clients
- Version-gated broadcast: `if client.protocol_version >= 2 { send_presence_update() }`

### 5.3 Schema Compatibility Rules

**Additive-Only Changes** (per SRS §3.1.1):
- ✅ Add new message types to `Envelope.oneof` (fields 18-28)
- ✅ Add optional fields to existing messages (e.g., `AttachRequest.protocol_version`)
- ✅ Add new enum values (e.g., `ErrorCode::SESSION_FULL = 7`)

**Forbidden Changes** (breaking):
- ❌ Remove or renumber existing fields
- ❌ Change field types (e.g., `uint32 → uint64`)
- ❌ Rename message types (breaks reflection/deserialization)

---

## 6. Complete Envelope Update

### 6.1 Phase 2 Envelope

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  oneof message {
    // Phase 1 messages (fields 2-17)
    AttachRequest attach_request = 2;
    AttachResponse attach_response = 3;
    InputData input_data = 4;
    OutputData output_data = 5;
    ResizeRequest resize_request = 6;
    DetachRequest detach_request = 7;
    ErrorResponse error_response = 8;
    DashboardRequest dashboard_request = 9;
    DashboardResponse dashboard_response = 10;
    HealthCheckRequest health_check_request = 11;
    HealthCheckResponse health_check_response = 12;
    UpgradeRequest upgrade_request = 13;
    UpgradeResponse upgrade_response = 14;
    DetectionRequest detection_request = 15;
    DetectionResponse detection_response = 16;
    MonitoringData monitoring_data = 17;
    
    // Phase 2 messages (fields 18-30)
    ScrollbackFetchRequest scrollback_fetch_request = 18;
    ScrollbackFetchResponse scrollback_fetch_response = 19;
    ListSessionsRequest list_sessions_request = 20;
    ListSessionsResponse list_sessions_response = 21;
    ClientHeartbeat client_heartbeat = 22;
    PresenceUpdate presence_update = 23;
    InputFocusUpdate input_focus_update = 24;
    WebRTCOffer webrtc_offer = 25;
    WebRTCAnswer webrtc_answer = 26;
    ICECandidate ice_candidate = 27;
    P2PConnectionStatus p2p_connection_status = 28;
    PeerHandshake peer_handshake = 29;
    PeerHandshakeResponse peer_handshake_response = 30;
  }
}
```

### 6.2 Error Code Extensions

```protobuf
enum ErrorCode {
  UNKNOWN = 0;
  SESSION_NOT_FOUND = 1;
  AUTH_FAILED = 2;               // JWT invalid, signature verification failed, peer_id mismatch
  PERMISSION_DENIED = 3;
  RATE_LIMIT_EXCEEDED = 4;
  INVALID_REQUEST = 5;
  SERVER_ERROR = 6;
  
  // Phase 2 additions
  SESSION_FULL = 7;              // Max clients attached (configurable limit)
  PROTOCOL_VERSION_MISMATCH = 8; // Client too old/new
  SCROLLBACK_CURSOR_INVALID = 9; // Tampered or expired cursor
  P2P_NEGOTIATION_FAILED = 10;   // WebRTC handshake failed
  HEARTBEAT_TIMEOUT = 11;        // Client evicted (stale connection)
  CHALLENGE_EXPIRED = 12;        // PeerHandshake challenge >60s old
  NONCE_INVALID = 14;            // Nonce not found, expired, or already consumed
}
```

---

## 7. Compression Strategy

### 7.1 Adaptive Compression (SRS §3.1.3)

**Trigger Conditions:**
1. **Chunk size >4KB**: Apply zstd to individual `OutputData` or `ScrollbackFetchResponse`
2. **Client capability**: Check `ClientInfo.capabilities["compression"] == "zstd"`
3. **Backpressure >50%**: Per-client write buffer exceeds 512KB of 1MB limit

**Compression Levels:**
- `zstd::level::fast()` (level 1) — 100-200 MB/s encode, ~60% compression ratio
- Target: <5ms compression latency for 64KB chunks (SRS §2.2 <30ms p95 budget)

### 7.2 Message-Specific Compression

| Message Type | Compress? | Threshold | Typical Size |
|--------------|-----------|-----------|--------------|
| `OutputData` | ✅ | >4KB | 512B - 64KB |
| `ScrollbackFetchResponse` | ✅ | >4KB | 50KB - 500KB |
| `AttachResponse.scrollback` | ❌ | N/A | <10KB (100 lines) |
| `PresenceUpdate` | ❌ | N/A | <1KB |
| Signaling (WebRTC) | ❌ | N/A | <2KB |

**Rationale:** Small, latency-sensitive messages (presence, signaling) bypass compression overhead.

---

## 8. Implementation Checklist

### 8.1 Protocol Schema (rust-engineer-protocol)

- [ ] Add Phase 2 messages to `messages.proto` (fields 18-28)
- [ ] Add `ClientInfo`, `ClientPresence`, `TURNCredentials` types
- [ ] Add `protocol_version` field to `AttachRequest`/`AttachResponse`
- [ ] Update `ErrorCode` enum with Phase 2 codes
- [ ] Run `prost-build` code generation
- [ ] Add roundtrip tests for new message types
- [ ] Fuzz parser with `cargo-fuzz` (protobuf_parser target)

### 8.2 WebSocket Server (rust-engineer-protocol + networking-engineer)

- [ ] Version negotiation logic in attach handler
- [ ] Scrollback pagination cursor implementation (opaque encoding)
- [ ] Presence tracking: ClientPresence registry per session
- [ ] Heartbeat timeout handler (30s interval, 2min eviction)
- [ ] Broadcast `PresenceUpdate` to all attached clients (version-gated)
- [ ] WebRTC signaling relay (offer/answer/ICE forwarding)

### 8.3 PeerHandshake Authentication (rust-engineer-protocol + security-engineer)

**Nonce storage (lazy cleanup strategy):**
```rust
fn consume_nonce(nonce: &str, expected_peer_id: &str) -> Result<(), ErrorCode> {
    match nonce_table.get(nonce) {
        Some((peer_id, expires_at)) if current_time() > expires_at => {
            nonce_table.remove(nonce); // Lazy cleanup
            Err(ErrorCode::NONCE_INVALID) // Expired
        }
        Some((peer_id, _)) if peer_id != expected_peer_id => {
            Err(ErrorCode::AUTH_FAILED) // Wrong peer_id
        }
        Some(_) => {
            nonce_table.remove(nonce); // Consume (single-use)
            Ok(())
        }
        None => Err(ErrorCode::NONCE_INVALID) // Not found
    }
}
```

**Error handling:**
- `CHALLENGE_EXPIRED = 12`: Challenge >60s old
- `AUTH_FAILED = 2`: Invalid Ed25519 signature, peer_id mismatch
- `NONCE_INVALID = 14`: Nonce not found, expired, or already consumed

### 8.4 Dual-Transport Management (networking-engineer)

**Shared sequence_number across WebSocket + DataChannel:**
```rust
struct SessionState {
    next_sequence: AtomicU64, // Shared across transports
    websocket: Option<WebSocketSender>,
    datachannel: Option<DataChannelSender>,
}

fn send_output(&mut self, data: &[u8]) {
    let seq = self.next_sequence.fetch_add(1, Ordering::SeqCst);
    let envelope = Envelope { sequence_number: seq, output_data: ... };
    
    // Broadcast to both transports (client deduplicates by sequence)
    if let Some(dc) = &self.datachannel {
        dc.send(envelope.clone());
    }
    if let Some(ws) = &self.websocket {
        ws.send(envelope);
    }
}
```

**Rationale:**
- Client deduplicates by sequence_number (transport-agnostic)
- Simplifies fallback: WebSocket resumes from last_seen_sequence
- Matches Phase 1 single-sequence stream behavior

### 8.3 Web Client (frontend-engineer)

- [ ] Send `protocol_version=2` in AttachRequest
- [ ] Handle `ScrollbackFetchRequest` for pagination
- [ ] Display presence indicators (avatars, typing status)
- [ ] Send `ClientHeartbeat` every 30s
- [ ] Send `InputFocusUpdate` on window focus/blur
- [ ] WebRTC client negotiation (offer/answer/ICE handling)

### 8.4 Storage (rust-engineer-storage)

- [ ] Persist scrollback with line numbers (for cursor pagination)
- [ ] Store client presence history (for audit logs)
- [ ] Index on `(session_id, line_number)` for fast cursor lookups

---

## 9. Testing Strategy

### 9.1 Unit Tests

```rust
#[test]
fn test_protocol_version_negotiation() {
    // Phase 1 client (no version field) → Phase 2 master
    let req = AttachRequest { protocol_version: None, .. };
    let resp = handle_attach(req);
    assert!(resp.attached_clients.is_none()); // No Phase 2 fields
}

#[test]
fn test_scrollback_cursor_pagination() {
    // Request 1000 lines, verify cursor points to correct offset
    let cursor = encode_cursor(session_id, oldest_line, timestamp);
    let resp = fetch_scrollback(cursor, 1000);
    assert_eq!(resp.lines.len(), 1000);
    assert!(resp.has_more);
}

#[test]
fn test_presence_heartbeat_timeout() {
    // Simulate 2-minute no-heartbeat → expect eviction
    advance_time(Duration::from_secs(120));
    let update = check_stale_clients();
    assert_eq!(update.event_type, PresenceEventType::HEARTBEAT_TIMEOUT);
}
```

### 9.2 Integration Tests

```typescript
// Web client: Multi-client attach
it('should receive presence update when second client joins', async () => {
  const client1 = await connect({ protocol_version: 2 });
  const client2 = await connect({ protocol_version: 2 });
  
  const update = await client1.waitForMessage('presence_update');
  expect(update.clients).toHaveLength(2);
  expect(update.event_type).toBe('CLIENT_JOINED');
});
```

### 9.3 Fuzz Tests

```rust
// Fuzz scrollback cursor parsing (prevent tampering)
fuzz_target!(|data: &[u8]| {
    let cursor = String::from_utf8_lossy(data);
    let _ = decode_cursor(&cursor); // Should never panic
});
```

---

## 10. Performance Targets

### 10.1 Latency Budget (SRS §2.2)

| Operation | Target | Measurement |
|-----------|--------|-------------|
| AttachRequest → AttachResponse | <50ms | p95 local network |
| ScrollbackFetchRequest (1000 lines) | <100ms | p95 w/ zstd |
| PresenceUpdate broadcast (10 clients) | <20ms | p99 |
| WebRTC offer → answer | <200ms | p95 (includes TURN credential generation) |

### 10.2 Throughput Targets

- **Scrollback fetch**: 500-1000 MB/s (zstd decompression, criterion.rs benchmark)
- **Presence updates**: 10k updates/sec (stress test with 100 sessions × 10 clients)
- **Concurrent sessions**: 1000 sessions, 10 clients each (SRS §7.2 acceptance criteria)

---

## 11. Timeline & Deliverables

**Friday 2026-08-15 Afternoon:**
- ✅ This design document (COMPLETE - 660 lines, 12 sections)
- ✅ Resolved 5 design questions with networking-engineer (Q5-Q9: DataChannel protocol, fallback, ICE, TURN, framing)
- ✅ Added PeerHandshake authentication schema (Ed25519 challenge-response)
- ✅ Complete `.proto` schema draft (13 new message types, fields 18-30)

**Friday 2026-08-15 Evening (First Update):**
- ✅ Resolved 3 additional design questions (Q9-Q11: challenge format, nonce lifetime, dual-transport)
- ✅ **All WebRTC design decisions complete** (8 questions resolved Friday)
- ✅ Monday meeting scope reduced to implementation details only

**Friday 2026-08-15 Evening (Second Update):**
- ✅ Resolved 3 implementation questions (Q12-Q14: error handling, nonce cleanup, sequence_number)
- ✅ **All implementation strategy decisions complete** (11 questions resolved Friday)
- ✅ Rust code examples provided by networking-engineer (nonce cleanup, dual-transport)
- ✅ Monday meeting scope reduced to final review + sign-off

**Monday 2026-08-17 09:00:**
- [ ] Final review session with networking-engineer (45 min)
- [ ] Sign-off on implementation strategy (error codes, nonce cleanup, sequence_number)
- [ ] Integration coordination with frontend-engineer (Phase 2 client features)

**Tuesday 2026-08-18 (Phase 2 Kickoff):**
- [ ] Commit updated `messages.proto` to main branch
- [ ] Run `prost-build` + verify roundtrip tests pass
- [ ] Hand off to networking-engineer for WebSocket server integration

**Week of 2026-08-18:**
- [ ] Fuzz tests for cursor parsing, presence timeout logic
- [ ] Benchmark scrollback pagination (criterion.rs)
- [ ] Integration tests with frontend-engineer (multi-client attach)

---

## 12. Open Questions for Review

### 12.1 Resolved (Friday 2026-08-15)

**networking-engineer answers (first round - afternoon):**
- ✅ Q5: WebRTC DataChannel protocol → **REUSE Protobuf Envelope** (same format both transports)
- ✅ Q6: P2P fallback → **YES, automatic transparent fallback** to WebSocket
- ✅ Q7: ICE strategy → **Full ICE** (not ice-lite, master may be behind NAT)
- ✅ Q8: TURN TTL → **15 minutes** (aligns with JWT auth)
- ✅ Q9: DataChannel framing → **Rely on message boundaries** (no length prefix)

**networking-engineer answers (second round - evening):**
- ✅ Q9: PeerHandshake challenge format → **32-byte pure random** (no embedded metadata)
- ✅ Q10: Nonce lifetime → **60-second window** (allows WebRTCOffer retry)
- ✅ Q11: Dual-transport strategy → **Keep both active** (instant fallback, ~4KB overhead)

### 12.2 Implementation Decisions (Friday 2026-08-15 Evening)

**networking-engineer answers (third round):**
- ✅ Q12: Error handling → `CHALLENGE_EXPIRED = 12` (new), `AUTH_FAILED = 2` (reuse), `NONCE_INVALID = 14` (new)
- ✅ Q13: Nonce cleanup → **Lazy cleanup** (simpler, zero overhead when idle)
- ✅ Q14: Dual-transport sequence → **Shared sequence_number** (single monotonic counter, client deduplicates)

### 12.3 Remaining (Non-Blocking for Phase 2 MVP)

**For rust-backend-lead:**
1. Approve `protocol_version` negotiation strategy?
2. Scrollback cursor encoding: use signed JWT or simple base64(session_id||line_number||hmac)?

**For security-engineer:**
3. Should `client_id` be cryptographically bound to Ed25519 peer_id? (Or keep separate for device flexibility?)

**For frontend-engineer:**
4. Presence UI: show all clients, or collapse to "3 others" for >4 clients?
5. Scrollback pagination: infinite scroll or "Load More" button?

**Note:** Questions 1-3 are architectural review items (not blocking implementation). Questions 4-5 are UI/UX decisions (frontend scope).

---

## Appendix A: Complete Phase 2 Schema

See `proto/monoterminal/v1/messages.proto` (to be committed on 2026-08-18).

Total message types: **30** (17 Phase 1 + 13 Phase 2)  
Estimated codegen size: ~9KB Rust code (prost output)  
Wire overhead: 10-20 bytes per message (Protobuf envelope + sequence_number)

**Phase 2 Message Breakdown:**
- Multi-client: ScrollbackFetchRequest/Response, ListSessionsRequest/Response (4)
- Presence: ClientHeartbeat, PresenceUpdate, InputFocusUpdate (3)
- WebRTC: PeerHandshake/Response, WebRTCOffer/Answer, ICECandidate, P2PConnectionStatus (6)

---

**END OF DOCUMENT**
