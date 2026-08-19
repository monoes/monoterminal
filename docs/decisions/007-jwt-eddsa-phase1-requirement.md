# ADR-007: JWT EdDSA Algorithm for Phase 1 Authentication

**Status:** Implemented  
**Date:** 2026-08-16  
**Deciders:** principal-architect, security-engineer  
**Implementer:** security-engineer (2.5 hours)  
**SRS Reference:** §3.2.2 (Ed25519 SSH Keys + JWT Authentication)

---

## Context

Phase 1 JWT authentication implementation currently uses **HS256 (HMAC-SHA256)** symmetric signing, but SRS §3.2.2 explicitly specifies **EdDSA (Ed25519)** asymmetric signing.

**Current Implementation:**
```rust
// EXAMPLE: Phase 1 implementation (incorrect algorithm)
Algorithm::HS256
EncodingKey::from_secret(hmac_bytes)   // Symmetric HMAC
DecodingKey::from_secret(hmac_bytes)   // Same bytes for verify
```

**SRS Specification (§3.2.2, line 790):**
```json
{
  "typ": "JWT",
  "alg": "EdDSA"
}
```

**Issue Discovered:**
- security-engineer fixed test failures caused by algorithm/key-type mismatch
- Code specified `Algorithm::EdDSA` but used HMAC creation methods
- Fix applied: Changed to `Algorithm::HS256` to match HMAC
- **This creates spec deviation** requiring architectural decision

---

## Decision

**Upgrade to EdDSA before Phase 1 Gate passage.**

Phase 1 JWT implementation MUST use:
```rust
// REQUIRED: EdDSA asymmetric signing
Algorithm::EdDSA
EncodingKey::from_ed25519_der(private_bytes)   // Signing only
DecodingKey::from_ed25519_der(public_bytes)    // Verification only
```

**Rationale:**

1. **SRS Compliance:** §3.2.2 explicitly specifies EdDSA (line 790)
2. **Phase 1 Acceptance Criteria:** "Ed25519/JWT auth" listed in §7.1 (line 1444)
3. **No Technical Debt:** Better to align now than accumulate debt
4. **No Migration Cost:** No deployed instances to migrate (still in development)
5. **Simple Fix:** Keypair generation + algorithm change (~2-3 hours work)

---

## Alternatives Considered

### Option A: Accept HS256 for Phase 1, Upgrade in Phase 2

**Pros:**
- Unblocks Phase 1 Gate immediately
- HS256 is secure for localhost deployment
- No keypair distribution problem (Phase 1 is 127.0.0.1:5000 only)

**Cons:**
- ❌ Violates SRS specification (§3.2.2)
- ❌ Violates Phase 1 acceptance criteria (§7.1)
- ❌ Technical debt accumulation
- ❌ Migration path needed later (invalidates all issued JWTs)
- ❌ Bad precedent (spec deviations without ADR approval)

**Verdict:** ❌ Rejected

---

### Option B: Upgrade to EdDSA Immediately (CHOSEN)

**Pros:**
- ✅ SRS compliance (§3.2.2)
- ✅ Phase 1 acceptance criteria met (§7.1)
- ✅ No technical debt
- ✅ No future migration cost
- ✅ Asymmetric crypto benefits (see below)

**Cons:**
- ⚠️ Delays Phase 1 Gate by ~2-3 hours (keypair generation + testing)
- ⚠️ Requires Ed25519 management documentation

**Verdict:** ✅ Accepted

---

## Security Benefits of EdDSA (vs HS256)

### Symmetric HS256 (Current)

**Architecture:**
```
Master Daemon (Server)
  ├── HMAC Bytes (signs JWTs)
  └── HMAC Bytes (verifies JWTs) <-- SAME BYTES

Web Client
  └── No verification capability (trusts server)
```

**Security Properties:**
- ✅ Secure for localhost (no distribution)
- ❌ Compromise = full forge capability
- ❌ Can't distribute verification material (reveals signing material)
- ❌ Not suitable for Phase 2 P2P (clients need to verify without signing)

---

### Asymmetric EdDSA (Required)

**Architecture:**
```
Master Daemon (Server)
  ├── Private Bytes (signs JWTs) <-- PROTECTED
  └── Public Bytes (verifies JWTs) <-- SHAREABLE

Web Client
  └── Public Bytes (verifies JWTs) <-- Can verify, can't forge
```

