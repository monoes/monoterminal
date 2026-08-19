# ADR-004: Protocol Schema Evolution Policy

**Status:** Draft → Pending Approval  
**Date:** 2026-08-15  
**Deciders:** principal-architect, rust-engineer-protocol  
**SRS Reference:** §3.1.1 (Protocol Buffers Schema Evolution)

---

## Context

MONOTERMINAL wire protocol (SRS §3.1.1) uses Protocol Buffers for all client-server and P2P communication. As features evolve across phases (Phase 1 → Phase 2 P2P, Phase 3+ enterprise), the protocol schema MUST support:

- **Backward compatibility**: New servers must accept messages from old clients
- **Forward compatibility**: Old servers should gracefully handle messages from new clients (or fail safely)
- **Version skew tolerance**: Mixed client/server versions must interoperate during rollouts

**Current state (Phase 1):**
- Single protocol file: `proto/monoterminal/v1/messages.proto`
- No version field in `Envelope` (only `sequence_number`)
- No documented evolution rules

**Trigger for this ADR:**
- Architecture review (2026-08-15) identified protocol schema duplication (`envelope.proto` vs `messages.proto`)
- Phase 2 P2P networking will introduce version skew scenarios (clients connect to each other, not just servers)

---

## Decision

Adopt **Protocol Buffers v3 evolution rules** with the following MONOTERMINAL-specific policies:

### 1. Schema Organization

**Single source of truth:**
- `proto/monoterminal/v1/messages.proto` is the ONLY protocol definition file
- `proto/envelope.proto` (obsolete prototype) → **DELETED**
- All future message types added to `v1/messages.proto` until breaking change required

**Versioning:**
- Protocol version lives in **package name**: `monoterminal.v1`, `monoterminal.v2`, etc.
- `Envelope` includes `protocol_version` field (uint32) for runtime negotiation
- File paths follow package: `proto/monoterminal/v1/`, `proto/monoterminal/v2/`, etc.

---

### 2. Safe Additions (Non-Breaking Changes)

**Allowed without version bump:**

✅ **Add new optional fields** (protobuf3 default: all fields optional)
```protobuf
message Envelope {
  uint64 sequence_number = 1;
  uint32 protocol_version = 28;  // NEW FIELD (added in v1.1)
  oneof message { ... }
}
```

✅ **Add new message types to `oneof`**
```protobuf
oneof message {
  // ... existing messages ...
  NewFeatureRequest new_feature_request = 30;  // NEW MESSAGE TYPE
  NewFeatureResponse new_feature_response = 31;
}
```

✅ **Add new enum values**
```protobuf
enum ErrorCode {
  UNKNOWN = 0;
  SESSION_NOT_FOUND = 1;
  // ... existing codes ...
  QUOTA_EXCEEDED = 10;  // NEW ERROR CODE
}
```

**Compatibility guarantee:**
- Old clients ignore unknown fields (protobuf3 default behavior)
- Old servers ignore unknown message types (log warning, send `ErrorResponse`)
- Old code treats unknown enum values as `UNKNOWN` (protobuf3 default)

---

### 2.4 Compression Support (Phase 2)

**Added:** 2026-08-15 (Phase 2 architectural review)

Phase 2 requires zstd compression (SRS §7.2) for bandwidth reduction on P2P connections. Compression metadata lives in the `Envelope` as a self-describing field.

**Envelope field:**

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  uint32 protocol_version = 28;
  CompressionType compression = 29;  // Added Phase 2, default NONE
  oneof message { ... }
}

