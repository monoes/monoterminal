# MONOTERMINAL Authentication Flow

**Version:** Phase 1 (Windows + Web)  
**SRS Reference:** §3.2.2 (Ed25519 SSH Keys + JWT Authentication)  
**ADR Reference:** ADR-007 (EdDSA Algorithm for Phase 1)

---

## Overview

MONOTERMINAL uses a two-stage authentication system:

1. **Ed25519 Challenge-Response** - Proves client possesses private key
2. **JWT Access/Refresh Pair** - Stateless session authentication

This document describes the complete authentication flow from client connection through session access.

---

## Ed25519 Key Generation

### First Run Setup

On first run, the master daemon generates an Ed25519 keypair:

**Location:** `~/.monoterminal/identity.key`  
**Permissions:** `0600` (owner read/write only)  
**Format:** 32-byte Ed25519 private key (raw bytes)  
**Public Key:** Derived from private key (no separate storage)

### Key Generation Code

```rust
use monoterminal_master::auth::load_or_generate_keypair;

// Automatically loads existing or generates new keypair
let keypair = load_or_generate_keypair()?;
```

**Security:**
- Uses `rand::OsRng` for cryptographically secure randomness
- Directory created with `0700` permissions
- Key file created with `0600` permissions
- Only owner can read/write the private key

---

## Authentication Flow

### Stage 1: Challenge-Response (Ed25519)

**Purpose:** Prove client possesses Ed25519 private key

```
Client                          Master Daemon
  |                                  |
  |-- 1. Connect (WebSocket) ------→|
  |                                  |
  |←- 2. Challenge (32-byte nonce) -|
  |                                  |
  |                                  |
  | 3. Sign challenge with private key
  |    signature = sign(nonce)      |
  |                                  |
  |-- 4. Response (signature) -----→|
  |                                  |
  |                                  | 5. Verify signature
  |                                  |    with public key
  |                                  |
  |←- 6. JWT Pair (Access+Refresh) -|
  |                                  |
```

**Steps:**

1. **Client connects** via TLS 1.3 WebSocket
2. **Server sends challenge:** 32-byte random nonce
3. **Client signs challenge:** Uses Ed25519 private key
4. **Client returns signature:** 64-byte Ed25519 signature
5. **Server verifies signature:** Using client's Ed25519 public key
6. **Server issues JWT pair:** Access (15min) + Refresh (30d)

---

### Stage 2: JWT Session Authentication

**Purpose:** Stateless session access without re-signing challenges

```
Client                          Master Daemon
  |                                  |
  |-- Request + Access JWT --------→|
  |                                  |
  |                                  | Verify JWT signature
  |                                  | Check expiration
  |                                  | Validate scopes
  |                                  |
  |←- Response -------------------→-|
  |                                  |
```

**JWT Format (Access):**

```json
{
  "header": {
    "typ": "JWT",
    "alg": "EdDSA"
  },
  "payload": {
    "sub": "ed25519:abc123...",
    "iss": "monoterminal-master",
    "exp": 1692201600,
    "iat": 1692200700,
    "scope": "session:attach session:create input:write"
  }
}
```

**JWT Properties:**

| Property | Access | Refresh |
|----------|--------|---------|
| TTL | 15 minutes (900s) | 30 days (2592000s) |
| Scope | `session:*`, `input:write` | `refresh` only |
| JTI | None | Yes (for reuse detection) |
| Purpose | Session operations | Renewal only |

---

## Refresh Flow

**Purpose:** Obtain new access JWT without re-authenticating

```
Client                          Master Daemon
  |                                  |
  |-- Refresh JWT ----------------→|
  |                                  |
  |                                  | Verify refresh JWT
  |                                  | Check reuse (JTI)
  |                                  | Mark JTI as used
  |                                  |
  |←- New JWT Pair ---------------→|
  |                                  |
```

**Refresh Reuse Detection:**

- Each refresh JWT has unique `jti` (JWT ID)
- On first use: Mark JTI as used, issue new pair
- On reuse: **Security breach** → Revoke ALL for that user

This prevents stolen refresh detection attacks.

---

## Algorithm: EdDSA (Ed25519)

