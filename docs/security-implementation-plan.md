# Security Architecture Implementation Plan
**Task:** task-11  
**Owner:** security-engineer  
**Status:** Blocked on task-1 (architecture)  
**Estimated Effort:** 4-5 days after unblock  

## Overview

Implementation of security layer per SRS §3.2 for MONOTERMINAL Phase 1 (Windows master + Web client).

## Components

### 1. Ed25519 Authentication (SRS §3.2.2)

**Crate:** `ed25519-dalek = "2"`

**Status:** ✅ Implemented (ADR-007)

**Files Implemented:**
- `auth/keys.rs` - Ed25519 key generation, storage, loading
- `auth/challenge.rs` - Challenge-response flow
- `auth/jwt.rs` - JWT with EdDSA signing

**Implementation Details:**

```rust
// Key generation and management (auth/keys.rs)
pub fn load_or_generate_keypair() -> Result<Ed25519KeyPair>
pub struct Ed25519KeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

// Storage location: ~/.monoterminal/identity.key (ADR-007)
// Auto-generates on first run if not found
// Permissions: 0600 (owner read/write only)

// Challenge-response (auth/challenge.rs)
pub fn create_challenge() -> Challenge  // 256-bit random nonce
pub fn verify_challenge_response(
    challenge: &Challenge,
    signature: &[u8; 64],
    public_key: &[u8; 32]
) -> Result<UserId>
```

**Security Requirements:** ✅ Implemented
- ✅ 256-bit random challenge using `rand::OsRng`
- ✅ Private key file permissions: 0600 (owner read/write only)
- ✅ Public key derived from private (no separate storage needed)
- ✅ Constant-time comparison (built into ed25519-dalek)

**Tests:**
- ✅ Key generation produces valid keypair
- ✅ Public key can verify signature created by private key
- ✅ Invalid signature fails verification
- ✅ Tampered challenge fails verification
- ✅ Different keypair cannot verify signature

### 2. JWT Implementation (SRS §3.2.2)

**Crate:** `jsonwebtoken = "9"`

**Files to Create:**
- `auth/jwt.rs` - Token generation, validation, rotation
- `auth/claims.rs` - JWT claims structure

**Implementation Details:**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // user@example.com
    pub iss: String,           // "monoterminal-master"
    pub exp: i64,              // Unix timestamp
    pub iat: i64,              // Issued at
    pub scope: String,         // Space-separated permissions
    pub token_type: TokenType, // Access or Refresh
    pub jti: String,           // Token ID for refresh token tracking
}

pub enum TokenType {
    Access,  // 15 minutes TTL
    Refresh, // 30 days TTL
}

// Token management
pub fn issue_tokens(user_id: &str, scope: &str) -> Result<(String, String)>
pub fn validate_access_token(token: &str) -> Result<Claims>
pub fn rotate_refresh_token(old_token: &str) -> Result<(String, String)>
```

**Token TTLs:**
- Access token: 900 seconds (15 minutes)
- Refresh token: 2592000 seconds (30 days)

**Refresh Token Rotation:**
1. Validate old refresh token
2. Check if token ID exists in used-tokens set (reuse detection)
3. If reused → revoke ALL tokens for that user (security breach)
4. If valid → issue new access + refresh, mark old refresh as used
5. Store used token IDs in SQLite with TTL expiry (30 days)

**Algorithm:** ✅ `EdDSA` (Ed25519 asymmetric signing per ADR-007)
- Private key for signing (server only)
- Public key for verification (shareable for Phase 2 P2P)

**Tests:**
- ✅ Access token expires after 15 minutes
- ✅ Refresh token expires after 30 days
- ✅ Invalid signature fails validation
- ✅ Expired token fails validation
- ✅ Token rotation produces new valid tokens
- ✅ Refresh token reuse detection triggers revocation
- ✅ Scope validation works correctly

### 3. TLS 1.3 Configuration (SRS §3.2.1)

**Crate:** `rustls = "0.21"`

**Files to Create:**
- `tls/config.rs` - TLS configuration builder
- `tls/certs.rs` - Certificate management
- `tls/self_signed.rs` - Development self-signed cert generation

**Implementation Details:**

```rust
use rustls::{ServerConfig, cipher_suite};

pub fn build_tls_config(cert_path: &Path, key_path: &Path) -> Result<ServerConfig> {
    let config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&rustls::version::TLS13])? // TLS 1.3 ONLY
        .with_no_client_auth()
        .with_single_cert(certs, private_key)?;
    
    Ok(config)
}