enum CompressionType {
  NONE = 0;      // No compression (default, v1.0 compatible)
  ZSTD = 1;      // Zstandard compression (Phase 2+)
  // Reserved: 2-15 for future algorithms (LZ4, Brotli, etc.)
}
```

**Compression logic:**

1. **Sender:** Compress `oneof message` bytes ONLY (leave `sequence_number`, `protocol_version`, `compression` uncompressed)
2. **Receiver:** If `compression == ZSTD`, decompress `oneof message` bytes before protobuf deserialization
3. **Backward compatibility:** v1.0 clients ignore `compression` field (protobuf3 default), receive uncompressed messages

**Size threshold:** Only compress messages ≥ 1KB
- **Rationale:** Compression overhead (CPU + zstd frame header) not worth it for small messages
- **Example skip:** `InputData` (10-50B), `CursorPosition` (50B), `ResizeRequest` (80B)
- **Example compress:** `OutputData` (4KB chunk: 4096B → 800-1200B, 3-5× ratio), `AttachResponse` scrollback (10KB-500KB)

**Compatibility:**
- ✅ Non-breaking addition (ADR-004 §2): old clients ignore field, default to `NONE`
- ✅ New clients detect old servers (via `protocol_version` downgrade), send uncompressed
- ✅ Per-message control: compress large `OutputData`, skip small `InputData`

**Overhead:** 1 byte per message (acceptable)

---

### 2.5 Field Number Reservation

**Added:** 2026-08-15 (Phase 2 architectural review)

Protobuf field numbers are immutable once assigned (changing breaks wire compatibility). This section documents MONOTERMINAL's reservation strategy to prevent collisions.

**Envelope field number allocation:**

| Range | Purpose | Current Usage | Future Expansion |
|-------|---------|---------------|------------------|
| **1** | Sequence tracking | `sequence_number = 1` (v1.0) | — (frozen) |
| **2-17** | Phase 1 message types | `attach_request = 2`, `output_data = 5`, `dashboard_request = 9`, etc. (16 Phase 1 types) | — (Phase 1 complete) |
| **18-30** | Phase 2 message types | `scrollback_fetch_request = 18`, `presence_update = 23`, `webrtc_offer = 25`, `peer_handshake = 29`, etc. (13 Phase 2 types) | Phase 2+ expansion up to field 39 |
| **28-29** | Envelope metadata fields | `protocol_version = 28` (v1.1), `compression = 29` (Phase 2) | — (coexist with message range) |
| **40-2047** | Unreserved | — | Long-term expansion (Phase 3+) |

**Rationale for field 28 (`protocol_version`):**
- Chosen to be high enough (28) to avoid collision with Phase 1 (2-17) and Phase 2 (18-30) message types
- Metadata fields (28-29) coexist within the message range without conflict
- Future metadata can use fields 31-39 before exhausting the practical range

**Rules:**
1. **Never reuse a field number** — even if a field is deprecated, leave its number permanently reserved
2. **Document exceptions** — if a field is used outside its designated range, explain in `PROTOCOL_CHANGELOG.md`
3. **Reserve in blocks** — when approaching range exhaustion (e.g., 18-30 at 90% full), open 31-50 for Phase 3 message expansion

**Phase 2 Impact:**
- Phase 2 message types use fields 18-30 (13 new types: scrollback, presence, WebRTC, P2P handshake)
- Based on protocol-phase2-design.md (rust-engineer-protocol, 2026-08-15)
- Leaves fields 31-39 available for Phase 2+ additions or Phase 3 features
- Metadata fields 28-29 are "exceptions" embedded in the message range (documented here)

---

### 3. Breaking Changes (Require Major Version Bump)

**Require new package `monoterminal.v2`:**

❌ **Change field number or type**
```protobuf
// BREAKING: field 1 changed from uint64 to string
message Envelope {
  string sequence_number = 1;  // ❌ BREAKING
}
```

❌ **Remove fields or message types**
```protobuf
// BREAKING: removed attach_request from oneof
oneof message {
  // attach_request = 2;  ❌ BREAKING (removed)
  AttachResponse attach_response = 3;
}
```

❌ **Make optional field required** (not possible in proto3, but don't add validation that treats absence as error)

❌ **Rename package**
```protobuf
package monoterminal.v2;  // ❌ BREAKING (old clients expect v1)
```

**Migration path for breaking changes:**
1. Create new package: `proto/monoterminal/v2/messages.proto`
2. Duplicate definitions, make breaking change
3. Master daemon supports BOTH `v1` and `v2` protocols during transition (6-12 months)
4. Deprecate `v1` after 95%+ clients upgraded

---

### 4. Version Negotiation Protocol

**Envelope must include version field:**

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  uint32 protocol_version = 28;  // 0 = v1.0, 1 = v1.1, 2 = v1.2, etc.
  oneof message { ... }
}
```

