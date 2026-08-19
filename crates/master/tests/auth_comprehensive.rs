// Comprehensive authentication tests for Ed25519 + JWT flow
// Covers: Challenge generation, signature verification, JWT issuance/refresh
// Target: High coverage for auth module (SRS §3.2)

use monoterminal_master::auth::*;
use std::time::Duration;

// ===== Challenge Tests =====

#[test]
fn test_challenge_creation() {
    let handler = Ed25519ChallengeHandler::new();
    let challenge = handler.create_challenge();

    // Nonce should be 32 bytes
    assert_eq!(challenge.nonce.len(), 32);

    // Should not be expired immediately
    assert!(!challenge.is_expired());

    // Should have time remaining
    assert!(challenge.time_remaining().is_some());
}

#[test]
fn test_challenge_custom_ttl() {
    let handler = Ed25519ChallengeHandler::with_ttl(Duration::from_secs(60));
    let challenge = handler.create_challenge();

    // Should have approximately 60 seconds remaining
    let remaining = challenge.time_remaining().unwrap();
    assert!(remaining.as_secs() >= 59 && remaining.as_secs() <= 60);
}

#[test]
fn test_challenge_nonces_are_unique() {
    let handler = Ed25519ChallengeHandler::new();
    let challenge1 = handler.create_challenge();
    let challenge2 = handler.create_challenge();

    // Two challenges should have different nonces
    assert_ne!(challenge1.nonce, challenge2.nonce);
}

#[test]
fn test_challenge_expiration() {
    let handler = Ed25519ChallengeHandler::with_ttl(Duration::from_millis(50));
    let challenge = handler.create_challenge();

    assert!(!challenge.is_expired());

    // Wait for expiration
    std::thread::sleep(Duration::from_millis(60));

    assert!(challenge.is_expired());
    assert!(challenge.time_remaining().is_none());
}

#[test]
fn test_expired_challenge_rejection() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    let handler = Ed25519ChallengeHandler::with_ttl(Duration::from_millis(10));
    let challenge = handler.create_challenge();

    // Wait for expiration
    std::thread::sleep(Duration::from_millis(20));

    // Generate valid signature
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let signature = signing_key.sign(&challenge.nonce);

    let public_key_bytes = signing_key.verifying_key().to_bytes();
    let signature_bytes = signature.to_bytes();

    // Should fail due to expiration
    let result = handler.verify_challenge_response(&challenge, &signature_bytes, &public_key_bytes);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("expired"));
}

#[test]
fn test_valid_signature_verification() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    let handler = Ed25519ChallengeHandler::new();
    let challenge = handler.create_challenge();

    // Generate key pair and sign challenge
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let signature = signing_key.sign(&challenge.nonce);

    let public_key_bytes = signing_key.verifying_key().to_bytes();
    let signature_bytes = signature.to_bytes();

    // Should succeed
    let result = handler.verify_challenge_response(&challenge, &signature_bytes, &public_key_bytes);
    assert!(result.is_ok());

    let user_id = result.unwrap();
    assert!(!user_id.as_ref().is_empty());
}

#[test]
fn test_invalid_signature_rejection() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    let handler = Ed25519ChallengeHandler::new();
    let challenge = handler.create_challenge();

    // Sign with one key
    let secret_bytes1: [u8; 32] = rand::thread_rng().gen();
    let signing_key1 = SigningKey::from_bytes(&secret_bytes1);
    let signature = signing_key1.sign(&challenge.nonce);

    // Verify with different key
    let secret_bytes2: [u8; 32] = rand::thread_rng().gen();
    let signing_key2 = SigningKey::from_bytes(&secret_bytes2);
    let public_key_bytes = signing_key2.verifying_key().to_bytes();
    let signature_bytes = signature.to_bytes();

    // Should fail
    let result = handler.verify_challenge_response(&challenge, &signature_bytes, &public_key_bytes);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("verification failed"));
}

