# Phase 2 Protocol Message Inventory

**Version:** 1.0  
**Date:** 2026-08-18  
**Author:** principal-architect  
**Status:** Ready for Implementation

---

## Executive Summary

Complete inventory of MONOTERMINAL wire protocol messages for Phase 1 (baseline) and Phase 2 (P2P + collaboration). Total: **30 message types** (17 Phase 1 + 13 Phase 2) using Protocol Buffers v3 schema.

**Field Number Allocation:**
- **1:** Envelope metadata (`sequence_number`)
- **2-17:** Phase 1 message types (baseline terminal I/O)
- **18-30:** Phase 2 message types (P2P, collaboration, persistence)
- **28-29:** Envelope metadata fields (`protocol_version`, `compression`)
- **31-39:** Reserved for Phase 3 expansion

---

## Message Inventory Table

| Field | Message Type | Purpose | Typical Size | Compression | Phase |
|-------|-------------|---------|--------------|-------------|-------|
| **1** | **Envelope Metadata** | | | | |
| 1 | `sequence_number` (uint64) | Monotonic sequence for deduplication | 8B | N/A | 1 |
| **2-17** | **Phase 1: Terminal I/O & Monomind Integration** | | | | |
| 2 | `AttachRequest` | Connect to session, JWT auth, terminal dimensions | 200-500B | ❌ No | 1 |
| 3 | `AttachResponse` | Session metadata, initial scrollback (100 lines) | 2-10KB | ❌ No | 1 |
| 4 | `InputData` | Keyboard input, paste data | 10-200B | ❌ No | 1 |
| 5 | `OutputData` | Terminal output chunk (PTY stdout/stderr) | 512B-64KB | ✅ Yes (>4KB) | 1 |
| 6 | `ResizeRequest` | Terminal window resize (rows×cols) | 50B | ❌ No | 1 |
| 7 | `DetachRequest` | Gracefully disconnect from session | 30B | ❌ No | 1 |
| 8 | `ErrorResponse` | Error code + human-readable message | 100-500B | ❌ No | 1 |
| 9 | `DashboardRequest` | Request monomind org dashboard data | 50B | ❌ No | 1 |
| 10 | `DashboardResponse` | Org status, agents, tasks, KG stats | 1-5KB | ✅ Yes (>4KB) | 1 |
| 11 | `HealthCheckRequest` | Request monomind health check | 30B | ❌ No | 1 |
| 12 | `HealthCheckResponse` | Health status, warnings, errors | 200B-2KB | ❌ No | 1 |
| 13 | `UpgradeRequest` | Trigger monomind CLI upgrade | 50B | ❌ No | 1 |
| 14 | `UpgradeResponse` | Upgrade result, new version | 100-500B | ❌ No | 1 |
| 15 | `DetectionRequest` | Detect monomind installation | 30B | ❌ No | 1 |
| 16 | `DetectionResponse` | Monomind installed, version, features | 100-300B | ❌ No | 1 |
| 17 | `MonitoringData` | Periodic stats (org, agents, tasks) | 500B-2KB | ❌ No | 1 |
| **18-30** | **Phase 2: P2P, Collaboration, Persistence** | | | | |
| 18 | `ScrollbackFetchRequest` | Paginate scrollback history (cursor-based) | 100B | ❌ No | 2 |
| 19 | `ScrollbackFetchResponse` | Scrollback page (1000-5000 lines) | 50KB-500KB | ✅ Yes (>4KB) | 2 |
| 20 | `ListSessionsRequest` | Discover active sessions (RBAC filtered) | 50B | ❌ No | 2 |
| 21 | `ListSessionsResponse` | Session summaries (metadata, clients, activity) | 500B-5KB | ❌ No | 2 |
| 22 | `ClientHeartbeat` | Keep-alive ping (30s interval) | 80B | ❌ No | 2 |
| 23 | `PresenceUpdate` | Broadcast client join/leave/focus change | 200B-1KB | ❌ No | 2 |
| 24 | `InputFocusUpdate` | Client active/idle state change | 60B | ❌ No | 2 |
| 25 | `WebRTCOffer` | SDP offer (P2P connection initiation) | 1-2KB | ❌ No | 2 |
| 26 | `WebRTCAnswer` | SDP answer + TURN credentials | 1-2KB | ❌ No | 2 |
| 27 | `ICECandidate` | Trickle ICE candidate (NAT traversal) | 100-300B | ❌ No | 2 |
| 28 | `P2PConnectionStatus` | WebRTC state (negotiating/connected/failed) | 100B | ❌ No | 2 |
| 29 | `PeerHandshake` | Ed25519 challenge-response (P2P auth) | 200B | ❌ No | 2 |
| 30 | `PeerHandshakeResponse` | Challenge or nonce (P2P auth flow) | 150B | ❌ No | 2 |
| **28-29** | **Envelope Metadata (Phase 2)** | | | | |
| 28 | `protocol_version` (uint32) | Version negotiation (0=v1.0, 1=v1.1, 2=v1.2) | 4B | N/A | 2 |
| 29 | `compression` (enum) | Compression type (NONE=0, ZSTD=1) | 1B | N/A | 2 |