**Handshake flow (AttachRequest):**

```
Client                          Server
  │                               │
  ├─ AttachRequest ──────────────►│
  │  (protocol_version = 2)       │
  │                               │
  │◄────────── AttachResponse ────┤
  │  (protocol_version = 1)       │ Server downgrades to min(2, 1) = 1
  │                               │
  │    All messages use v1        │
```

**Rejection flow (version too old):**

```
Client                          Server
  │                               │
  ├─ AttachRequest ──────────────►│
  │  (protocol_version = 1)       │
  │                               │ Server requires v2+
  │◄────────── ErrorResponse ─────┤
  │  code=INCOMPATIBLE_VERSION    │
  │  message="Upgrade client to v2"
```

**Version compatibility matrix:**

| Client | Server v1.0 | Server v1.1 | Server v2.0 |
|--------|-------------|-------------|-------------|
| v1.0   | ✅ Full      | ✅ Downgrade | ❌ Reject    |
| v1.1   | ✅ Downgrade | ✅ Full      | ❌ Reject    |
| v2.0   | ❌ Reject    | ❌ Reject    | ✅ Full      |

---

### 4.3 P2P Version Negotiation (Phase 2)

**Added:** 2026-08-15 (Phase 2 architectural amendment)

Phase 2 introduces **peer-to-peer WebRTC DataChannel connections** (SRS §2.3.1), where clients connect directly to each other, not just to the master daemon. P2P connections require independent version negotiation since either peer may initiate the handshake.

**Context:**
- **Client-Server** (§4 above): Server downgrades to `min(client_version, server_version)`
- **Peer-to-Peer** (this section): Symmetric negotiation—both peers must agree on the same protocol version

---

#### P2P Handshake Flow

```
Peer A (initiator)              Peer B (responder)
    │                               │
    │  [WebRTC DataChannel established via WebSocket signaling]
    │                               │
    ├─ PeerHandshake ──────────────►│  (protocol_version = 2)
    │                               │
    │◄──── PeerHandshakeAck ────────┤  (protocol_version = min(2, 1) = 1)
    │                               │
    │    Both use version 1         │
```

**Who initiates:** The peer that sent the `WebRTCOffer` (SDP offer) over the WebSocket connection initiates the `PeerHandshake` message once the DataChannel is open.

**Message sequence:**
1. **WebSocket phase**: Peers exchange `WebRTCOffer`, `WebRTCAnswer`, `ICECandidate` over existing WebSocket connection (fields 25-27 in design doc)
2. **DataChannel opens**: DTLS encryption established, binary channel ready
3. **P2P handshake phase**: Initiator sends `PeerHandshake` as first message over DataChannel
4. Responder computes `negotiated_version = min(initiator_version, responder_version)`
5. Responder sends `PeerHandshakeAck` with negotiated version
6. **Both peers use negotiated version** for all subsequent DataChannel messages

---

#### New Message Types

Add to `Envelope.oneof message`:

```protobuf
message PeerHandshake {
  uint32 protocol_version = 1;  // This peer's supported protocol version
  string peer_id = 2;            // Ed25519 public key (hex), matches WebRTCOffer.peer_id
  bytes signature = 3;           // Ed25519 signature over (protocol_version || peer_id || timestamp)
  uint64 timestamp_ms = 4;       // Unix timestamp (prevents replay attacks)
}

message PeerHandshakeAck {
  uint32 protocol_version = 1;  // Negotiated version = min(initiator, responder)
  string peer_id = 2;            // Responder's Ed25519 public key
  bytes signature = 3;           // Ed25519 signature over (protocol_version || peer_id || timestamp)
  uint64 timestamp_ms = 4;
}
```

**Field number assignment:**