#[test]
fn test_wrong_challenge_signed() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    let handler = Ed25519ChallengeHandler::new();
    let challenge1 = handler.create_challenge();
    let challenge2 = handler.create_challenge();

    // Sign challenge1 but verify against challenge2
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let signature = signing_key.sign(&challenge1.nonce);

    let public_key_bytes = signing_key.verifying_key().to_bytes();
    let signature_bytes = signature.to_bytes();

    // Should fail
    let result =
        handler.verify_challenge_response(&challenge2, &signature_bytes, &public_key_bytes);
    assert!(result.is_err());
}

#[test]
fn test_malformed_signature() {
    let handler = Ed25519ChallengeHandler::new();
    let challenge = handler.create_challenge();

    // Use a valid key format but wrong signature
    use ed25519_dalek::SigningKey;
    let secret_bytes = [1u8; 32];
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let public_key = signing_key.verifying_key().to_bytes();

    // Wrong signature (all zeros) for this challenge
    let wrong_signature = [0u8; 64];

    let result = handler.verify_challenge_response(&challenge, &wrong_signature, &public_key);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Signature verification failed"));
}

#[test]
fn test_same_pubkey_produces_same_user_id() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    let handler = Ed25519ChallengeHandler::new();

    // Generate key and sign two different challenges
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let public_key_bytes = signing_key.verifying_key().to_bytes();

    let challenge1 = handler.create_challenge();
    let signature1 = signing_key.sign(&challenge1.nonce);
    let user_id1 = handler
        .verify_challenge_response(&challenge1, &signature1.to_bytes(), &public_key_bytes)
        .unwrap();

    let challenge2 = handler.create_challenge();
    let signature2 = signing_key.sign(&challenge2.nonce);
    let user_id2 = handler
        .verify_challenge_response(&challenge2, &signature2.to_bytes(), &public_key_bytes)
        .unwrap();

    // Same key should produce same user ID
    assert_eq!(user_id1, user_id2);
}

// ===== JWT Tests =====

#[test]
fn test_jwt_service_creation() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair);
    assert!(service.is_ok());
}

#[test]
fn test_token_pair_issuance() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let tokens = service.issue_tokens(&user_id);
    assert!(tokens.is_ok());

    let token_pair = tokens.unwrap();
    assert!(!token_pair.access.is_empty());
    assert!(!token_pair.refresh.is_empty());
}

#[test]
fn test_access_token_verification() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // Verify access token
    let claims = service.verify_access_token(&token_pair.access);
    assert!(claims.is_ok());

    let claims = claims.unwrap();
    assert_eq!(claims.sub, "alice@example.com");
    assert!(claims.scope.contains("session:attach"));
    assert!(claims.jti.is_some()); // Access tokens now have JTI for revocation
}

#[test]
fn test_refresh_token_has_jti() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // Parse refresh token manually (would normally not expose this)
    // For testing, we verify it through refresh flow
    let new_tokens = service.refresh_access_token(&token_pair.refresh);
    assert!(new_tokens.is_ok());
}

#[test]
fn test_access_token_has_correct_scopes() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();
    let claims = service.verify_access_token(&token_pair.access).unwrap();

    assert!(claims.scope.contains("session:attach"));
    assert!(claims.scope.contains("session:create"));
    assert!(claims.scope.contains("input:write"));
}

#[test]
fn test_refresh_token_cannot_be_used_as_access() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // Try to use refresh token as access token
    let result = service.verify_access_token(&token_pair.refresh);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing session scope"));
}

#[test]
fn test_access_token_cannot_be_refreshed() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // Try to refresh with access token
    let result = service.refresh_access_token(&token_pair.access);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Not a refresh token"));
}

#[test]
fn test_token_refresh_produces_new_tokens() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair1 = service.issue_tokens(&user_id).unwrap();
    let token_pair2 = service.refresh_access_token(&token_pair1.refresh).unwrap();

    // Should produce different tokens
    assert_ne!(token_pair1.access, token_pair2.access);
    assert_ne!(token_pair1.refresh, token_pair2.refresh);

    // But for same user
    let claims1 = service.verify_access_token(&token_pair1.access).unwrap();
    let claims2 = service.verify_access_token(&token_pair2.access).unwrap();
    assert_eq!(claims1.sub, claims2.sub);
}

