# Auth Integration Phase 1: Final Wiring Implementation Plan
**Security Engineer** | Prepared during build compilation wait

---

## Overview

Wire JWT authentication verification into all authenticated handlers per SRS §3.2.2.

**Prerequisites from devops-lead:**
- ✅ Server struct has `auth_service: Arc<dyn AuthService>` field
- ✅ `handle_websocket()` receives `auth_service` parameter
- ✅ `process_message()` receives `auth_service` parameter
- ✅ Code compiles

---

## Implementation Tasks

### Task 1: Wire Auth into AttachRequest Handler (30 min)

**File:** `crates/master/src/server/handler.rs`

**Location:** Line 227 (after debug log, before session_id parse)

**Changes:**
```rust
// BEFORE (current):
debug!("Processing AttachRequest from {}: session_id={}", peer_addr, req.session_id);

// Parse session_id from string
let session_id = Uuid::parse_str(&req.session_id)

// AFTER (with auth):
debug!("Processing AttachRequest from {}: session_id={}", peer_addr, req.session_id);

// Verify JWT authentication (SRS §3.2.2)
let claims = verify_auth_token(auth_service, &req.auth_token)?;
debug!("Auth verified for user: {}", claims.sub);

// Parse session_id from string
let session_id = Uuid::parse_str(&req.session_id)
```

**Edge cases to handle:**
- Empty auth_token → AuthFailed
- Invalid JWT → AuthFailed  
- Expired JWT → AuthFailed
- Wrong signature → AuthFailed

**Test:** Create integration test with real JWT from Ed25519AuthService

---

### Task 2: Wire Auth into InputData Handler (30 min)

**File:** `crates/master/src/server/handler.rs`

**Location:** Line 281 (after debug log, before attached check)

**Changes:**
```rust
// BEFORE (current):
debug!("Processing InputData from {}: {} bytes", peer_addr, input.data.len());

// Ensure client is attached
let session_id = attached_session

// AFTER (with auth):
debug!("Processing InputData from {}: {} bytes", peer_addr, input.data.len());

// Verify JWT authentication (SRS §3.2.2)
verify_auth_token(auth_service, &input.auth_token)?;

// Ensure client is attached
let session_id = attached_session
```

**Note:** Don't need the claims for InputData, just verify token is valid.

**Test:** Test with missing token, expired token, invalid token

---

### Task 3: Wire Auth into ResizeRequest Handler (30 min)

**File:** `crates/master/src/server/handler.rs`

**Location:** Line 297 (after debug log, before attached check)

**Changes:**
```rust
// BEFORE (current):
debug!("Processing ResizeRequest from {}: {}x{}", peer_addr, resize.rows, resize.cols);

// Ensure client is attached
let session_id = attached_session

// AFTER (with auth):
debug!("Processing ResizeRequest from {}: {}x{}", peer_addr, resize.rows, resize.cols);

// Verify JWT authentication (SRS §3.2.2)
verify_auth_token(auth_service, &resize.auth_token)?;

// Ensure client is attached
let session_id = attached_session
```

**Test:** Test with valid/invalid tokens

---

### Task 4: Integration Tests (1 hour)

**File:** `crates/master/tests/auth_integration.rs`

**New tests to add:**

1. **test_attach_with_valid_jwt()**
   - Create auth service
   - Issue JWT tokens
   - Create AttachRequest with valid token
   - Verify attach succeeds

2. **test_attach_with_missing_jwt()**
   - Create AttachRequest with empty auth_token
   - Verify returns AuthFailed error

3. **test_attach_with_invalid_jwt()**
   - Create AttachRequest with garbage token
   - Verify returns AuthFailed error

4. **test_input_with_valid_jwt()**
   - Attach with valid token
   - Send InputData with valid token
   - Verify input accepted

5. **test_input_with_missing_jwt()**
   - Send InputData with empty token
   - Verify returns AuthFailed

6. **test_resize_with_valid_jwt()**
   - Attach with valid token
   - Send ResizeRequest with valid token
   - Verify resize accepted

7. **test_resize_with_expired_jwt()**
   - Create auth service
   - Issue token
   - Wait 16 minutes (past 15min TTL)
   - Send ResizeRequest
   - Verify returns AuthFailed

---

## Verification Checklist

After implementation:

- [ ] Code compiles with no warnings
- [ ] All 7 integration tests pass
- [ ] Manual test: AttachRequest without token fails
- [ ] Manual test: InputData without token fails
- [ ] Manual test: ResizeRequest without token fails
- [ ] Manual test: Valid JWT allows all operations
- [ ] Grep codebase: no TODO comments about auth left behind
- [ ] Code review: verify_auth_token called in all 3 handlers

---

## Timeline

- Task 1 (AttachRequest): 30 min
- Task 2 (InputData): 30 min
- Task 3 (ResizeRequest): 30 min
- Task 4 (Integration tests): 1 hour
- **Total:** 2.5 hours

---

## Dependencies Confirmed

✅ Protocol has auth_token fields:
- AttachRequest.auth_token (field 2)
- InputData.auth_token (field 2)
- ResizeRequest.auth_token (field 3)

✅ verify_auth_token helper exists:
- Location: `handler.rs:207`
- Signature: `fn verify_auth_token(auth_service: &dyn AuthService, token: &str) -> Result<Claims>`

---

## Rollback Plan

If anything fails:
1. Comment out auth verification calls
2. Code still compiles (auth is additive)
3. Report blocker to eng-director
4. No other systems blocked

---

**Status:** Ready to execute immediately when devops-lead signals "build compiles"

**ETA:** 2.5 hours from that signal to completion
