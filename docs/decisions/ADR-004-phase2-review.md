# ADR-004 Phase 2 Architectural Constraints Review

**Review Date:** 2026-08-15  
**Reviewer:** principal-architect  
**Task:** task-7  
**Scope:** Analyze ADR-004 (Protocol Schema Evolution) against Phase 2 requirements

---

## Executive Summary

**Status:** ✅ **PASS WITH RECOMMENDATIONS**

ADR-004's protocol evolution policy **adequately supports Phase 2 requirements** with minor gaps that should be addressed before Phase 2 implementation begins. The policy's protobuf3 evolution rules allow all identified Phase 2 message types to be added as non-breaking changes.

**Critical Findings:**
- ✅ **No blocking issues** — Phase 2 can proceed under ADR-004's rules
- ⚠️ **3 architectural gaps** requiring clarification (detailed below)
- ⚠️ **1 missing consideration** — P2P version negotiation differs from client-server
- ✅ **Compression support** — compatible with current envelope design

**Recommendation:** Approve ADR-004 with amendments to address P2P version negotiation (Section 5) and compression envelope field (Section 6).

---

## 1. Phase 2 Protocol Requirements Analysis

### 1.1 Phase 2 Features Requiring Protocol Changes

From SRS §7.2 and §5.2.3:

| Feature | Protocol Impact | Message Types Needed |
|---------|-----------------|---------------------|
| **P2P WebRTC** | High — clients connect to each other, not just servers | Version negotiation in peer-to-peer handshake |
| **Multi-session management** | Medium | `CreateSessionRequest/Response`, `ListSessionsRequest/Response`, `KillSessionRequest/Response` |
| **Multi-client attach** | Low — already in v1 | Existing `AttachRequest/Response` sufficient |
| **Presence indicators** | Medium | `ClientJoined`, `ClientLeft`, `PresenceUpdate` |
| **Input broadcasting modes** | Medium | `RequestInputControl`, `GrantInputControl`, `InputLockAcquired`, `InputLockReleased` |
| **Cursor sharing** | Low | `CursorPosition` (broadcast) |
| **Scrollback sync** | Low | `ScrollViewport` (broadcast) |
| **zstd compression** | High — affects envelope | `compression_type` field in `Envelope` OR transparent below protobuf layer |

**Total new message types:** ~12-15 additions to `Envelope.oneof message`

### 1.2 Compatibility with ADR-004 Evolution Rules

**Analysis:** All Phase 2 message types can be added as **non-breaking changes** under ADR-004 §2 (Safe Additions):

✅ **Add new message types to `oneof`** (ADR-004 §2):
```protobuf
oneof message {
  // ... existing v1.0 messages ...
  CreateSessionRequest create_session_request = 20;     // NEW (Phase 2)
  CreateSessionResponse create_session_response = 21;   // NEW (Phase 2)
  ClientJoined client_joined = 22;                      // NEW (Phase 2)
  ClientLeft client_left = 23;                          // NEW (Phase 2)
  CursorPosition cursor_position = 24;                  // NEW (Phase 2)
  ScrollViewport scroll_viewport = 25;                  // NEW (Phase 2)
  // ... etc.
}
```

**Old clients:** Ignore unknown message types, server responds with `ErrorResponse` (ADR-004 §2 guarantee)

**Old servers:** Ignore unknown message types, log warning, send `ErrorResponse`

**Verdict:** ✅ **No protocol version bump required for Phase 2**

---

## 2. Critical Gap #1: P2P Version Negotiation

### 2.1 Problem Statement

ADR-004 §4 (Version Negotiation Protocol) assumes **client-server topology**:

```
Client                          Server
  │                               │
  ├─ AttachRequest ──────────────►│
  │  (protocol_version = 2)       │
  │                               │
  │◄────────── AttachResponse ────┤
  │  (protocol_version = 1)       │ Server downgrades to min(2, 1) = 1
```

**Phase 2 introduces P2P topology** (SRS §2.3.1):