```protobuf
oneof message {
  // ... Phase 1 messages (2-17) ...
  // ... Phase 2 WebSocket messages (18-28, per protocol-phase2-design.md) ...
  
  // Phase 2 P2P DataChannel messages
  PeerHandshake peer_handshake = 29;
  PeerHandshakeAck peer_handshake_ack = 30;
}
```

---

#### Negotiation Algorithm

**Initiator (Peer A):**
```rust
fn initiate_p2p_handshake(datachannel: &DataChannel, my_version: u32) {
    let handshake = PeerHandshake {
        protocol_version: my_version,  // e.g., 2 (v1.1)
        peer_id: my_ed25519_pubkey.to_hex(),
        signature: sign_handshake(my_version, my_peer_id, timestamp),
        timestamp_ms: now_millis(),
    };
    datachannel.send(encode_envelope(handshake));
}
```

**Responder (Peer B):**
```rust
fn handle_peer_handshake(msg: PeerHandshake, my_version: u32) -> Result<u32> {
    // Verify signature prevents MITM
    verify_signature(msg.peer_id, msg.signature, msg.protocol_version)?;
    
    // Compute negotiated version
    let negotiated = min(msg.protocol_version, my_version);  // e.g., min(2, 1) = 1
    
    // Reject if negotiated version below minimum supported
    if negotiated < MIN_SUPPORTED_VERSION {
        return Err("Incompatible protocol version");
    }
    
    let ack = PeerHandshakeAck {
        protocol_version: negotiated,
        peer_id: my_ed25519_pubkey.to_hex(),
        signature: sign_handshake_ack(negotiated, my_peer_id, timestamp),
        timestamp_ms: now_millis(),
    };
    datachannel.send(encode_envelope(ack));
    Ok(negotiated)
}
```

---

#### Incompatible Version Handling

**Scenario:** Peer A requires v2+, Peer B only supports v1

```
Peer A (min_version=2)          Peer B (max_version=1)
    │                               │
    ├─ PeerHandshake ──────────────►│  (protocol_version = 2)
    │                               │  negotiated = min(2, 1) = 1
    │                               │  1 < min_supported=2 → REJECT
    │◄────────── ErrorResponse ─────┤
    │  code=PROTOCOL_VERSION_MISMATCH│
    │  "Peer requires v2+, you have v1"
    │                               │
    └── Close DataChannel ──────────┘
```

**Error handling:**
- Responder sends `ErrorResponse(PROTOCOL_VERSION_MISMATCH)` over DataChannel
- Both peers close DataChannel gracefully
- Client falls back to WebSocket connection (always available as baseline)
- UI shows notification: "Direct P2P unavailable (version mismatch), using server relay"

---

#### Security Considerations

**Ed25519 Signature Verification:**
- `signature` field prevents MITM attacks during WebRTC negotiation
- Each peer verifies the other's signature before accepting the handshake
- `timestamp_ms` prevents replay attacks (reject if |now - timestamp| > 30 seconds)

**Why sign the handshake:**
- WebRTC DTLS provides encryption but not peer identity verification
- STUN/TURN traversal can expose IP addresses to malicious signaling servers
- Ed25519 signature binds `protocol_version` to the peer's cryptographic identity
- Matches existing auth model (SRS §3.2: Ed25519 SSH keys + JWT)

**Signature payload:**
```rust
let payload = format!(
    "MONOTERMINAL-P2P-HANDSHAKE:{}:{}:{}",
    protocol_version,
    peer_id,
    timestamp_ms
);
let signature = ed25519_sign(keypair, payload.as_bytes());
```

---

#### Backward Compatibility

**Phase 1 Clients (no P2P support):**
- Never establish WebRTC DataChannel (WebSocket-only)
- `PeerHandshake` message never sent (no P2P connections)
- No compatibility issues (P2P is opt-in via client capability negotiation)

**Phase 2 Clients with P2P:**
- Must implement `PeerHandshake`/`PeerHandshakeAck` to use DataChannel
- Graceful degradation: If handshake fails → close DataChannel, continue via WebSocket
- Master daemon doesn't participate in P2P handshake (relay-only during signaling phase)

