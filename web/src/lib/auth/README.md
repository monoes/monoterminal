# Browser Ed25519 Authentication Module

**SRS Reference:** §3.2.2 - Client-side Ed25519 authentication  
**Implementation:** Phase 1 - Ed25519 keypair management

## Overview

This module implements browser-based Ed25519 authentication for MONOTERMINAL web client, providing:

- Ed25519 keypair generation using `@noble/ed25519`
- Secure keypair storage in IndexedDB
- Challenge-response authentication flow
- JWT management

## Architecture

```
auth/
├── storage.ts      - IndexedDB keypair persistence
├── keys.ts         - Ed25519 key generation and signing
├── challenge.ts    - Challenge-response protocol
├── index.ts        - Main AuthService API
└── *.test.ts       - Comprehensive unit tests
```

## Quick Start

### 1. Install Dependencies

```bash
npm install @noble/ed25519
npm install -D fake-indexeddb  # For testing
```

### 2. Initialize Auth Service

```typescript
import { createAuthService } from './lib/auth';

// Initialize with default keypair ID
const auth = await createAuthService();

// Or with custom ID
const auth = await createAuthService('my-custom-keypair');
```

### 3. Get Public Key

```typescript
// Raw bytes (32 bytes)
const publicKey = auth.getPublicKey();

// Hex fingerprint (64 chars)
const fingerprint = auth.getPublicKeyFingerprint();

// Base64 (for transmission)
const base64 = auth.getPublicKeyBase64();
```

### 4. Sign Challenge

```typescript
// Receive challenge from server
const challengeData = {
  nonce: "base64-encoded-32-byte-nonce",
  expiresAt: 1692345678000
};

// Sign it
const response = await auth.signChallenge(challengeData);
// Returns: { signature: "base64", publicKey: "base64" }
```

### 5. Store JWT

```typescript
// After successful authentication
const authString = "eyJhbGc..."; // Example from server
auth.setJWT(authString, 900); // 900 seconds = 15 minutes

// Check authentication status
if (auth.isAuthenticated()) {
  const jwt = auth.getJWT();
  // Use in API requests
}

// Check time remaining
const remaining = auth.getJWTTimeRemaining(); // seconds
```

## Complete Authentication Flow

```typescript
import { createAuthService } from './lib/auth';
import { WebSocketClient } from './lib/websocket-client';

async function authenticate() {
  // 1. Initialize auth service
  const auth = await createAuthService();
  
  // 2. Connect to server (WebSocket/HTTP)
  const ws = new WebSocketClient({ url: 'wss://localhost:8080' });
  ws.connect();
  
  // 3. Request challenge from server
  const challenge = await requestChallenge(ws);
  // Server returns: { nonce: "base64", expiresAt: timestamp }
  
  // 4. Sign challenge
  const response = await auth.signChallenge(challenge);
  
  // 5. Send signed response to server
  const result = await sendChallengeResponse(ws, response);
  // Server verifies signature and returns JWT
  
  // 6. Store JWT
  auth.setJWT(result.access, 900);
  
  // 7. Use JWT in subsequent requests
  ws.config.jwtAuth = auth.getJWT()!;
  ws.attach('session-123', 24, 80);
}
```

## API Reference

### `AuthService`

Main authentication service class.

#### Methods

##### `async initialize(keypairId?: string): Promise<void>`
Initialize service with keypair (loads existing or generates new).

##### `getPublicKey(): Uint8Array`
Get Ed25519 public key (32 bytes).

##### `getPublicKeyFingerprint(): string`
Get hex-encoded fingerprint (64 chars).

##### `getPublicKeyBase64(): string`
Get base64-encoded public key.

##### `async signChallenge(challengeData: any): Promise<{ signature: string; publicKey: string }>`
Sign server challenge and return base64-encoded response.

##### `setJWT(jwt: string, expiresIn?: number): void`
Store JWT with expiration (default: 900s).

##### `getJWT(): string | null`
Get current JWT (returns null if expired/missing).

##### `isAuthenticated(): boolean`
Check if user has valid JWT.

##### `getJWTTimeRemaining(): number | null`
Get seconds until JWT expiration.

##### `clearJWT(): void`
Clear JWT (logout).

##### `async reset(keypairId?: string): Promise<void>`
Reset service and delete keypair.

### Helper Functions

#### `async createAuthService(keypairId?: string): Promise<AuthService>`
Create and initialize auth service in one call.

#### `async generateKeypair(): Promise<Ed25519Keypair>`
Generate new Ed25519 keypair.

#### `async sign(message: Uint8Array, privateKey: Uint8Array): Promise<Uint8Array>`
Sign message with Ed25519 private key.

