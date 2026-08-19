# Auth Integration Code Snippets
**Evidence for Criterion #4 Verification**

## 1. Ed25519 Key Generation & Storage

### File: `crates/master/src/auth/keys.rs`

```rust
/// Load or generate Ed25519 keypair
///
/// Storage pattern (SRS §3.2.2):
/// - Private key: ~/.monoterminal/identity.key (0600 permissions)
/// - Public key: Derived from private (no separate storage)
pub fn load_or_generate_keypair() -> Result<Ed25519KeyPair> {
    let key_path = get_identity_key_path()?;
    
    if key_path.exists() {
        load_keypair(&key_path)
    } else {
        let keypair = Ed25519KeyPair::generate();
        save_keypair(&key_path, &keypair)?;
        Ok(keypair)
    }
}

/// Save keypair to file with secure permissions
fn save_keypair(path: &Path, keypair: &Ed25519KeyPair) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .context("Failed to create identity key file")?;
    
    // Set file permissions to 0600 (owner rw only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms)
            .context("Failed to set key file permissions")?;
    }
    
    file.write_all(keypair.signing_bytes())?;
    file.sync_all()?;
    Ok(())
}
```

---

## 2. JWT Token Issuance with EdDSA

### File: `crates/master/src/auth/jwt.rs`

```rust
pub fn issue_tokens(&self, user_id: &UserId) -> Result<TokenPair> {
    let now = timestamp();
    
    let access = Claims {
        sub: user_id.0.clone(),
        iss: self.issuer.clone(),
        exp: now + 900,        // ✅ 15 minutes per SRS §3.2.2
        iat: now,
        scope: "session:attach session:create input:write".into(),
        jti: Some(gen_jti()),  // ✅ JTI for revocation support
    };
    
    let refresh = Claims {
        sub: user_id.0.clone(),
        iss: self.issuer.clone(),
        exp: now + 2592000,    // ✅ 30 days per SRS §3.2.2
        iat: now,
        scope: "token:refresh".into(),
        jti: Some(gen_jti()),  // ✅ JTI for reuse detection
    };
    
    Ok(TokenPair {
        access: self.build(&access)?,
        refresh: self.build(&refresh)?,
    })
}

fn build(&self, c: &Claims) -> Result<String> {
    // ✅ ADR-007: EdDSA (Ed25519) asymmetric signing
    encode(&Header::new(Algorithm::EdDSA), c, &self.enc)
        .map_err(|e| anyhow!("Encode failed: {}", e))
}
```

---

## 3. JWT Validation in WebSocket Handler

### File: `crates/master/src/server/handler.rs`

```rust
/// Verify JWT authentication token
/// SRS §3.2.2: Ed25519/JWT authentication with 15-minute access tokens
fn verify_auth_token(
    auth_service: &dyn AuthService,
    token: &str,
) -> Result<Claims> {
    auth_service
        .verify_access(token)
        .map_err(|e| ServerError::AuthFailed(format!("JWT verification failed: {}", e)))
}

// ✅ Example: AttachRequest handler (lines 232-248)
Some(envelope::Message::AttachRequest(req)) => {
    // SRS §3.2.2: JWT authentication verification
    if !dev_mode {
        if req.auth_token.is_empty() {
            return Err(ServerError::AuthFailed("Missing authentication token".to_string()));
        }
        
        let _claims = verify_auth_token(auth_service, &req.auth_token)?;
        debug!("JWT verified for AttachRequest from {}", peer_addr);
    } else {
        warn!("⚠️  DEV MODE: Skipping JWT verification for AttachRequest");
    }
    
    // ... process attach request
}

// ✅ Similar verification in:
// - InputData handler (lines 322-335)
// - ResizeRequest handler (lines 350-363)
```

---

## 4. Token Refresh with JTI Reuse Detection

### File: `crates/master/src/auth/jwt.rs`

```rust
pub fn refresh_access_token(&self, tok: &str) -> Result<TokenPair> {
    let c = self.parse(tok)?;
    
    // ✅ Verify it's a refresh token (scope validation)
    if c.scope != "token:refresh" {
        return Err(anyhow!("Not a refresh token"));
    }
    
    // ✅ JTI reuse detection (prevents token replay)
    let jti = c.jti.as_ref().ok_or(anyhow!("Missing JTI"))?;
    {
        let mut u = self.used.lock().unwrap();
        if u.contains(jti) {
            return Err(anyhow!("Reuse detected: {}", c.sub));
        }
        u.insert(jti.clone());
    }
    
    // ✅ Issue new token pair
    self.issue_tokens(&UserId(c.sub))
}
```