---

#### Version Compatibility Matrix (P2P)

| Initiator | Responder v1.0 | Responder v1.1 | Responder v2.0 |
|-----------|----------------|----------------|----------------|
| v1.0      | ✅ Full (v1.0)  | ✅ Downgrade (v1.0) | ❌ Reject      |
| v1.1      | ✅ Downgrade (v1.0) | ✅ Full (v1.1)  | ❌ Reject      |
| v2.0      | ❌ Reject      | ❌ Reject      | ✅ Full (v2.0)  |

**Note:** Matrix assumes v2.0 is a breaking change (new package `monoterminal.v2`). Within the same major version (v1.x), negotiation always succeeds with the lower version.

---

#### Testing Requirements

Add to ADR-004 §5 test suite:

```rust
#[test]
fn test_p2p_version_negotiation_downgrade() {
    // Initiator v1.1, Responder v1.0 → negotiate v1.0
    let initiator_handshake = PeerHandshake { protocol_version: 1, .. };
    let responder_version = 0;
    let ack = handle_peer_handshake(initiator_handshake, responder_version);
    assert_eq!(ack.protocol_version, 0); // min(1, 0) = 0
}

#[test]
fn test_p2p_incompatible_version_rejected() {
    // Initiator v2.0, Responder v1.1, min_supported=2 → reject
    let initiator_handshake = PeerHandshake { protocol_version: 2, .. };
    let responder_version = 1;
    let result = handle_peer_handshake(initiator_handshake, responder_version);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ErrorCode::PROTOCOL_VERSION_MISMATCH);
}

#[test]
fn test_p2p_signature_verification() {
    // Invalid signature → reject handshake
    let mut handshake = PeerHandshake { protocol_version: 1, .. };
    handshake.signature = vec![0u8; 64]; // Invalid signature
    let result = verify_peer_handshake(&handshake);
    assert!(result.is_err());
}

#[test]
fn test_p2p_replay_attack_prevention() {
    // Timestamp >30s old → reject
    let old_timestamp = now_millis() - 60_000; // 60 seconds ago
    let handshake = PeerHandshake { timestamp_ms: old_timestamp, .. };
    let result = verify_peer_handshake(&handshake);
    assert!(result.is_err());
}
```

---

#### Integration with WebSocket Protocol

**Signaling phase** (over WebSocket, using messages from protocol-phase2-design.md):

1. Client A → Master: `WebRTCOffer(sdp, peer_id)` [field 25]
2. Master → Client A: `WebRTCAnswer(sdp, turn_credentials)` [field 26]
3. Client A ↔ Master: `ICECandidate` exchange [field 27]
4. **WebRTC DataChannel opens** (DTLS encrypted, binary transport)

**P2P phase** (over DataChannel, using new messages):

5. Initiator → Responder: `PeerHandshake` [field 29] — **FIRST MESSAGE over DataChannel**
6. Responder → Initiator: `PeerHandshakeAck` [field 30]
7. Both peers use negotiated version for all subsequent DataChannel messages

**Fallback to WebSocket:**
- If DataChannel fails to open → clients continue using existing WebSocket connection
- If `PeerHandshake` fails (version mismatch) → close DataChannel, continue via WebSocket
- Master daemon always relays messages for clients without P2P (backward compatibility)

---

#### Implementation Notes

**Field Number Allocation:**
- `PeerHandshake` = 29, `PeerHandshakeAck` = 30
- Follows Phase 2 WebSocket messages (18-28, per protocol-phase2-design.md §6.1)
- Keeps P2P messages grouped separately from client↔master messages

**When to implement:**
- Phase 2 P2P implementation (Month 4-6, SRS §7.2)
- After WebRTC signaling infrastructure complete (networking-engineer)
- Before P2P collaboration features ship (multi-client session sharing)