```
Client A                        Client B
    │                               │
    │  ◄─── SDP Offer ─────────────│  (WebRTC handshake)
    │  ───► SDP Answer ────────────►│
    │                               │
    └──── DataChannel (DTLS) ───────┘
          Protobuf messages
```

**Questions:**
1. Which peer initiates version negotiation in P2P?
2. Does `AttachRequest` still carry the version, or is there a separate `PeerHandshake` message?
3. What happens if two peers have incompatible versions (e.g., v1.0 ↔ v2.0)?

### 2.2 Current ADR-004 Coverage

**Covered:**
- ✅ Client→Server version negotiation (§4)
- ✅ Downgrade logic: `min(client_version, server_version)`
- ✅ Rejection for incompatible versions (`INCOMPATIBLE_VERSION` error)

**Not Covered:**
- ❌ Peer→Peer version negotiation
- ❌ Who sends the first `AttachRequest` in P2P (or is there a `PeerHandshake`?)
- ❌ Bidirectional downgrade (both peers must agree on same version)

### 2.3 Recommended Amendment

**Add to ADR-004 §4 (Version Negotiation Protocol):**

#### 4.3 P2P Version Negotiation (Phase 2)

**P2P Handshake Flow:**

```
Peer A (initiator)              Peer B (responder)
    │                               │
    ├─ PeerHandshake ──────────────►│  (protocol_version = 2)
    │                               │
    │◄──── PeerHandshakeAck ────────┤  (protocol_version = 1)
    │                               │
    │    Both use min(2, 1) = 1     │
```

**New message types (add to `Envelope.oneof`):**

```protobuf
message PeerHandshake {
  uint32 protocol_version = 1;  // This peer's version
  string peer_id = 2;            // WebRTC peer ID
  bytes public_key = 3;          // Ed25519 key for auth
}

message PeerHandshakeAck {
  uint32 protocol_version = 1;  // Responder's version (negotiated = min)
  string peer_id = 2;
  bytes public_key = 3;
}
```

**Negotiation Rule:**
- **Initiator** (Peer A): Sends `PeerHandshake` with its version
- **Responder** (Peer B): Computes `negotiated_version = min(A_version, B_version)`
- **Responder** sends `PeerHandshakeAck` with `negotiated_version`
- **Both peers** use `negotiated_version` for all subsequent messages

**Incompatible Version Handling:**
- If `negotiated_version < min_supported_version` for either peer → close DataChannel, log error
- Example: Peer A requires v2+, Peer B only supports v1 → connection rejected

**Impact:** Requires 2 new message types (`PeerHandshake`, `PeerHandshakeAck`) — compatible with ADR-004 §2 (non-breaking addition)

---

## 3. Critical Gap #2: Compression Field Placement

### 3.1 Problem Statement

Phase 2 requires **zstd compression** (SRS §7.2). Where does compression metadata live?

**Option A: Envelope-level field**
```protobuf
message Envelope {
  uint64 sequence_number = 1;
  uint32 protocol_version = 28;
  CompressionType compression = 29;  // NEW FIELD
  oneof message { ... }
}

enum CompressionType {
  NONE = 0;
  ZSTD = 1;
  // Future: LZ4, Brotli
}
```

**Option B: Transparent below protobuf**
- Compress the entire serialized `Envelope` before sending
- Decompress before protobuf deserialization
- Compression negotiated via separate handshake (not in protobuf)

### 3.2 Current ADR-004 Coverage

**Not Mentioned:** Compression is not addressed in ADR-004 at all.

### 3.3 Analysis

**Option A (Envelope field):**
- ✅ Explicit, self-describing (receiver knows how to decompress)
- ✅ Per-message compression control (compress large `OutputData`, skip small `InputData`)
- ✅ Compatible with ADR-004 §2 (non-breaking: old clients ignore field, default to `NONE`)
- ❌ Adds 1 byte overhead per message