**Test Evidence:**
```rust
// From auth_integration.rs:95-109
#[test]
fn test_refresh_reuse_detection() {
    let auth_service = create_test_auth_service();
    let user_id = UserId::from("dave@example.com");
    let pair = auth_service.issue_tokens(&user_id).unwrap();
    
    // First refresh should succeed
    let refresh_1 = auth_service.refresh_access(&pair.refresh);
    assert!(refresh_1.is_ok(), "First refresh should succeed");
    
    // Second refresh with same token should fail (reuse detection)
    let refresh_2 = auth_service.refresh_access(&pair.refresh);
    assert!(refresh_2.is_err(), "Refresh reuse should be detected");
    assert!(refresh_2.unwrap_err().to_string().contains("Reuse detected"));
}
```

---

## 5. Ed25519 Challenge-Response Flow

### File: `crates/master/src/auth/challenge.rs`

```rust
/// Generate a new challenge with random nonce
pub fn create_challenge(&self) -> Challenge {
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    
    Challenge {
        nonce,
        expires_at: Instant::now() + self.challenge_ttl,  // ✅ 30 seconds default
    }
}

/// Verify a signed challenge response
pub fn verify_challenge_response(
    &self,
    challenge: &Challenge,
    signature: &Signature,
    public_key: &PublicKey,
) -> Result<UserId> {
    // ✅ Check challenge hasn't expired
    if challenge.is_expired() {
        return Err(anyhow!("Challenge expired"));
    }
    
    // ✅ Parse Ed25519 public key
    let verifying_key = VerifyingKey::from_bytes(public_key)
        .map_err(|e| anyhow!("Invalid public key: {}", e))?;
    
    // ✅ Verify signature against challenge nonce
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    verifying_key
        .verify(&challenge.nonce, &sig)
        .map_err(|e| anyhow!("Signature verification failed: {}", e))?;
    
    // ✅ Derive user ID from public key fingerprint (SHA-256 hash)
    let user_id = derive_user_id_from_pubkey(public_key);
    Ok(user_id)
}

/// Derive a stable user ID from Ed25519 public key
fn derive_user_id_from_pubkey(public_key: &PublicKey) -> UserId {
    use sha2::{Digest, Sha256};
    
    let mut hasher = Sha256::new();
    hasher.update(public_key);
    let hash = hasher.finalize();
    
    // Use first 16 bytes of hash as hex string (32 hex chars)
    let hex_str = hex::encode(&hash[..16]);
    UserId(format!("ed25519:{}", hex_str))
}
```

---

## 6. Rate Limiting Implementation

### File: `crates/master/src/auth/rate_limit.rs`

```rust
/// Check if connection from peer is allowed (100/min limit per SRS §3.2.4)
pub fn check_connection(&self, peer_addr: &SocketAddr) -> Result<(), RateLimitError> {
    let mut buckets = self.connection_buckets.lock().unwrap();
    let bucket = buckets.entry(*peer_addr).or_insert_with(|| {
        TokenBucket::new(
            self.max_connections_per_minute,  // ✅ 100 per SRS
            self.max_connections_per_minute,
            Duration::from_secs(60),
        )
    });
    
    if bucket.try_acquire(1) {
        Ok(())
    } else {
        Err(RateLimitError::Exceeded(format!(
            "Connection rate limit exceeded for {}", peer_addr
        )))
    }
}

/// Record an auth failure (triggers ban after 5 failures/hour per SRS §3.2.4)
pub fn record_auth_failure(&self, peer_addr: &SocketAddr) {
    let mut trackers = self.auth_trackers.lock().unwrap();
    let tracker = trackers.entry(*peer_addr).or_insert_with(AuthFailureTracker::new);
    tracker.record_failure();
}

// From AuthFailureTracker implementation:
fn record_failure(&mut self) {
    let now = Instant::now();
    self.failures.push(now);
    
    // Keep only failures from last hour
    let one_hour_ago = now - Duration::from_secs(3600);
    self.failures.retain(|&t| t > one_hour_ago);
    
    // ✅ Check if should ban (5 failures in 1 hour per SRS)
    if self.failures.len() >= 5 {
        self.ban_until = Some(now + Duration::from_secs(900));  // ✅ 15 min ban
    }
}
```