**Coordination:**
- rust-engineer-protocol: Add message types to `messages.proto`, signature verification logic
- networking-engineer: WebRTC DataChannel lifecycle, handshake state machine
- security-engineer: Ed25519 signature scheme, replay attack prevention

---

**Impact:** Non-breaking addition (ADR-004 §2) — P2P handshake is new functionality, no impact on existing WebSocket-only clients.

---

### 4.4 Design Principle: Structured Fields Over Opaque JSON (Pre-Release Revision)

**Added:** 2026-08-16 (Protocol v1.0 pre-release design revision)

**Context:**

During monomind integration implementation (tasks 15, 17, 19, 21), an architectural question arose: Should protocol messages like `DashboardResponse` use structured fields (`org_name`, `agents[]`, etc.) or opaque JSON strings (`json_data: string`)?

**Original v1.0 design** (protocol-changelog.md, 2026-08-15):
```protobuf
message DashboardResponse {
  string json_data = 1;  // JSON response from monomind CLI
  ErrorCode error = 2;
}
```

**Revised v1.0 design** (messages.proto, 2026-08-16):
```protobuf
message DashboardResponse {
  string org_name = 1;
  string org_status = 2;
  repeated AgentInfo agents = 3;
  repeated TaskInfo tasks = 4;
  KnowledgeGraphStats kg_stats = 5;
  int64 timestamp = 6;
}
```

---

#### Decision: Always Use Structured Fields

**Rule:** Protocol messages MUST use structured protobuf fields, not opaque JSON/string blobs.

**Rationale:**

1. **Type Safety:**
   - Frontend TypeScript interfaces match protobuf schema 1:1
   - Compile-time verification catches field mismatches
   - No runtime JSON parsing errors

2. **Performance:**
   - No JSON serialization/deserialization overhead
   - Smaller wire size (protobuf binary < JSON text)
   - Direct memory mapping (zero-copy in some cases)

3. **Schema Evolution:**
   - Protobuf field evolution rules apply (add fields backward-compatibly)
   - JSON blobs are opaque — schema changes break silently
   - Version negotiation works at field level, not blob level

4. **Consistency:**
   - Matches existing pattern in `MonitoringData` message (SRS §2.4.2)
   - All Phase 1 messages use structured fields
   - No "mixed mode" protocol design

**Counter-Example (Why NOT JSON):**

```protobuf
// ❌ ANTI-PATTERN: Opaque JSON blob
message DashboardResponse {
  string json_data = 1;  // BAD: Schema hidden inside string
  // - Frontend must parse JSON manually
  // - No type checking
  // - Can't add fields incrementally (blob is all-or-nothing)
  // - Version skew: new server fields → old client JSON parser crashes
}
```

---

#### When JSON Is Acceptable (Exceptions)

**Rule:** Opaque JSON is allowed ONLY when:
1. **External API pass-through** (e.g., raw GitHub API response, temporary debugging)
2. **User-supplied data** (e.g., arbitrary metadata, plugin config)
3. **Pre-release prototyping** (to be replaced with structured fields before v1.0 ships)

**Never use JSON for:**
- Core protocol types (session, terminal I/O, authentication)
- Inter-service communication (daemon ↔ client)
- Data that needs schema evolution

---

#### Backward Compatibility Impact

**Status:** No backward compatibility concern (pre-release change)

- v1.0 has NOT shipped to production (Phase 1 MVP in development)
- No deployed clients expect `json_data` field
- This is a **design revision**, not a breaking change

**If this were post-release:**
- Would require new major version (v2.0)
- OR add structured fields alongside `json_data` (deprecated)
- OR version negotiation to switch representation

---

#### Implementation Evidence

**Conversion Layer:**
- File: `crates/monomind-bridge/src/responses.rs`
- Functions: `to_dashboard_response()`, `to_health_check_response()`
- Pattern: Internal types → Protobuf types (no JSON intermediate step)

**Test Coverage:**
- 7 unit tests covering conversion logic, edge cases, empty states
- Example: `test_to_dashboard_response_empty()` verifies default values