#### `async verify(signature: Uint8Array, message: Uint8Array, publicKey: Uint8Array): Promise<boolean>`
Verify Ed25519 signature.

## Storage

Keypairs are stored in IndexedDB:

- **Database:** `monoterminal-auth`
- **Store:** `keypairs`
- **Key:** User-provided ID (default: `'default'`)

### Stored Data

```typescript
interface StoredKeypair {
  id: string;
  publicKey: Uint8Array;     // 32 bytes
  privateKey: Uint8Array;    // 32 bytes
  createdAt: number;         // Unix timestamp (ms)
  lastUsed: number;          // Unix timestamp (ms)
}
```

### Manual Storage Operations

```typescript
import { storeKeypair, loadKeypair, deleteKeypair, listKeypairs } from './lib/auth/storage';

// Store
await storeKeypair('my-id', publicKey, privateKey);

// Load
const keypair = await loadKeypair('my-id');

// Delete
await deleteKeypair('my-id');

// List all (without private keys)
const list = await listKeypairs();
```

## Security Considerations

### Private Key Protection

- ✅ Private keys stored in IndexedDB (more secure than localStorage)
- ✅ Keys never leave the browser except when signing
- ✅ IndexedDB is origin-isolated (per-domain)
- ❌ Keys accessible to JavaScript (not hardware-isolated)
- ❌ Vulnerable to XSS attacks (sanitize all user input)

### Best Practices

1. **Use HTTPS only** - Keys transmitted over WebSocket need TLS 1.3
2. **Implement CSP** - Content Security Policy to prevent XSS
3. **Rotate keys periodically** - Delete old keypairs after rotation
4. **Handle expiration** - JWT expires after 15 minutes, re-authenticate
5. **Clear on logout** - Call `auth.clearJWT()` on user logout

## Testing

Run the test suite:

```bash
npm test -- auth
```

### Test Coverage

- ✅ Keypair generation and uniqueness
- ✅ Message signing and verification
- ✅ Challenge parsing and expiration
- ✅ IndexedDB storage operations
- ✅ JWT lifecycle management
- ✅ Error handling and edge cases

### Mock IndexedDB

Tests use `fake-indexeddb` for IndexedDB operations:

```typescript
import 'fake-indexeddb/auto';  // in test setup
```

## Browser Compatibility

**Supported browsers** (per SRS §1.2):

- ✅ Chrome 90+ (desktop & Android)
- ✅ Firefox 88+
- ✅ Safari 14+ (desktop & iOS)
- ✅ Edge 90+

**Why @noble/ed25519?**

- WebCrypto API lacks Ed25519 support in many browsers
- Pure TypeScript implementation (~5KB minified)
- Well-audited, maintained by reputable cryptographer
- Works in all target browsers

## Performance

**Ed25519 operations:**

- Key generation: ~1-5ms
- Signing: ~0.5-2ms (50µs theoretical, browser overhead adds latency)
- Verification: ~1-3ms (100µs theoretical)

**IndexedDB operations:**

- Store: ~5-20ms
- Load: ~2-10ms
- Delete: ~2-10ms

## Troubleshooting

### "Auth service not initialized"

```typescript
const auth = new AuthService();
await auth.initialize();  // ← Don't forget this!
```

Or use the helper:

```typescript
const auth = await createAuthService();  // Already initialized
```

### "Challenge has expired"

Challenges expire after 30 seconds (server-side). Request a new challenge if:

```typescript
import { isChallengeExpired } from './lib/auth';

if (isChallengeExpired(challenge)) {
  // Request new challenge
  challenge = await requestChallenge();
}
```

### IndexedDB blocked in private/incognito mode

Some browsers block IndexedDB in private mode. Fallback:

```typescript
try {
  const auth = await createAuthService();
} catch (error) {
  if (error.message.includes('database')) {
    // Fallback: store in memory only (lost on page refresh)
    console.warn('IndexedDB unavailable, using in-memory storage');
  }
}
```

## Future Enhancements

**Phase 2+** (not in this implementation):

- [ ] Refresh rotation
- [ ] Multiple keypair support (user can have multiple identities)
- [ ] Hardware security key integration (WebAuthn)
- [ ] Keypair import/export (backup/restore)
- [ ] Key derivation from passphrase (PBKDF2)

## Related Files

- `web/src/lib/websocket-client.ts` - WebSocket transport (uses JWT)
- `crates/master/src/auth/` - Server-side authentication
- `docs/monoterminal-srs.md` - §3.2.2 Security specification

## License

Part of MONOTERMINAL project.