---

## Message Details

### Phase 1: Terminal I/O & Monomind Integration

#### AttachRequest (field 2)
**Purpose:** Establish connection to a session, authenticate with JWT, negotiate protocol version  
**Key Fields:**
- `session_id` (string): Session UUID (empty = create default session)
- `auth_token` (string, optional): JWT for RBAC authentication
- `rows`, `cols` (uint32): Terminal dimensions
- `last_seen_sequence` (uint64): Resume from sequence (for reconnect)
- `protocol_version` (uint32, Phase 2+): Client's max supported protocol version
- `client_info` (ClientInfo, Phase 2+): Device metadata, capabilities

**Size:** 200-500 bytes  
**Compression:** No (latency-sensitive handshake)

---

#### AttachResponse (field 3)
**Purpose:** Confirm session attach, return metadata and initial scrollback  
**Key Fields:**
- `session_id` (string): Session UUID
- `metadata` (SessionMetadata): Shell, working directory, created timestamp
- `scrollback` (repeated Line): Last 100 lines (Phase 2), full history (Phase 1)
- `scrollback_cursor` (string, Phase 2+): Opaque cursor for pagination
- `attached_clients` (repeated ClientPresence, Phase 2+): Who else is connected

**Size:** 2-10 KB (100 lines × 80-100 bytes/line)  
**Compression:** No (initial payload, want fast first paint)

---

#### InputData (field 4)
**Purpose:** Send keyboard input to PTY  
**Key Fields:**
- `data` (bytes): Raw input bytes (UTF-8)
- `client_id` (string, Phase 2+): Who sent this input (audit trail)
- `timestamp_ms` (uint64, Phase 2+): Client timestamp (latency tracking)

**Size:** 10-200 bytes (single keypress to pasted paragraph)  
**Compression:** No (latency-critical, user-facing)

---

#### OutputData (field 5)
**Purpose:** Terminal output chunk from PTY  
**Key Fields:**
- `data` (bytes): Raw output bytes (UTF-8)
- `timestamp_ms` (uint64): Server timestamp
- `final` (bool): Last chunk of stream (PTY exited)

**Size:** 512B-64KB (adaptive chunking, SRS §3.1.2)  
**Compression:** **YES** if >4KB (zstd level 1, 3-5× ratio)

**Bandwidth impact:**
- Uncompressed: 4KB OutputData → 4096 bytes on wire
- Compressed: 4KB OutputData → 800-1200 bytes (60-75% reduction)
- TURN relay savings: 75% bandwidth reduction (reduces VPS costs)

---

#### ResizeRequest (field 6)
**Purpose:** Notify PTY of terminal window resize  
**Key Fields:**
- `rows`, `cols` (uint32): New terminal dimensions

**Size:** 50 bytes  
**Compression:** No

---

#### DetachRequest (field 7)
**Purpose:** Gracefully disconnect from session (keep session alive in DETACHED state)  
**Size:** 30 bytes  
**Compression:** No

---

#### ErrorResponse (field 8)
**Purpose:** Error notification (auth failed, session not found, etc.)  
**Key Fields:**
- `code` (ErrorCode enum): Error type (see §Error Codes below)
- `message` (string): Human-readable error description

**Size:** 100-500 bytes  
**Compression:** No

---

#### DashboardRequest (field 9)
**Purpose:** Request monomind org dashboard data  
**Size:** 50 bytes  
**Compression:** No

---

#### DashboardResponse (field 10)
**Purpose:** Monomind org status, agents, tasks, knowledge graph stats  
**Key Fields:**
- `org_name`, `org_status` (string): Org identity
- `agents` (repeated AgentInfo): Active agents, status, uptime
- `tasks` (repeated TaskInfo): Task queue, dependencies, progress
- `kg_stats` (KnowledgeGraphStats): Node count, edge count, last update

