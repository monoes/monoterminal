# ADR-014: Collaboration Primitives

**Status:** Draft — Pending Phase 1 Gate  
**Date:** 2026-08-17  
**Deciders:** principal-architect, security-engineer  
**SRS Reference:** §2.1.5, §3.2.3, §7.2 (Multi-Client Collaboration)  
**Phase:** Phase 2 (P2P + Persistence)

---

## Context

Phase 2 introduces multi-client collaboration features to enable:
- **Shared terminal sessions** (multiple users attached to the same PTY)
- **Presence indicators** (see who else is connected, active/idle status)
- **Input coordination** (prevent input conflicts, show typing indicators)
- **Role-based access control** (owner/editor/viewer permissions per SRS §2.1.5)

**Current State (Phase 1):**
- **Single-client sessions** (only one client can attach to a session at a time)
- **No access control** (anyone with network access can attach)
- **No presence awareness** (can't see if someone else is using a session)

**Phase 2 Requirements (SRS §7.2, §2.1.5):**
- Multi-client attach (collaboration: N clients on one session)
- Presence indicators (who's here, typing status)
- RBAC roles: owner, editor, viewer (per §2.1.5)

---

## Decision

Implement **collaboration primitives** with broadcast model, presence tracking, RBAC, and input queueing.

---

## 1. Multi-Client Attach Model

### 1.1 Broadcast Model (Not Synchronized Cursors)

**Design choice:** All clients see the SAME terminal output (broadcast model), NOT independent cursors.

**Characteristics:**
- ✅ **Same view:** All clients see identical terminal state
- ✅ **Simpler model:** No per-client cursor synchronization (tmux/screen model)
- ✅ **Input serialization:** Inputs queued, applied sequentially to PTY

**Rejected alternative: Independent cursors**
- ❌ Each client would need separate PTY instance (not true collaboration)
- ❌ Confusing UX: shared shell state, not shared screen

---

## 2. Presence Indicators

### 2.1 Presence Data Model

**Per protocol-phase2-design.md §2:**

```protobuf
message ClientPresence {
    string client_id = 1;
    string device_name = 2;
    ClientType client_type = 3;
    uint64 last_seen_ms = 4;
    bool is_active = 5;
    uint64 joined_at_ms = 6;
    optional string user_id = 7;
}
```

### 2.2 Heartbeat & Stale Client Detection

**Heartbeat schedule:**
- **Client sends:** Every 30 seconds (ClientHeartbeat message)
- **Master checks:** Every 60 seconds (background task)
- **Eviction threshold:** 120 seconds (2 missed heartbeats + 1 grace period)

### 2.3 Input Focus Tracking

**Purpose:** Show who's actively typing (prevents input conflicts).

**UI display:** Green border if active, gray if idle >5min.

---

## 3. Role-Based Access Control (RBAC)

### 3.1 Permission Model

**Roles (per SRS §2.1.5):**

| Role | Create Session | Read Output | Send Input | Terminate Session |
|------|----------------|-------------|------------|-------------------|
| **owner** | ✅ | ✅ | ✅ | ✅ |
| **editor** | ❌ | ✅ | ✅ | ❌ |
| **viewer** | ❌ | ✅ | ❌ | ❌ |

### 3.2 ACL Management

**Grant/revoke permissions via master daemon API.**

**CLI interface:**

```bash
# Grant editor role
monoterminal session grant --session abc123 --user bob@example.com --role editor

# Revoke access
monoterminal session revoke --session abc123 --user bob@example.com
```

### 3.3 JWT Integration

**Authentication flow:**
1. Client sends AttachRequest with auth_token field (JWT string)
2. Master verifies JWT signature using Ed25519 public key (per ADR-007, ADR-008)
3. Extract user_id from JWT "sub" claim
4. Check session ACL: Does user_id have permission for requested action?
5. If authorized, attach client; otherwise return ErrorCode::PERMISSION_DENIED

**Integration points:**
- Uses existing Ed25519 key infrastructure (ADR-008)
- JWT verification via jsonwebtoken crate (EdDSA algorithm)
- User ID stored in `session.owner_user_id` (database column)
- ACL stored in `session.acl` TEXT column (JSON-encoded, per ADR-012 §2.1), loaded into HashMap at runtime

---

## 4. Input Conflict Resolution

### 4.1 Input Queueing (Sequential Execution)

**Design choice:** Serialize all inputs (FIFO queue), NO simultaneous input.

**Properties:**
- ✅ **Deterministic:** Inputs processed in receive order
- ✅ **Audit trail:** Every input logged with client_id
- ❌ **No undo:** Can't retract input once queued (acceptable for Phase 2)

**Alternative rejected: CRDT**
- ❌ Overkill for sequential shell I/O
- ❌ CRDTs designed for concurrent edits (Google Docs), not terminals

### 4.2 Input Locking (Future Phase 3+)

**Deferred:** Explicit "request input lock" mechanism.

**Phase 2 decision:** Queue-only (no locking), defer to Phase 3 if user complaints.

---

## 5. Collaboration UI Patterns

### 5.1 Presence Bar

**Web client:** Avatar bar at top showing all attached clients.

**Indicators:**
- **Green ring:** Active (typing)
- **Gray ring:** Idle (no focus or >30s since last input)

### 5.2 Share Dialog

**Owner-only feature:** Grant/revoke access with role selection (editor/viewer).

---

## 6. Testing Strategy

### 6.1 Collaboration Tests

**Test coverage:**
- Multi-client attach (2+ clients on one session)
- RBAC permission denied (viewer attempts to send input)
- Stale client eviction (no heartbeat for 2min)

### 6.2 Input Queueing Tests

**Verify:** Sequential execution order (A before B in FIFO).

---

## Consequences

### Positive
- ✅ Real-time collaboration (pair programming, debugging)
- ✅ RBAC enforced (owner controls who can type)
- ✅ Presence awareness (see who's connected, active/idle)
- ✅ Input audit trail (compliance: who typed what)

### Negative
- ⚠️ Input conflicts possible (queue-only, no locking in Phase 2)
- ⚠️ Heartbeat overhead (30s interval, 100 clients = 3.3 msg/sec)
- ⚠️ ACL management adds complexity (grant/revoke API, UI)

### Neutral
- Broadcast model simpler than synchronized cursors
- Viewer role useful for monitoring (live demo, debugging sessions)

---

## References

- **ADR-004:** Protocol Schema Evolution (PresenceUpdate, ClientHeartbeat)
- **ADR-007/008:** JWT + Ed25519 authentication
- **ADR-013:** Multi-Session Architecture (session ownership)
- **SRS §2.1.5:** RBAC roles (owner, editor, viewer)
- **SRS §3.2.3:** Security - Ed25519 + JWT auth
- **protocol-phase2-design.md:** Presence protocol, heartbeat timing

---

## Follow-up Actions

1. ⏳ **Pending Phase 1 gate passage** (Friday 5/7 threshold)
2. ⏳ **Approve ADR-014** (eng-director, security-engineer review)
3. ⏳ **Implement presence tracking** (rust-backend-lead, Week 2-3)
4. ⏳ **Implement RBAC** (security-engineer, Week 3-4)
5. ⏳ **UI mockups for presence bar** (frontend-lead, Week 2)
6. ⏳ **Integration test: 10-client collaboration** (test-engineer-e2e, Week 5)

---

**Phase 2 ADRs complete.** All four architectural design documents delivered.