// Cipher suite preference order (SRS §3.2.1)
const CIPHER_SUITES: &[SupportedCipherSuite] = &[
    cipher_suite::TLS13_AES_256_GCM_SHA384,       // Strongest
    cipher_suite::TLS13_AES_128_GCM_SHA256,       // Default
    cipher_suite::TLS13_CHACHA20_POLY1305_SHA256, // Mobile optimized
];
```

**Certificate Management:**
- **Development:** Self-signed cert (TOFU - Trust On First Use)
- **Production:** Let's Encrypt via ACME (future Phase 2+)

**Self-Signed Cert Generation:**
```rust
pub fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>)> {
    // Uses rcgen crate
    // Valid for 365 days
    // Subject: CN=monoterminal-master
}
```

**Tests:**
- ✅ TLS 1.3 handshake succeeds
- ✅ TLS 1.2 handshake is rejected
- ✅ Cipher suite negotiation follows preference order
- ✅ Self-signed cert generation produces valid cert
- ✅ Certificate loading from files works

### 4. RBAC Foundation (SRS §3.2.3)

**Files to Create:**
- `auth/rbac.rs` - Role definitions and permission checking
- `auth/permissions.rs` - Permission types and session-level permissions

**Implementation Details:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Admin,      // session:*, client:*, config:write
    User,       // session:attach, session:create, session:resize, input:write
    ReadOnly,   // session:attach, input:none
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    Owner,      // Full control
    ReadWrite,  // Attach, input, resize
    ReadOnly,   // Attach only, no input
}

pub struct SessionPermissions {
    pub owner_uid: u32,
    pub allowed_users: HashMap<u32, Permission>,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Attach,
    Input,
    Resize,
    Kill,
    ConfigWrite,
}

pub fn check_permission(
    session: &SessionPermissions,
    user_uid: u32,
    action: Action
) -> Result<(), PermissionError> {
    // Implementation per SRS §3.2.3
}
```

**Permission Matrix:**

| Action | Owner | ReadWrite | ReadOnly |
|--------|-------|-----------|----------|
| Attach | ✅ | ✅ | ✅ |
| Input | ✅ | ✅ | ❌ |
| Resize | ✅ | ✅ | ❌ |
| Kill | ✅ | ❌ | ❌ |
| ConfigWrite | ✅ | ❌ | ❌ |

**Tests:**
- ✅ Owner can perform all actions
- ✅ ReadWrite can attach, input, resize but not kill
- ✅ ReadOnly can only attach
- ✅ User without permission is denied
- ✅ Permission upgrade works correctly
- ✅ Permission downgrade works correctly

### 5. Rate Limiting (SRS §3.2.4)

**Crate:** `tower = "0.4"`

**Files to Create:**
- `ratelimit/mod.rs` - Rate limiter implementation
- `ratelimit/buckets.rs` - Token bucket algorithm
- `ratelimit/limits.rs` - Limit definitions

**Implementation Details:**

```rust
use tower::limit::RateLimit;

pub struct RateLimits {
    pub connections: TokenBucket,    // 100/min
    pub auth_attempts: TokenBucket,  // 5/hour per IP
    pub session_creates: TokenBucket, // 20/min
    pub input_rate: TokenBucket,     // 10 KB/s per session
}

pub struct TokenBucket {
    capacity: usize,
    tokens: AtomicUsize,
    refill_rate: Duration,
    last_refill: AtomicU64,
}

impl TokenBucket {
    pub fn try_acquire(&self, tokens: usize) -> bool
    pub fn refill(&self)
}

// Auth failure tracking for temp bans
pub struct AuthFailureTracker {
    failures: HashMap<IpAddr, Vec<Instant>>,
    ban_list: HashMap<IpAddr, Instant>, // IP -> unban time
}

impl AuthFailureTracker {
    pub fn record_failure(&mut self, ip: IpAddr)
    pub fn is_banned(&self, ip: IpAddr) -> bool
    pub fn should_ban(&self, ip: IpAddr) -> bool // 5 failures in 1 hour
}
```

**Rate Limits (SRS §3.2.4):**

| Resource | Limit | Window | Action on Exceed |
|----------|-------|--------|------------------|
| New Connections | 100 | 1 minute | Reject with 429 |
| Auth Attempts | 5 | 1 hour | 15 min temp ban |
| Session Creates | 20 | 1 minute | Reject with 429 |
| Input Rate | 10 KB/s | Per session | Drop excess |

**Tests:**
- ✅ Token bucket allows requests within limit
- ✅ Token bucket rejects requests exceeding limit
- ✅ Token bucket refills at correct rate
- ✅ Auth failure tracking records failures
- ✅ 5th auth failure triggers 15-minute ban
- ✅ Ban expires after 15 minutes
- ✅ Input rate limiting drops excess data

### 6. FIPS 140-2 Mode (SRS §3.2.5)

**Compile-time feature flag:** `fips-mode`

**Files to Create:**
- `fips/mod.rs` - FIPS mode configuration
- `Cargo.toml` feature flags

**Changes under FIPS mode:**
- TLS: Use `openssl` crate with FIPS-validated build (not rustls)
- Hashing: SHA-256/384/512 only (no BLAKE2, SHA-1)
- Random: `/dev/random` (blocking) instead of `/dev/urandom`
- Ciphers: AES-GCM only (no ChaCha20-Poly1305)

