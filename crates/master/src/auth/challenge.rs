// Ed25519 challenge-response authentication
// SRS §3.2.2: Challenge-response flow with 256-bit nonce

use super::{PublicKey, Signature, UserId};
use anyhow::{anyhow, Result};
use ed25519_dalek::{Verifier, VerifyingKey};
use rand::RngCore;
use std::time::{Duration, Instant};

/// Authentication challenge containing a random nonce
#[derive(Debug, Clone)]
pub struct Challenge {
    pub nonce: [u8; 32],
    pub expires_at: Instant,
}

impl Challenge {
    /// Check if challenge has expired
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }

    /// Get remaining time until expiration
    pub fn time_remaining(&self) -> Option<Duration> {
        self.expires_at.checked_duration_since(Instant::now())
    }
}

/// Challenge handler for Ed25519 authentication
pub struct Ed25519ChallengeHandler {
    challenge_ttl: Duration,
}

impl Ed25519ChallengeHandler {
    /// Create new challenge handler with default TTL (30 seconds)
    pub fn new() -> Self {
        Self {
            challenge_ttl: Duration::from_secs(30),
        }
    }

    /// Create new challenge handler with custom TTL
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { challenge_ttl: ttl }
    }

    /// Generate a new challenge with random nonce
    pub fn create_challenge(&self) -> Challenge {
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);

        Challenge {
            nonce,
            expires_at: Instant::now() + self.challenge_ttl,
        }
    }

    /// Verify a signed challenge response
    ///
    /// # Arguments
    /// * `challenge` - The original challenge that was sent to the client
    /// * `signature` - The Ed25519 signature from the client
    /// * `public_key` - The client's Ed25519 public key (32 bytes)
    ///
    /// # Returns
    /// * `Ok(UserId)` - The authenticated user ID (derived from public key fingerprint)
    /// * `Err(_)` - If verification fails (expired, invalid signature, etc.)
    pub fn verify_challenge_response(
        &self,
        challenge: &Challenge,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<UserId> {
        // Check challenge hasn't expired
        if challenge.is_expired() {
            return Err(anyhow!("Challenge expired"));
        }

        // Parse Ed25519 public key
        let verifying_key = VerifyingKey::from_bytes(public_key)
            .map_err(|e| anyhow!("Invalid public key: {}", e))?;

        // Parse signature
        let sig = ed25519_dalek::Signature::from_bytes(signature);

        // Verify signature against challenge nonce
        verifying_key
            .verify(&challenge.nonce, &sig)
            .map_err(|e| anyhow!("Signature verification failed: {}", e))?;

        // Derive user ID from public key fingerprint (SHA-256 hash)
        // This ensures one public key = one user identity
        let user_id = derive_user_id_from_pubkey(public_key);

        Ok(user_id)
    }
}