**Size:** 1-5 KB (small org), 10-50 KB (large org with 100+ agents)  
**Compression:** **YES** if >4KB (dashboard data is structured, compresses well)

---

#### HealthCheckRequest/Response (fields 11-12)
**Purpose:** Verify monomind CLI is installed, functional, up-to-date  
**Size:** Request 30B, Response 200B-2KB  
**Compression:** No

---

#### UpgradeRequest/Response (fields 13-14)
**Purpose:** Trigger monomind CLI upgrade via `npx monomind@latest upgrade`  
**Size:** Request 50B, Response 100-500B  
**Compression:** No

---

#### DetectionRequest/Response (fields 15-16)
**Purpose:** Detect monomind installation (first-run check)  
**Size:** Request 30B, Response 100-300B  
**Compression:** No

---

#### MonitoringData (field 17)
**Purpose:** Periodic stats broadcast (every 60s) for embedded dashboard  
**Size:** 500B-2KB  
**Compression:** No (periodic telemetry, low volume)

---

### Phase 2: P2P, Collaboration, Persistence

#### ScrollbackFetchRequest (field 18)
**Purpose:** Paginate scrollback history (fetch older lines beyond initial 100)  
**Key Fields:**
- `session_id` (string): Session UUID
- `cursor` (string): Opaque cursor from `AttachResponse.scrollback_cursor`
- `limit` (uint32): Max lines to return (default 1000, max 5000)
- `compression` (CompressionType, optional): Request zstd compression

**Size:** 100 bytes  
**Compression:** No (request message, small)

---

#### ScrollbackFetchResponse (field 19)
**Purpose:** Return scrollback page (1000-5000 lines)  
**Key Fields:**
- `lines` (repeated Line): Scrollback page
- `next_cursor` (string, optional): Cursor for next page (null if EOF)
- `has_more` (bool): More data available
- `compression` (CompressionType): Actual compression used
- `total_bytes` (uint32): Uncompressed size (for UI progress bar)

**Size:** 50KB-500KB (1000 lines × 50-500 bytes/line)  
**Compression:** **YES** if >4KB (60-80% reduction for log data)

**Use case:** User scrolls to top of terminal, requests history from disk (SQLite cold tier)

---

#### ListSessionsRequest/Response (fields 20-21)
**Purpose:** Discover active sessions (filtered by RBAC permissions)  
**Request Fields:**
- `auth_token` (string, optional): JWT for user identification

**Response Fields:**
- `sessions` (repeated SessionSummary): Session metadata, client count, activity status

**Size:** Request 50B, Response 500B-5KB (10-100 sessions)  
**Compression:** No (session discovery, low frequency)

---

#### ClientHeartbeat (field 22)
**Purpose:** Keep-alive ping from client to master (30-second interval)  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `timestamp_ms` (uint64): Client timestamp (for latency calc)

**Size:** 80 bytes  
**Compression:** No (frequent, small, latency-sensitive)

**Eviction policy:** No heartbeat for 120 seconds → client evicted, `PresenceUpdate(HEARTBEAT_TIMEOUT)` broadcast

---

#### PresenceUpdate (field 23)
**Purpose:** Broadcast client join/leave/focus change to all attached clients  
**Key Fields:**
- `session_id` (string): Which session
- `clients` (repeated ClientPresence): Full current client list
- `event_type` (PresenceEventType enum): CLIENT_JOINED, CLIENT_LEFT, HEARTBEAT_TIMEOUT, FOCUS_CHANGED
- `affected_client_id` (string, optional): Which client triggered this update (for UI diff)

**Size:** 200B-1KB (1-10 clients × 100-200B each)  
**Compression:** No (presence updates are time-sensitive, low volume)

---

#### InputFocusUpdate (field 24)
**Purpose:** Notify master that client gained/lost input focus (typing indicator)  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `is_active` (bool): true = focused, false = blurred

**Size:** 60 bytes  
**Compression:** No (low frequency, user-driven)

---

#### WebRTCOffer (field 25)
**Purpose:** Initiate WebRTC P2P connection (SDP offer)  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `sdp` (string): SDP offer (WebRTC session description)
- `peer_id` (string): Client's Ed25519 public key (hex)
- `nonce` (uint64): Single-use nonce from `PeerHandshakeResponse`

**Size:** 1-2 KB (SDP is verbose)  
**Compression:** No (signaling is infrequent, latency-sensitive)

---