**Why EdDSA vs HMAC?**

| Property | EdDSA (Chosen) | HMAC (Rejected) |
|----------|----------------|-----------------|
| Key Type | Asymmetric | Symmetric |
| Signing | Private key only | Shared secret |
| Verification | Public key (shareable) | Same secret |
| Forgery | Impossible without private | Possible if secret leaked |
| Phase 2 P2P | ✅ Works | ❌ Fails |

**Per ADR-007:** EdDSA is mandatory for Phase 1 because:
1. SRS §3.2.2 explicitly specifies `"alg": "EdDSA"`
2. Phase 2 P2P requires asymmetric verification
3. No migration cost (nothing deployed yet)

---

## Security Properties

### Asymmetric Verification (Phase 2 Ready)

**Server:**
- Private key (signs JWTs)
- Never shared

**Clients (Phase 2 P2P):**
- Public key (verifies JWTs)
- Cannot forge JWTs
- Can verify peer signatures

**Example (Phase 2 P2P):**
```
Client A ──signs JWT with private A──→ Client B
Client B ──verifies with public A────→ ✅ Valid
Client B ──tries to forge as A──────→ ❌ Fails (no private A)
```

This asymmetric property is **essential for Phase 2 P2P** where clients must verify each other's JWTs without being able to forge them.

---

## Scopes

**Access scopes (permissions):**

| Scope | Description |
|-------|-------------|
| `session:attach` | Attach to existing session |
| `session:create` | Create new session |
| `input:write` | Send input to session |
| `session:resize` | Resize session viewport |

**Refresh scopes:**
- `refresh` - Can only refresh access

---

## Implementation Example

### Server Side (Master Daemon)

```rust
use monoterminal_master::auth::{
    load_or_generate_keypair,
    Ed25519AuthService,
};

// Initialize auth service
let keypair = load_or_generate_keypair()?;
let auth_service = Ed25519AuthService::new(&keypair)?;

// Challenge-response flow
let challenge = auth_service.create_challenge();
// ... send to client, receive signature ...
let user_id = auth_service.verify_challenge_response(
    &challenge,
    &signature,
    &client_public_key
)?;

// Issue JWT pair
let pair = auth_service.issue_tokens(&user_id)?;
// pair.access = "eyJhbGc..."
// pair.refresh = "eyJhbGc..."

// Later: Verify access
let claims = auth_service.verify_access(&pair.access)?;
assert_eq!(claims.sub, user_id.as_ref());
```

### Client Side (Web/Browser)

```typescript
// Phase 1: Web client trusts server (localhost only)
// Phase 2: Will verify JWTs with server's public key

async function authenticateWithServer(privateKey: Uint8Array) {
  // 1. Connect to server
  const ws = new WebSocket('wss://127.0.0.1:5000');

  // 2. Receive challenge
  const challenge = await receiveChallenge(ws);

  // 3. Sign challenge with Ed25519 private key
  const signature = await ed25519.sign(challenge, privateKey);

  // 4. Send signature
  await sendSignature(ws, signature);

  // 5. Receive JWT pair
  const { accessJWT, refreshJWT } = await receiveJWTs(ws);

  // 6. Store for session use
  localStorage.setItem('access', accessJWT);
  localStorage.setItem('refresh', refreshJWT);
}
```

---

## Phase 2 P2P Considerations

**Current (Phase 1):**
- Web client trusts master daemon (localhost)
- No need to verify JWT signature client-side

**Future (Phase 2 P2P):**
- Clients connect directly (WebRTC DataChannel)
- Client A signs JWT, Client B verifies
- EdDSA enables this: public key shareable, private key protected

**Migration:** None needed - Phase 1 EdDSA is Phase 2 ready.

---

## References

- **SRS:** §3.2.2 (Ed25519 SSH Keys + JWT Authentication)
- **ADR-007:** EdDSA Algorithm for Phase 1 Authentication
- **RFC 8032:** Edwards-Curve Digital Signature Algorithm (EdDSA)
- **RFC 7519:** JSON Web (JWT)
- **Crate:** `ed25519-dalek = "2"`
- **Crate:** `jsonwebtoken = "9"`