**Option B (Transparent):**
- ✅ Zero protobuf overhead
- ✅ Simpler protobuf schema
- ❌ Requires separate compression negotiation (not self-describing)
- ❌ All-or-nothing (can't selectively compress messages)
- ❌ Breaks protobuf inspection tools (`protoc --decode` won't work without decompression step)

### 3.4 Recommended Amendment

**Add to ADR-004 §2 (Safe Additions):**

#### 2.4 Compression Support (Phase 2)

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
  // Reserved: 2-15 for future algorithms
}
```

**Compression logic:**

1. **Sender:** Compress `oneof message` field ONLY (leave `sequence_number`, `protocol_version`, `compression` uncompressed)
2. **Receiver:** If `compression == ZSTD`, decompress `oneof message` bytes before protobuf deserialization
3. **Backward compatibility:** v1.0 clients ignore `compression` field, receive uncompressed messages (sender detects via `protocol_version` downgrade)

**Size threshold:** Only compress messages ≥ 1KB (compression overhead not worth it for small messages like `InputData`)

**Impact:** Non-breaking addition (ADR-004 §2), 1 byte overhead per message

---

## 4. Critical Gap #3: Field Number Reservation Strategy

### 4.1 Problem Statement

ADR-004 §2 shows `protocol_version` added at field number **28**:

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  uint32 protocol_version = 28;  // Why 28? Gap: 2-27 unused
  oneof message { ... }
}
```

**Question:** Why leave 26 unused field numbers (2-27)? What's the reservation strategy?

### 4.2 Current ADR-004 Coverage

**Not Explained:** No rationale for field number 28 choice.

### 4.3 Analysis

**Possible Rationales:**
1. **Reserve 2-10 for high-priority metadata** (e.g., compression, encryption flags, tracing IDs)
2. **Reserve 11-27 for oneof message expansion** (SRS §3.1.1 shows messages start at 2)
3. **Random choice** (no strategy)

**Risk:** If no reservation strategy documented, future developers may:
- Accidentally use field 2-27 for incompatible purposes
- Create conflicts when backporting features
- Break the "field numbers never change" protobuf rule

### 4.4 Recommended Amendment

**Add to ADR-004 §2 (Safe Additions):**

#### 2.5 Field Number Reservation

**Envelope field number allocation:**

| Range | Purpose | Example |
|-------|---------|---------|
| 1 | Sequence number (v1.0) | `sequence_number = 1` |
| 2-19 | Reserved for `oneof message` expansion | `attach_request = 2`, `output_data = 5`, etc. |
| 20-27 | Reserved for envelope metadata (future) | Encryption, tracing, QoS flags |
| 28-31 | Envelope metadata (current) | `protocol_version = 28`, `compression = 29` |
| 32-2047 | Unreserved (future use) | — |

**Rule:** Never use field numbers outside the designated range for that purpose. Document exceptions in `PROTOCOL_CHANGELOG.md`.

**Rationale for 28:**
- Leaves 2-19 for message type expansion (Phase 2 adds ~12-15 types)
- Leaves 20-27 for future metadata (encryption=20, trace_id=21, qos=22, etc.)
- Signals "this is metadata, not a message type" by being outside the 2-19 range

---

## 5. Positive Findings

### 5.1 Adequate Test Coverage Plan

ADR-004 §5 (Testing Requirements) covers:
- ✅ v1.0 client ignoring v1.1 fields
- ✅ v1.1 server handling v1.0 clients
- ✅ Unknown message type rejection
- ✅ Unknown enum value defaulting

**Phase 2 Extension Needed:**
- Add P2P version negotiation tests (`PeerHandshake` ↔ `PeerHandshakeAck`)
- Add compression round-trip tests (compress/decompress, size validation)

### 5.2 Documentation Requirements Are Strong

ADR-004 §6 mandates:
- ✅ `PROTOCOL_CHANGELOG.md` updates for every change
- ✅ Version compatibility matrix
- ✅ Inline protobuf comments

**No changes needed** — this is already comprehensive.

### 5.3 Single Source of Truth Achieved

ADR-004 §1 decision:
- ✅ `proto/monoterminal/v1/messages.proto` is the only file
- ✅ `proto/envelope.proto` deleted (obsolete prototype)

**Phase 2 Impact:** All new messages added to same file — no fragmentation risk.

---

## 6. Phase 2 Message Inventory (Detailed)

### 6.1 Confirmed New Message Types

Based on SRS §5.2.3 (Collaboration Features) and §7.2 (Phase 2 Goals):

| # | Message Type | Purpose | Size Est. | Frequency |
|---|--------------|---------|-----------|-----------|
| 20 | `CreateSessionRequest` | Create new session | 100B | Low (1/session) |
| 21 | `CreateSessionResponse` | Return session ID | 150B | Low |
| 22 | `ListSessionsRequest` | Query active sessions | 50B | Low (1/attach) |
| 23 | `ListSessionsResponse` | Return session list | 500B-2KB | Low |
| 24 | `KillSessionRequest` | Terminate session | 80B | Low |
| 25 | `KillSessionResponse` | Confirm kill | 50B | Low |
| 26 | `ClientJoined` | Presence: new client | 120B | Medium (1/attach) |
| 27 | `ClientLeft` | Presence: client disconnect | 100B | Medium (1/detach) |
| 28 | `PresenceUpdate` | Client metadata change | 150B | Low (on name/avatar change) |
| 29 | `RequestInputControl` | Request write permission | 80B | Low (moderator mode) |
| 30 | `GrantInputControl` | Grant write permission | 80B | Low |
| 31 | `InputLockAcquired` | Exclusive input started | 80B | Low (30s timeout mode) |
| 32 | `InputLockReleased` | Exclusive input ended | 80B | Low |
| 33 | `CursorPosition` | Cursor sharing broadcast | 50B | **High** (debounced 100ms) |
| 34 | `ScrollViewport` | Scrollback sync broadcast | 60B | Medium (debounced 500ms) |
| 35 | `PeerHandshake` | P2P version negotiation (initiator) | 200B | Low (1/P2P connection) |
| 36 | `PeerHandshakeAck` | P2P version negotiation (responder) | 200B | Low |

**Total:** 17 new message types (fields 20-36)

**Compatibility:** All fit within field range 2-19 if we shift them down, OR use 20-36 as proposed in §4.4 (unreserved range)

**Recommendation:** Use fields 20-36 to avoid collision with future v1.0 additions in the 2-19 range.

---

## 7. Compression Impact Analysis

### 7.1 Compression Effectiveness Estimate

**Phase 2 compression target:** zstd (SRS §7.2)

**Compressible messages:**

| Message Type | Uncompressed | Compressed (zstd) | Ratio | Frequency |
|--------------|--------------|-------------------|-------|-----------|
| `OutputData` (4KB chunk) | 4096B | 800-1200B | 3-5× | **High** (100/s) |
| `AttachResponse` (scrollback) | 10KB-500KB | 2KB-100KB | 5-10× | Low (1/attach) |
| `ListSessionsResponse` | 500B-2KB | 200B-800B | 2-3× | Low |

**Non-compressible (overhead > benefit):**

| Message Type | Size | Frequency | Verdict |
|--------------|------|-----------|---------|
| `InputData` | 10-50B | High (typing) | ❌ Skip compression |
| `CursorPosition` | 50B | High (100ms debounce) | ❌ Skip compression |
| `ResizeRequest` | 80B | Low (on window resize) | ❌ Skip compression |

**Compression threshold:** 1KB (messages < 1KB not worth compressing)

### 7.2 Compression Compatibility

**With ADR-004 evolution rules:**
- ✅ Adding `compression` field (§3.4) is a non-breaking change
- ✅ Old clients ignore field, receive uncompressed messages
- ✅ New clients detect old servers (via `protocol_version`), send uncompressed

**With WebRTC DataChannel:**
- ✅ zstd operates above DataChannel (DataChannel already has DTLS compression, but we control app-layer)
- ✅ No conflict with DTLS compression (orthogonal layers)

---

## 8. Risk Assessment

### 8.1 High Risk (Requires ADR-004 Amendment)

| Risk | Impact | Mitigation |
|------|--------|------------|
| **P2P version negotiation undefined** | Phase 2 P2P launch blocked (no handshake protocol) | Add §4.3 (P2P Version Negotiation) to ADR-004 before Phase 2 |
| **Compression field missing** | Phase 2 compression can't be backward-compatible | Add §2.4 (Compression Support) to ADR-004 before Phase 2 |

### 8.2 Medium Risk (Recommendations)

| Risk | Impact | Mitigation |
|------|--------|------------|
| **Field number collisions** | Future developers use 2-27 incorrectly | Add §2.5 (Field Number Reservation) to ADR-004 |
| **Test coverage gap** | P2P scenarios not tested | Extend ADR-004 §5 test suite with P2P handshake tests |

### 8.3 Low Risk (Monitor)

| Risk | Impact | Mitigation |
|------|--------|------------|
| **17 new message types** | Binary size increase (~5-10KB) | Acceptable — within Rust binary overhead budget |
| **CursorPosition broadcast rate** | 100ms debounce = 10 msg/s × 50 clients = 500 msg/s | Acceptable — Phase 2 target is 100 sessions × 50 clients max |

---

## 9. Recommendations

### 9.1 Immediate (Before Monday 0800 ADR-004 Approval)

1. **Amend ADR-004 §4:** Add §4.3 "P2P Version Negotiation (Phase 2)" (see §2.3 of this review)
2. **Amend ADR-004 §2:** Add §2.4 "Compression Support (Phase 2)" (see §3.4 of this review)
3. **Amend ADR-004 §2:** Add §2.5 "Field Number Reservation" (see §4.4 of this review)

**Estimated Time:** 30 minutes (principal-architect)

### 9.2 Before Phase 2 Implementation Start

4. **Extend ADR-004 §5:** Add P2P handshake tests to schema evolution test suite (rust-engineer-protocol, 2 hours)
5. **Extend ADR-004 §5:** Add compression round-trip tests (rust-engineer-protocol, 2 hours)
6. **Create Phase 2 message inventory:** Document all 17 new message types in `docs/PROTOCOL_CHANGELOG.md` (rust-engineer-protocol, 1 hour)

### 9.3 Phase 2 Gate Criteria

7. **Version negotiation working:** P2P handshake tested with v1.0 ↔ v1.1 peers
8. **Compression working:** zstd compression tested with 4KB `OutputData` chunks (3-5× ratio)
9. **Backward compatibility proven:** v1.0 client can attach to v1.1 server running Phase 2 features (graceful degradation)

---

## 10. Conclusion

**Verdict:** ✅ **APPROVE ADR-004 WITH AMENDMENTS**

ADR-004's protocol evolution policy is **architecturally sound** for Phase 2, with three gaps that should be closed before Phase 2 implementation:

1. **P2P version negotiation** — add §4.3 to ADR-004
2. **Compression envelope field** — add §2.4 to ADR-004
3. **Field number reservation** — add §2.5 to ADR-004

With these amendments, ADR-004 provides a **complete foundation** for Phase 2's protocol expansion (17 new message types, zstd compression, P2P collaboration features).

**No blocking issues found.** Phase 2 can proceed on schedule (Months 4-6, SRS §7.2).

---

## Appendix: Phase 2 Protocol Timeline

| Milestone | Deliverable | Owner | Due |
|-----------|-------------|-------|-----|
| **ADR-004 Amendments** | §4.3, §2.4, §2.5 added | principal-architect | Before Monday 0800 approval |
| **P2P Handshake Protobuf** | `PeerHandshake`, `PeerHandshakeAck` messages | rust-engineer-protocol | Phase 2 Month 1 |
| **Compression Protobuf** | `compression` field, `CompressionType` enum | rust-engineer-protocol | Phase 2 Month 1 |
| **Collaboration Protobuf** | 15 message types (§6.1) | rust-engineer-protocol | Phase 2 Month 2 |
| **Schema Evolution Tests** | P2P + compression tests | rust-engineer-protocol | Phase 2 Month 2 |
| **Protocol v1.1 Release** | All Phase 2 messages, backward-compatible | rust-backend-lead | Phase 2 Month 3 |

---

**Review Completed:** 2026-08-15  
**Next Action:** Amend ADR-004, submit to eng-director for Monday 0800 approval  
**Follow-up:** Phase 2 protocol implementation plan (rust-engineer-protocol, post-approval)