#[test]
fn test_refresh_token_reuse_detected() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // First refresh should work
    let result1 = service.refresh_access_token(&token_pair.refresh);
    assert!(result1.is_ok());

    // Second refresh with same token should fail
    let result2 = service.refresh_access_token(&token_pair.refresh);
    assert!(result2.is_err());
    assert!(result2.unwrap_err().to_string().contains("Reuse detected"));
}

#[test]
fn test_tampered_token_rejected() {
    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let service = JwtService::new(&keypair).unwrap();
    let user_id = UserId::from("alice@example.com");

    let token_pair = service.issue_tokens(&user_id).unwrap();

    // Tamper with token
    let mut tampered = token_pair.access.clone();
    tampered.push('x');

    let result = service.verify_access_token(&tampered);
    assert!(result.is_err());
}

#[test]
fn test_token_from_different_key_rejected() {
    let keypair1 = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let keypair2 = Ed25519KeyPair::from_bytes(b"different-key-32-bytes-long!!!!!");

    let service1 = JwtService::new(&keypair1).unwrap();
    let service2 = JwtService::new(&keypair2).unwrap();

    let user_id = UserId::from("alice@example.com");
    let token_pair = service1.issue_tokens(&user_id).unwrap();

    // service2 should reject service1's tokens
    let result = service2.verify_access_token(&token_pair.access);
    assert!(result.is_err());
}

// ===== Integration Tests =====

#[test]
fn test_full_auth_flow() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    // Setup
    let jwt_keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let auth_service = Ed25519AuthService::new(&jwt_keypair).unwrap();

    // Step 1: Client connects, server sends challenge
    let challenge = auth_service.create_challenge();

    // Step 2: Client signs challenge with private key
    let secret_bytes: [u8; 32] = rand::thread_rng().gen();
    let client_key = SigningKey::from_bytes(&secret_bytes);
    let signature = client_key.sign(&challenge.nonce);
    let public_key = client_key.verifying_key().to_bytes();

    // Step 3: Server verifies signature and gets user ID
    let user_id = auth_service
        .verify_challenge_response(&challenge, &signature.to_bytes(), &public_key)
        .unwrap();

    // Step 4: Server issues JWT tokens
    let tokens = auth_service.issue_tokens(&user_id).unwrap();

    // Step 5: Client uses access token for requests
    let claims = auth_service.verify_access(&tokens.access).unwrap();
    assert_eq!(claims.sub, user_id.as_ref());

    // Step 6: Client refreshes access token before expiry
    let new_tokens = auth_service.refresh_access(&tokens.refresh).unwrap();

    // Step 7: Old access still works (not revoked in this simple impl)
    let _old_claims = auth_service.verify_access(&tokens.access).unwrap();

    // Step 8: New access token also works
    let new_claims = auth_service.verify_access(&new_tokens.access).unwrap();
    assert_eq!(new_claims.sub, user_id.as_ref());
}

#[test]
fn test_concurrent_auth_attempts() {
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;
    use std::sync::Arc;
    use std::thread;

    let keypair = Ed25519KeyPair::from_bytes(b"test-signing-key-32-bytes-long!!");
    let auth_service = Arc::new(Ed25519AuthService::new(&keypair).unwrap());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let service = Arc::clone(&auth_service);
            thread::spawn(move || {
                let challenge = service.create_challenge();
                let secret_bytes: [u8; 32] = rand::thread_rng().gen();
                let client_key = SigningKey::from_bytes(&secret_bytes);
                let signature = client_key.sign(&challenge.nonce);
                let public_key = client_key.verifying_key().to_bytes();

                service
                    .verify_challenge_response(&challenge, &signature.to_bytes(), &public_key)
                    .unwrap()
            })
        })
        .collect();

    // All threads should succeed
    for handle in handles {
        assert!(handle.join().is_ok());
    }
}