---

## 7. Full Auth Flow Integration Test

### File: `crates/master/tests/auth_integration.rs`

```rust
#[test]
fn test_full_auth_flow_ed25519_to_jwt() {
    use ed25519_dalek::{Signer, SigningKey};
    use monoterminal_master::auth::Ed25519ChallengeHandler;
    
    let auth_service = create_test_auth_service();
    let challenge_handler = Ed25519ChallengeHandler::new();
    
    // ✅ Step 1: Generate challenge
    let challenge = challenge_handler.create_challenge();
    
    // ✅ Step 2: Client signs challenge
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let signature = signing_key.sign(&challenge.nonce);
    
    // ✅ Step 3: Verify signature and get user ID
    let user_id = challenge_handler
        .verify_challenge_response(
            &challenge,
            &signature.to_bytes(),
            signing_key.verifying_key().as_bytes(),
        )
        .expect("Signature verification failed");
    
    assert!(user_id.as_ref().starts_with("ed25519:"));
    
    // ✅ Step 4: Issue JWT for authenticated user
    let pair = auth_service.issue_tokens(&user_id)
        .expect("Failed to issue after challenge-response");
    
    // ✅ Step 5: Verify access works
    let claims = auth_service.verify_access(&pair.access).unwrap();
    assert_eq!(claims.sub, user_id.as_ref());
}
```

---

## 8. Browser Auth Integration (Task-4)

### File: `web/src/lib/auth/index.ts`

```typescript
/**
 * Sign a challenge received from the server
 * 
 * @param challengeData - Challenge data from server (JSON)
 * @returns Serialized challenge response (signature + public key as base64)
 */
async signChallenge(challengeData: any): Promise<{ signature: string; publicKey: string }> {
  if (!this.keypair) {
    throw new Error('Auth service not initialized. Call initialize() first.');
  }
  
  const challenge = parseChallenge(challengeData);
  const response = await signChallenge(
    challenge, 
    this.keypair.privateKey, 
    this.keypair.publicKey
  );
  return serializeChallengeResponse(response);
}
```

### File: `web/src/lib/auth/keys.ts`

```typescript
/**
 * Generate a new Ed25519 keypair using cryptographically secure random
 */
export async function generateKeypair(): Promise<Ed25519Keypair> {
  // ✅ Generate random 32-byte private key
  const privateKey = ed25519.utils.randomPrivateKey();
  
  // ✅ Derive public key from private key
  const publicKey = await ed25519.getPublicKeyAsync(privateKey);
  
  return { publicKey, privateKey };
}

/**
 * Load or generate Ed25519 keypair for the default identity
 * - Loads from IndexedDB if exists
 * - Generates new keypair if not found
 * - Stores the new keypair in IndexedDB
 */
export async function loadOrGenerateKeypair(id: string = 'default'): Promise<Ed25519Keypair> {
  const stored = await loadKeypair(id);
  
  if (stored) {
    console.log(`Loaded existing Ed25519 keypair (id: ${id})`);
    return { publicKey: stored.publicKey, privateKey: stored.privateKey };
  }
  
  console.log(`Generating new Ed25519 keypair (id: ${id})`);
  const keypair = await generateKeypair();
  await storeKeypair(id, keypair.publicKey, keypair.privateKey);
  return keypair;
}
```

---

## Summary

All code snippets above demonstrate:

1. ✅ **Ed25519 key generation** with secure storage (0600 permissions)
2. ✅ **JWT issuance** with correct claims (15min access, 30day refresh)
3. ✅ **JWT validation** integrated into WebSocket handlers
4. ✅ **Token refresh** with JTI reuse detection
5. ✅ **Challenge-response** flow with Ed25519 signature verification
6. ✅ **Rate limiting** per SRS §3.2.4
7. ✅ **Full integration** from challenge → signature → JWT → validation
8. ✅ **Browser integration** using @noble/ed25519 and IndexedDB

**RBAC is intentionally not shown** (deferred to Phase 2 per architectural decision).