#### WebRTCAnswer (field 26)
**Purpose:** Respond to WebRTC offer (SDP answer + TURN credentials)  
**Key Fields:**
- `sdp` (string): SDP answer
- `turn` (TURNCredentials, optional): TURN relay credentials (15-minute TTL)
- `offer_timestamp_ms` (uint64): Echo from offer (latency calc)

**Size:** 1-2 KB  
**Compression:** No

**TURN credentials structure:**
- `urls` (repeated string): `["turn:turn.monoterminal.io:3478", "turns:turn.monoterminal.io:5349"]`
- `username`, `credential` (string): Time-limited HMAC-SHA256 credentials
- `expires_at_ms` (uint64): Credential expiry timestamp

---

#### ICECandidate (field 27)
**Purpose:** Trickle ICE candidate exchange (NAT traversal)  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `candidate` (string): ICE candidate string (IP:port)
- `sdp_mid`, `sdp_mline_index` (optional): SDP media stream identifiers

**Size:** 100-300 bytes  
**Compression:** No (frequent during negotiation, small)

**Flow:** Client and master exchange 5-20 ICE candidates over 2-10 seconds (STUN + TURN discovery)

---

#### P2PConnectionStatus (field 28)
**Purpose:** Notify client of WebRTC connection state change  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `state` (P2PState enum): NEGOTIATING, CONNECTED, FAILED, DISCONNECTED
- `error_message` (string, optional): Failure reason

**Size:** 100 bytes  
**Compression:** No

**States:**
- `NEGOTIATING`: Exchanging SDP/ICE
- `CONNECTED`: DataChannel open, P2P active
- `FAILED`: NAT traversal failed → fallback to WebSocket
- `DISCONNECTED`: Connection lost (mobile backgrounding) → reconnecting

---

#### PeerHandshake (field 29)
**Purpose:** Authenticate peer before WebRTC negotiation (Ed25519 challenge-response)  
**Key Fields:**
- `session_id`, `client_id` (string): Session + client identity
- `peer_id` (string): Client's Ed25519 public key (hex)
- `challenge_response` (bytes): Ed25519 signature of challenge
- `timestamp_ms` (uint64): Request timestamp (replay attack prevention)

**Size:** 200 bytes  
**Compression:** No

**Flow:**
1. Client → Master: `PeerHandshake` (initial, no challenge_response)
2. Master → Client: `PeerHandshakeResponse` (challenge = 32 random bytes)
3. Client → Master: `PeerHandshake` (challenge_response = Ed25519.sign(challenge))
4. Master → Client: `PeerHandshakeResponse` (accepted=true, nonce for WebRTCOffer)

---

#### PeerHandshakeResponse (field 30)
**Purpose:** Return challenge or nonce (P2P auth flow)  
**Key Fields:**
- `accepted` (bool): Authentication successful
- `error_message` (string, optional): Rejection reason
- `challenge` (bytes, optional): 32-byte challenge (initial handshake)
- `nonce` (uint64): Single-use nonce for WebRTCOffer (60-second TTL)

**Size:** 150 bytes  
**Compression:** No

---

## Error Codes

### Phase 1 Error Codes
| Code | Name | Description |
|------|------|-------------|
| 0 | `UNKNOWN` | Unspecified error |
| 1 | `SESSION_NOT_FOUND` | Session ID does not exist |
| 2 | `AUTH_FAILED` | JWT invalid, Ed25519 signature verification failed |
| 3 | `PERMISSION_DENIED` | RBAC: user lacks required role (viewer tried to send input) |
| 4 | `RATE_LIMIT_EXCEEDED` | Too many requests (>100 connections/min) |
| 5 | `INVALID_REQUEST` | Malformed message, missing required fields |
| 6 | `SERVER_ERROR` | Internal server error (panic, database failure) |

### Phase 2 Error Codes
| Code | Name | Description |
|------|------|-------------|
| 7 | `SESSION_FULL` | Max clients attached (default: 50/session) |
| 8 | `PROTOCOL_VERSION_MISMATCH` | Client too old/new, upgrade required |
| 9 | `SCROLLBACK_CURSOR_INVALID` | Cursor tampered or expired |
| 10 | `P2P_NEGOTIATION_FAILED` | WebRTC handshake failed (ICE timeout, TURN unavailable) |
| 11 | `HEARTBEAT_TIMEOUT` | Client evicted (no heartbeat for 120s) |
| 12 | `CHALLENGE_EXPIRED` | PeerHandshake challenge >60s old |
| 14 | `NONCE_INVALID` | Nonce not found, expired, or already consumed |