**Consistency Check:**
```rust
// ✅ CORRECT: MonitoringData uses structured fields (existing)
message MonitoringData {
  string org_name = 1;
  int32 active_agents = 2;
  repeated RunSummary recent_runs = 7;  // NOT json_data
}

// ✅ CORRECT: DashboardResponse matches pattern (revised)
message DashboardResponse {
  string org_name = 1;
  repeated AgentInfo agents = 3;
  KnowledgeGraphStats kg_stats = 5;  // NOT json_data
}
```

---

#### Future Guidance

**For every new protocol message:**

1. **Default to structured fields**
   - Break complex types into sub-messages (`AgentInfo`, `TaskInfo`)
   - Use `repeated` for lists, `map` for key-value pairs
   - Use protobuf enums for string constants ("running"|"stopped")

2. **Only use JSON if:**
   - Data is genuinely unstructured (user metadata, plugin config)
   - Pass-through from external API (temporary, to be normalized later)
   - Document why JSON is necessary (ADR or code comment)

3. **Migration path:**
   - If prototyping with JSON, add TODO: Replace with structured fields before v1.0
   - Track in protocol changelog: "Planned: Migrate `foo_data` JSON → `FooData` message"

---

**Approval:** principal-architect (architectural principle)  
**Impact:** Non-breaking (pre-release revision), sets precedent for all future messages  
**Referenced by:** PROTOCOL_CHANGELOG.md v1.0 (DashboardResponse description)

---

### 5. Testing Requirements

**Schema evolution test suite** (required before Phase 2):

```rust
// crates/protocol/tests/schema_evolution.rs

#[test]
fn test_v1_client_ignores_v1_1_new_fields() {
    // Serialize Envelope with protocol_version=2 (v1.1)
    // Deserialize as v1.0 schema (missing protocol_version field)
    // Assert: no error, version field ignored
}

#[test]
fn test_v1_1_server_handles_v1_0_client() {
    // Serialize Envelope without protocol_version (v1.0)
    // Deserialize as v1.1 schema (expects protocol_version)
    // Assert: protocol_version defaults to 0 (treat as v1.0)
}

#[test]
fn test_unknown_message_type_rejected() {
    // Client sends future message type (field 50)
    // Server (v1.0) deserializes
    // Assert: logs warning, responds with ErrorResponse
}

#[test]
fn test_unknown_enum_value_defaults_to_unknown() {
    // Client sends ErrorCode=100 (future value)
    // Server (v1.0) deserializes
    // Assert: ErrorCode defaults to UNKNOWN
}
```

**CI enforcement:**
- `cargo test -p monoterminal-protocol` must pass
- `cargo build` with `--features proto-v1,proto-v2` (when v2 exists)

---

### 6. Documentation Requirements

**For every new protocol message:**

1. **Update changelog** (`docs/PROTOCOL_CHANGELOG.md`):
   ```markdown
   ## v1.1 (2026-09-01)
   
   ### Added
   - `protocol_version` field to `Envelope` (field 28)
   - `HealthCheckRequest` / `HealthCheckResponse` (fields 11-12)
   
   ### Compatibility
   - Backward compatible with v1.0 clients
   - New fields ignored by old clients
   ```

2. **Update version matrix** (`docs/PROTOCOL_VERSIONS.md`):
   ```markdown
   | Version | Min Client | Min Server | Features |
   |---------|-----------|-----------|----------|
   | v1.0    | 0.1.0     | 0.1.0     | Attach, Input, Resize, Detach |
   | v1.1    | 0.2.0     | 0.2.0     | + Health Check, Dashboard |
   | v2.0    | 0.5.0     | 0.5.0     | + P2P, Compression, New Auth |
   ```

3. **Protobuf comments** (inline):
   ```protobuf
   message Envelope {
     uint64 sequence_number = 1;
     
     // Protocol version (added v1.1, optional for v1.0 compatibility)
     // 0 = v1.0 (default), 1 = v1.1, 2 = v1.2, etc.
     uint32 protocol_version = 28;
   }
   ```

---

## Alternatives Considered

### Option A: Separate Files per Version