impl Default for Ed25519ChallengeHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Derive a stable user ID from Ed25519 public key
///
/// Uses SHA-256 hash of the public key to create a deterministic,
/// unique identifier. Same public key always produces same user ID.
fn derive_user_id_from_pubkey(public_key: &PublicKey) -> UserId {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(public_key);
    let hash = hasher.finalize();

    // Use first 16 bytes of hash as hex string (32 hex chars)
    let hex_str = hex::encode(&hash[..16]);
    UserId(format!("ed25519:{}", hex_str))
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn test_challenge_creation() {
        let handler = Ed25519ChallengeHandler::new();
        let challenge = handler.create_challenge();

        // Nonce should be random (not all zeros)
        assert_ne!(challenge.nonce, [0u8; 32]);

        // Challenge should not be expired immediately
        assert!(!challenge.is_expired());

        // Should have time remaining
        assert!(challenge.time_remaining().is_some());
    }

    #[test]
    fn test_challenge_expiration() {
        let handler = Ed25519ChallengeHandler::with_ttl(Duration::from_millis(10));
        let challenge = handler.create_challenge();

        assert!(!challenge.is_expired());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(15));

        assert!(challenge.is_expired());
        assert!(challenge.time_remaining().is_none());
    }

    #[test]
    fn test_valid_signature_verification() {
        let handler = Ed25519ChallengeHandler::new();
        let challenge = handler.create_challenge();

        // Generate keypair
        use rand::Rng;
        let secret_bytes: [u8; 32] = rand::thread_rng().gen();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        // Sign challenge
        let signature = signing_key.sign(&challenge.nonce);

        // Verify
        let result = handler.verify_challenge_response(
            &challenge,
            &signature.to_bytes(),
            verifying_key.as_bytes(),
        );

        assert!(result.is_ok());
        let user_id = result.unwrap();
        assert!(user_id.0.starts_with("ed25519:"));
    }

    #[test]
    fn test_invalid_signature_fails() {
        let handler = Ed25519ChallengeHandler::new();
        let challenge = handler.create_challenge();

        // Generate keypair
        use rand::Rng;
        let secret_bytes: [u8; 32] = rand::thread_rng().gen();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        // Create invalid signature (all zeros)
        let invalid_signature = [0u8; 64];

        // Verification should fail
        let result = handler.verify_challenge_response(
            &challenge,
            &invalid_signature,
            verifying_key.as_bytes(),
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("verification failed"));
    }

    #[test]
    fn test_wrong_keypair_fails() {
        let handler = Ed25519ChallengeHandler::new();
        let challenge = handler.create_challenge();

        // Generate two different keypairs
        use rand::Rng;
        let secret_bytes1: [u8; 32] = rand::thread_rng().gen();
        let signing_key1 = SigningKey::from_bytes(&secret_bytes1);
        let secret_bytes2: [u8; 32] = rand::thread_rng().gen();
        let signing_key2 = SigningKey::from_bytes(&secret_bytes2);

        // Sign with key1
        let signature = signing_key1.sign(&challenge.nonce);

        // Try to verify with key2's public key
        let result = handler.verify_challenge_response(
            &challenge,
            &signature.to_bytes(),
            signing_key2.verifying_key().as_bytes(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_expired_challenge_fails() {
        let handler = Ed25519ChallengeHandler::with_ttl(Duration::from_millis(10));
        let challenge = handler.create_challenge();

        // Generate keypair and sign
        use rand::Rng;
        let secret_bytes: [u8; 32] = rand::thread_rng().gen();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let signature = signing_key.sign(&challenge.nonce);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(15));

        // Verification should fail due to expiration
        let result = handler.verify_challenge_response(
            &challenge,
            &signature.to_bytes(),
            signing_key.verifying_key().as_bytes(),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[test]
    fn test_tampered_challenge_fails() {
        let handler = Ed25519ChallengeHandler::new();
        let mut challenge = handler.create_challenge();

        // Generate keypair and sign original challenge
        use rand::Rng;
        let secret_bytes: [u8; 32] = rand::thread_rng().gen();
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let signature = signing_key.sign(&challenge.nonce);

        // Tamper with challenge nonce
        challenge.nonce[0] ^= 0xFF;

        // Verification should fail
        let result = handler.verify_challenge_response(
            &challenge,
            &signature.to_bytes(),
            signing_key.verifying_key().as_bytes(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_user_id_derivation_deterministic() {
        // Same public key should always produce same user ID
        let pubkey = [0x42u8; 32];

        let user_id1 = derive_user_id_from_pubkey(&pubkey);
        let user_id2 = derive_user_id_from_pubkey(&pubkey);

        assert_eq!(user_id1, user_id2);
        assert!(user_id1.0.starts_with("ed25519:"));
    }

    #[test]
    fn test_different_pubkeys_different_user_ids() {
        let pubkey1 = [0x42u8; 32];
        let pubkey2 = [0x43u8; 32];

        let user_id1 = derive_user_id_from_pubkey(&pubkey1);
        let user_id2 = derive_user_id_from_pubkey(&pubkey2);

        assert_ne!(user_id1, user_id2);
    }
}