**Security Properties:**
- ✅ Separation (verification material can be shared)
- ✅ Public material leak = no forge capability
- ✅ Suitable for P2P (Phase 2: clients verify each other's JWTs)
- ✅ Aligns with Ed25519 SSH infrastructure (SRS §3.2.2)

---

## Phase 2 P2P Requirement

**Why EdDSA is MANDATORY for Phase 2:**

Phase 2 introduces **peer-to-peer WebRTC connections** where clients connect directly to each other (SRS §7.2). In P2P scenarios:

1. **Client A** signs JWT with private material
2. **Client B** verifies JWT with Client A's public material
3. **Client B** must verify WITHOUT holding Client A's signing capability

**HS256 fails this requirement:**
- Symmetric → verification requires signing capability
- Sharing verification → any client can forge

**EdDSA satisfies this requirement:**
- Asymmetric → public material verifies, can't sign
- Public distribution → clients verify each other safely

**Conclusion:** EdDSA is not optional for Phase 2. Implementing now avoids migration cost later.

---

## Implementation Plan

### Step 1: Ed25519 Generation Module

**Location:** `crates/master/src/auth/keys.rs` (new module)

**Key storage pattern** (example):
- Private: `~/.monoterminal/identity.key` (0600 permissions)
- Public: Derived from private (no separate storage needed)
- Aligns with SRS §3.2.2 SSH pattern

### Step 2: JWT Service with EdDSA

**Location:** `crates/master/src/auth/jwt.rs`

**Algorithm requirement:**
- Header: `Algorithm::EdDSA`
- Encoding: Ed25519 private material
- Decoding: Ed25519 public material

### Step 3: Test Coverage

**Location:** `crates/master/src/auth/tests.rs`

**Required tests:**
- Roundtrip encode/decode
- Public-only verification (client scenario)
- Forged JWT rejection
- Expired JWT rejection

### Step 4: Documentation Updates

**Files to update:**

1. **docs/security-implementation-plan.md:**
   - Line 95: Confirm `EdDSA` algorithm (already correct in spec)
   - Add generation instructions

2. **web/docs/AUTH_FLOW.md:** (new file)
   - Document Ed25519 generation
   - JWT format
   - Client verification (Phase 2 preparation)

3. **README.md:**
   - Add setup step: "Generate Ed25519 on first run"

---

## Timeline & Blocking Impact

**Estimated Effort:** 2-3 hours (security-engineer)

**Breakdown:**
- Generation module: 1 hour
- JWT service update: 30 minutes
- Test updates: 1 hour
- Documentation: 30 minutes

**Blocking Assessment:**

| Work Stream | Blocked? | Impact |
|-------------|----------|--------|
| WebSocket server (tasks 16, 18) | ❌ No | Can integrate in parallel |
| E2E tests | ⚠️ Yes | Auth tests fail until fixed |
| Phase 1 Gate | ✅ Yes | MUST be fixed before gate passage |

**Mitigation:**
- security-engineer starts immediately (today)
- WebSocket integration continues in parallel (uses placeholder auth until ready)
- E2E auth tests skipped temporarily (marked `#[ignore]` with TODO)

---

## Migration Path (If Already Deployed)

**Not applicable for Phase 1** (no deployed instances), but documented for future reference:

**If HS256 JWTs were already issued:**

1. **Dual-algorithm support period** (1 week):
   - Accept both HS256 (legacy) and EdDSA (new)
   - Server logs warning for HS256 usage

2. **Deprecation notice:**
   - 7-day countdown in UI
   - Force re-authentication after deadline

3. **Cutover:**
   - Reject HS256 after 7 days
   - EdDSA only

**Phase 1 advantage:** No migration needed (fresh implementation).

---

## Consequences

### Positive

- ✅ SRS compliance (§3.2.2)
- ✅ Phase 1 acceptance criteria met
- ✅ No technical debt
- ✅ Phase 2 P2P ready (asymmetric verification)
- ✅ Material separation (public shareable)
- ✅ Better security posture (compromise isolation)

### Negative

- ⚠️ Delays Phase 1 Gate by 2-3 hours (acceptable)
- ⚠️ Requires Ed25519 management documentation
- ⚠️ Slightly more complex than HS256 (asymmetric vs symmetric)

### Neutral

- EdDSA signing/verification performance: 50µs sign, 100µs verify (per SRS §3.2.2) — negligible overhead
- Material size: 32 bytes (Ed25519 private) — no storage concern

---

## Approval & Execution

**Approval required from:**
- ✅ principal-architect (approved, this ADR)
- ⏳ security-engineer (assigned, implementing)

**Execution:**
1. security-engineer implements Steps 1-4 (2-3 hours)
2. Test coverage verification (auth tests pass)
3. Update ADR status: Decided → Implemented
4. Mark Phase 1 Gate criteria "Ed25519/JWT auth" as ✅ Complete

**Coordination:**
- rust-backend-lead: WebSocket integration continues in parallel
- qa-lead: E2E auth tests re-enabled after implementation

---

## References

- SRS §3.2.2 (Ed25519 SSH Keys + JWT Authentication, line 779-803)
- SRS §7.1 Phase 1 Acceptance Criteria (line 1444)
- RFC 8032: Edwards-Curve Digital Signature Algorithm (EdDSA)
- RFC 7519: JSON Web Token (JWT)
- ed25519-dalek crate: https://docs.rs/ed25519-dalek/
- jsonwebtoken crate: https://docs.rs/jsonwebtoken/

---

**Maintained by:** principal-architect  
**Last updated:** 2026-08-16  
**Status:** ✅ Implemented (security-engineer, 19/19 tests passing)

---

## Implementation Record

**Completed:** 2026-08-16  
**Implementer:** security-engineer  
**Time:** 2.5 hours (within 2-3h estimate)

**Deliverables:**
- `crates/master/src/auth/keys.rs` (227 lines) - Ed25519 key generation
- `crates/master/src/auth/jwt.rs` (updated) - EdDSA algorithm
- `crates/master/tests/auth_integration.rs` (updated) - 19 tests passing
- `web/docs/AUTH_FLOW.md` (new) - Comprehensive auth guide
- `docs/security-implementation-plan.md` (updated) - EdDSA status
- `README.md` (updated) - Ed25519 setup instructions

**Verification:**
- ✅ Compilation: Success
- ✅ Tests: 19/19 passing
- ✅ JWT Header: `{"alg": "EdDSA", "typ": "JWT"}`
- ✅ Ed25519 key format: PEM (PKCS#8 DER for private, SPKI DER for public)

**SRS Compliance:**
- ✅ §3.2.2: Ed25519 SSH Keys + JWT Authentication
- ✅ Phase 1 acceptance criteria: "Ed25519/JWT auth"
- ✅ Phase 2 ready: Asymmetric verification for P2P

**Phase 1 Gate Criteria:**
- ✅ "Ed25519/JWT auth" → **COMPLETE**