**Pattern:**
```
proto/
  monoterminal/v1/messages.proto
  monoterminal/v2/messages.proto
  monoterminal/v3/messages.proto
```

**Pros:**
- Clear versioning (file path = version)
- No ambiguity about which schema is active

**Cons:**
- Code duplication (must copy unchanged messages between versions)
- Larger binary (compiles all versions)
- Complex `build.rs` (multi-proto compilation)

**Verdict:** ❌ Rejected (over-engineering for Phase 1-2)

---

### Option B: JSON Schema Instead of Protobuf

**Pros:**
- Human-readable wire format
- No code generation needed
- JSON Schema has built-in versioning

**Cons:**
- 3-5× larger payloads vs Protobuf
- Slower serialization (no binary format)
- No static typing (loses Rust's type safety)

**Verdict:** ❌ Rejected (SRS §3.1.1 mandates Protobuf)

---

### Option C: GraphQL Subscriptions

**Pros:**
- Schema evolution built-in
- Field deprecation support
- Strong tooling (GraphiQL, code generation)

**Cons:**
- Requires HTTP/2 or WebSocket upgrade (adds complexity)
- No binary format (text-based)
- Overkill for terminal I/O (not a query language)

**Verdict:** ❌ Rejected (wrong tool for the job)

---

## Consequences

### Positive

- ✅ Clear rules for safe additions (no version bump needed)
- ✅ Version negotiation prevents silent breakage
- ✅ Test suite catches compatibility regressions
- ✅ Documentation trail for protocol changes

### Negative

- ⚠️ Must maintain backward compatibility for 6-12 months during transitions
- ⚠️ Server must compile multiple protocol versions (increases binary size)
- ⚠️ Version negotiation adds 1 round-trip to handshake

### Neutral

- Protocol version field adds 4 bytes to every message (negligible)
- Protobuf3's "all fields optional" is a feature, not a bug (simplifies evolution)

---

## Implementation Plan

**Phase 1 (Immediate - Before task-7 Windows Service):**
1. ✅ Delete `proto/envelope.proto` (rust-engineer-protocol, 1 hour)
2. ✅ Add `protocol_version` field to `Envelope` in `messages.proto` (rust-engineer-protocol, 30 min)
3. ✅ Update `build.rs` comment to confirm single source of truth (5 min)
4. ✅ Create `docs/PROTOCOL_CHANGELOG.md` and `docs/PROTOCOL_VERSIONS.md` (principal-architect, 1 hour)

**Phase 1.5 (Before Phase 2 P2P - Non-Blocking for MVP):**
5. ⏳ Implement version negotiation in AttachRequest handler (rust-engineer-protocol, 4 hours)
6. ⏳ Add schema evolution test suite (rust-engineer-protocol, 4 hours)
7. ⏳ Add `ErrorCode::INCOMPATIBLE_VERSION` to protobuf (30 min)

**Phase 2+ (Before P2P ships):**
8. ⏳ Document version compatibility matrix in SRS appendix
9. ⏳ Add CI check: protocol tests must pass before merge

---

## References

- SRS §3.1.1 (Protocol Buffers Schema, backward/forward compatibility)
- Protocol Buffers v3 Language Guide: https://protobuf.dev/programming-guides/proto3/
- Protobuf Evolution Best Practices: https://protobuf.dev/programming-guides/proto3/#updating
- Architecture Review 2026-08-15 (principal-architect)

---

## Follow-up Actions

1. ✅ **THIS ADR:** Approve and merge (principal-architect + eng-director review)
2. ⏳ **Immediate:** Execute Phase 1 tasks above (rust-engineer-protocol)
3. ⏳ **Before Phase 2:** Execute Phase 1.5 tasks (version negotiation + tests)
4. ⏳ **Ongoing:** Update `PROTOCOL_CHANGELOG.md` for every wire-format change

---

**Approval required from:** eng-director, rust-backend-lead  
**Estimated review time:** 30 minutes  
**Implementation time (Phase 1):** 3 hours  
**Status:** Ready for review