---

## Compression Strategy Summary

**Compress (zstd level 1):**
- `OutputData` (field 5) if >4KB
- `ScrollbackFetchResponse` (field 19) if >4KB
- `DashboardResponse` (field 10) if >4KB

**Skip compression:**
- All other messages (<4KB typical size, or latency-sensitive)

**Rationale:**
- Compression threshold: 4KB (below this, compression overhead > bandwidth savings)
- zstd level 1: 100-200 MB/s encode, <5ms latency for 64KB chunks
- Compression ratio: 3-5× for OutputData (logs, build output), 2-3× for structured data (DashboardResponse)

---

## Message Flow Examples

### Example 1: Client Attach (Phase 2)
```
Client → Master: AttachRequest (protocol_version=2, session_id="abc123")
Master → Client: AttachResponse (scrollback last 100 lines, cursor="xyz", attached_clients=[...])
Client → Master: ScrollbackFetchRequest (cursor="xyz", limit=1000)
Master → Client: ScrollbackFetchResponse (1000 lines, next_cursor="def456")
Master → All Clients: PresenceUpdate (CLIENT_JOINED, affected_client_id="client-2")
```

### Example 2: WebRTC P2P Connection Establishment
```
# Phase 1: Peer Authentication
Client → Master: PeerHandshake (peer_id="ed25519:abcd...", no challenge_response)
Master → Client: PeerHandshakeResponse (challenge=<32 random bytes>)
Client → Master: PeerHandshake (challenge_response=Ed25519.sign(challenge))
Master → Client: PeerHandshakeResponse (accepted=true, nonce=123456)

# Phase 2: WebRTC Signaling
Client → Master: WebRTCOffer (sdp="v=0\r\no=...", nonce=123456)
Master → Client: WebRTCAnswer (sdp="v=0\r\na=...", turn=TURNCredentials{...})
Client ↔ Master: ICECandidate × 5-20 (trickle ICE)

# Phase 3: P2P Active
Master → Client: P2PConnectionStatus (state=CONNECTED)
[WebRTC DataChannel established, terminal I/O flows over P2P]
```

### Example 3: Multi-Client Collaboration
```
# Client 1 attaches
Client 1 → Master: AttachRequest (session_id="abc123")
Master → Client 1: AttachResponse (attached_clients=[])

# Client 2 joins
Client 2 → Master: AttachRequest (session_id="abc123")
Master → Client 2: AttachResponse (attached_clients=[Client 1])
Master → Client 1: PresenceUpdate (clients=[Client 1, Client 2], event_type=CLIENT_JOINED)

# Client 1 types
Client 1 → Master: InputData (data="echo hello\n", client_id="client-1")
Master → PTY: Write "echo hello\n"
PTY → Master: Output "hello\n"
Master → All Clients: OutputData (data="hello\n")
```

---

## Wire Format Overhead

**Envelope structure:**
```
[sequence_number: 8B] [message type tag: 1-2B] [message payload: variable] = 10-20B overhead/message
```

**Typical message sizes (on wire):**
- Small control: 50-100B (ResizeRequest, DetachRequest)
- Medium I/O: 500B-4KB (InputData, uncompressed OutputData)
- Large data: 50KB-500KB (ScrollbackFetchResponse, large DashboardResponse)

**Bandwidth estimate (100 clients, 1000 sessions):**
- Heartbeats: 100 clients × 80B × 2/min = 16 KB/min = 266 B/s
- OutputData (average): 100 clients × 2KB/s = 200 KB/s (uncompressed), 50-80 KB/s (compressed)
- Presence updates: ~1 KB/min (low frequency)

**Total steady-state bandwidth:** ~100 KB/s (compressed), manageable on $5/month VPS

---

## References

- **SRS §3.1.1:** Protocol Buffers Schema Evolution
- **SRS §3.1.2:** Message Chunking (OutputData adaptive chunking)
- **SRS §3.1.3:** Compression Strategy (zstd for >4KB messages)
- **ADR-004:** Protocol Schema Evolution Policy
- **ADR-011:** P2P Networking Architecture (WebRTC signaling messages)
- **ADR-013:** Multi-Session Architecture (session discovery, presence)
- **ADR-014:** Collaboration Primitives (RBAC, heartbeat, input focus)
- **protocol-phase2-design.md:** Detailed message schemas, Q&A

---

**Status:** Complete and ready for implementation
