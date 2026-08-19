# ADR-008: Ed25519-Only Authentication for Phase 1

**Status:** Approved  
**Date:** 2026-08-16  
**Deciders:** principal-architect, security-engineer  
**Implementer:** security-engineer (6 hours remaining, target 2026-08-18 EOD)  
**SRS Reference:** §3.2.2 (Authentication - Challenge/Response Flow, Ed25519 + JWT)

---

## Context

Phase 1 Gate criteria #3 (Monomind Detection) and #4 (Embedded Dashboard) blocked by incomplete authentication flow (62.5% E2E pass rate).

**Backend infrastructure exists:**
- Ed25519 keypair generation (ADR-007)
- JWT issuance/verification with EdDSA (ADR-007, 19/19 tests passing)
- ChallengeRequest handler (security-engineer, 75% complete)

**Missing:**
- Protocol messages: AuthRequest, AuthResponse, TokenRefreshRequest/Response
- Server handlers: Auth request processing, JWT refresh
- Web client: Ed25519 keypair generation, challenge signing, JWT storage
- E2E tests for full auth flow

---

## Decision

**APPROVED: Ed25519-only authentication for Phase 1**

NO username/password. NO multi-method auth.

### Rationale

1. **SRS Compliance:** §3.2.2 explicitly specifies Ed25519 + JWT (lines 762-777, 1444)
2. **ADR-007 Foundation:** EdDSA already implemented, tested, working
3. **Phase 2 P2P Ready:** Asymmetric verification mandatory for peer-to-peer (ADR-007 §7)
4. **Simplicity:** No user management DB for MVP
5. **Extensible:** Username/password can be ADDED in Phase 4 (enterprise) without breaking existing flow

### Rejected: Multi-Method Auth (Username/Password + Ed25519)

- ❌ Not in SRS §3.2.2
- ❌ Requires user management DB (out of Phase 1 scope)
- ❌ Delays Phase 1 Gate by 10-15 hours
- ⚠️ Can be added in Phase 4 if enterprise customers require it

---

## Protocol Design

### New Message Types (Fields 18-23)

```protobuf
// proto/monoterminal/v1/messages.proto

message ChallengeRequest {}

message ChallengeResponse {
  bytes nonce = 1;          // 32-byte random (256 bits)
  int64 expires_at = 2;     // Unix timestamp, 30s TTL
}

message AuthRequest {
  bytes public_key = 1;     // Ed25519 public key (32 bytes)
  bytes signature = 2;      // Ed25519 signature over nonce (64 bytes)
  bytes nonce = 3;          // From ChallengeResponse
}

message AuthResponse {
  string access_token = 1;  // JWT (15min TTL)
  string refresh_token = 2; // JWT (30d TTL)
  string user_id = 3;       // SHA-256(public_key) hex
}

message TokenRefreshRequest {
  string refresh_token = 1;
}

message TokenRefreshResponse {
  string access_token = 1;  // New 15min JWT
  string refresh_token = 2; // Rotated 30d JWT
}
```

### Field Numbers (ADR-004 Compliance)

```protobuf
message Envelope {
  uint64 sequence_number = 1;
  oneof message {
    // Phase 1 (2-17)
    MonitoringData monitoring_data = 17;
    
    // Auth (18-23)
    ChallengeRequest challenge_request = 18;
    ChallengeResponse challenge_response = 19;
    AuthRequest auth_request = 20;
    AuthResponse auth_response = 21;
    TokenRefreshRequest token_refresh_request = 22;
    TokenRefreshResponse token_refresh_response = 23;
    // Phase 2 expansion: 24-30 available
  }
}
```

---

## Implementation Status

**Backend (security-engineer): 75% complete**
- ✅ Ed25519/JWT infrastructure (651 lines, ADR-007)
- ✅ Protocol messages defined
- ✅ ChallengeRequest handler
- ⏳ AuthRequest handler (Ed25519 verify → JWT issue) — 2h remaining
- ⏳ TokenRefreshRequest handler (rotation + reuse detection) — 1.5h
- ⏳ Rate limiting (5 attempts/hour per SRS §3.2.4) — 1h
- ⏳ Integration tests — 1.5h

**Web Client: 0% (blocked until backend complete)**

**Timeline:** 6 hours remaining (2026-08-18 EOD target on track)

---

## Security Properties

**Nonce Management:**
- 32-byte random nonce (crypto::rand::thread_rng)
- 30-second TTL (prevents replay)
- Background cleanup task (every 60s)

**User ID Derivation:**
- Algorithm: `SHA-256(public_key).to_hex()`
- Output: 64 hex characters
- Properties: Deterministic, anonymous, collision-resistant

**Rate Limiting (SRS §3.2.4):**
- 5 auth attempts per hour per IP address
- Action: Reject with ErrorCode::RATE_LIMIT_EXCEEDED
- Storage: HashMap (Phase 1), SQLite (Phase 2)

**JWT Security:**
- Access: 15min TTL (in-memory, included in every request)
- Refresh: 30d TTL (localStorage), rotated on use
- Refresh reuse detection: Old refresh invalidated immediately

---

## Success Criteria

**Phase 1 Gate Unblocked:**
- ✅ Protocol: 6 message types (fields 18-23) — security-engineer reports DONE
- ⏳ Server: AuthRequest + TokenRefresh handlers — 6h remaining
- ⏳ Client: Ed25519 keypair + JWT storage — blocked until backend complete
- ⏳ Tests: 12+ E2E auth tests — 1.5h planned
- ⏳ QA: 62.5% → 100% pass rate (8/8 tests) — target 2026-08-18

**Deliverables:**
1. Protocol schema: 6 new message types (ADR-004 compliant)
2. Server: auth/handlers.rs, auth/challenge.rs (~500 lines)
3. Web client: auth/keypair.ts, auth/manager.ts (~270 lines)
4. Tests: auth_e2e.rs, auth.test.ts (~400 lines)

---

## Backward Compatibility

**Impact:** ✅ Non-breaking addition (ADR-004 compliant)

- New message types added to Envelope.oneof
- Old clients ignore unknown messages (protobuf3 default)
- Phase 1 is pre-release (no deployed clients, no migration)

---

## Future Work

**Phase 2:**
- Revocation list (SQLite table)
- Session management (track active sessions per user_id)
- Display name mapping (user_id → human-readable name)
- Keypair export/import UI

**Phase 4 (Enterprise):**
- Multi-method auth: username/password as ADDITIONAL method
- OAuth2/OIDC SSO (Google, Microsoft, Okta)
- FIPS mode (SRS §3.2.5)
- Audit logging (all auth attempts to SQLite)

---

## References

- SRS §3.2.2 (Authentication - Ed25519 + JWT, lines 752-803)
- SRS §3.2.4 (Rate Limiting, lines 850-862)
- SRS §7.1 Phase 1 Acceptance Criteria (line 1444)
- ADR-007: JWT EdDSA Algorithm for Phase 1 Authentication
- ADR-004: Protocol Schema Evolution Policy
- RFC 8032: Edwards-Curve Digital Signature Algorithm (EdDSA)
- RFC 7519: JSON Web Token (JWT)

---

**Approval:** principal-architect (2026-08-16)  
**Implementation:** security-engineer (75% complete, 6h remaining)  
**Target:** 2026-08-18 EOD  
**Phase 1 Gate Impact:** UNBLOCKS criteria #3 (Monomind Detection) and #4 (Embedded Dashboard)