**Implementation:**
```toml
[features]
fips-mode = ["openssl"]

[dependencies]
openssl = { version = "0.10", optional = true, features = ["fips"] }
```

**Tests:**
- ✅ FIPS mode only allows approved algorithms
- ✅ Non-approved ciphers are rejected in FIPS mode
- ✅ FIPS mode uses /dev/random on Linux

## Module Structure (Pending Architecture Decision)

**Option A: Monolithic** (within master crate)
```
crates/master/src/
  security/
    auth/
      ed25519.rs
      jwt.rs
      challenge.rs
      rbac.rs
    tls/
      config.rs
      certs.rs
    ratelimit/
      mod.rs
```

**Option B: Modular** (separate crate)
```
crates/security/
  src/
    auth/
    tls/
    ratelimit/
  Cargo.toml
```

**Recommendation:** Option B (modular) for:
- Better separation of concerns
- Reusability if future clients need auth
- Easier testing in isolation
- Clear dependency boundaries

## Database Schema

```sql
-- Refresh token tracking (for reuse detection)
CREATE TABLE refresh_tokens (
    token_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked BOOLEAN DEFAULT FALSE
);
CREATE INDEX idx_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_tokens_expiry ON refresh_tokens(expires_at);

-- Auth failure tracking (for rate limiting)
CREATE TABLE auth_failures (
    ip_addr TEXT NOT NULL,
    attempt_time INTEGER NOT NULL,
    PRIMARY KEY (ip_addr, attempt_time)
);
CREATE INDEX idx_failures_ip ON auth_failures(ip_addr);

-- Session permissions (already in SRS schema)
CREATE TABLE session_permissions (
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    permission TEXT NOT NULL CHECK(permission IN ('owner', 'read_write', 'read_only')),
    granted_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, user_id),
    FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
);

-- Audit log (already in SRS schema)
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    session_id TEXT,
    details TEXT,
    result TEXT NOT NULL CHECK(result IN ('success', 'failure'))
);
```

## Integration Points

Security layer will provide:

1. **Authentication Middleware** for WebSocket handlers
2. **Permission Checker Trait** for authorization
3. **Rate Limiter as Tower Layer**
4. **TLS Config Builder** for server setup

## Testing Strategy

**Unit Tests:** Each component tested in isolation

**Integration Tests:**
- Full auth flow: challenge → signature → JWT issuance
- Token refresh with rotation and reuse detection
- TLS handshake with real connections
- Rate limit enforcement under load

**Security Tests:**
- Timing attack resistance (constant-time operations)
- Invalid input handling (malformed tokens, corrupted signatures)
- Replay attack prevention (refresh token reuse)
- Brute force protection (auth rate limiting)

**Coverage Target:** 80% (per SRS §6.1.4)

## Documentation to Create

1. **Security Threat Model** (Phase 1 scope)
2. **Certificate Management Guide**
3. **Auth Flow Documentation**
4. **Rate Limiting Guide**

## Dependencies

**Blocked on:**
- ✅ Task-1 (architecture) - module structure decision

**Blocks:**
- Task-9 (Protocol) - needs auth token structure in protobuf
- Task-12 (Web Client) - needs JWT handling and TLS config

## Timeline Estimate

Assuming unblocked and architecture decided:

- **Day 1:** Ed25519 auth + challenge-response
- **Day 2:** JWT implementation + refresh rotation
- **Day 3:** TLS 1.3 config + self-signed certs
- **Day 4:** RBAC + rate limiting
- **Day 5:** Integration tests + documentation

**Total: 4-5 days**

## Security Review Checklist

Before marking complete:

- [ ] All sensitive data are file-referenced, never inline
- [ ] TLS 1.3 ONLY enforcement verified
- [ ] No RSA fallback exists
- [ ] Refresh token reuse detection tested
- [ ] Rate limit bypass attempts fail
- [ ] Permission matrix fully tested
- [ ] Constant-time operations for crypto
- [ ] Audit logging for all auth events
- [ ] Threat model documented
- [ ] Security test coverage ≥80%

## Risk Register (Phase 1 Scope)

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| 0-day in rustls | HIGH | LOW | Monitor CVE feeds, rapid patch SLA |
| Key compromise | HIGH | MEDIUM | Key rotation procedure, audit logging |
| JWT signing key leak | HIGH | LOW | File permissions 0600, no git commits |
| Rate limit bypass | MEDIUM | MEDIUM | Multiple layers |
| FIPS compliance gap | LOW | LOW | Feature flag, external audit |

**0-day Response Plan:**
1. Monitor security advisories for all crypto dependencies
2. Patch SLA: Critical (24h), High (7d), Medium (30d)
3. Emergency release process for security patches

**Disclosure Policy:**
- Security contact: security@monoterminal.dev
- Response SLA: 48 hours
- Fix timeline: 30 days for coordinated disclosure
