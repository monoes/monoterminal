// Authentication and authorization module
// Implements SRS §3.2: Ed25519 + JWT + RBAC + Rate Limiting

pub mod challenge;
pub mod jwt;
pub mod keys;
pub mod rate_limit;
pub mod rbac;

use anyhow::Result;
use async_trait::async_trait;

// Re-exports for convenience
pub use challenge::{Challenge, Ed25519ChallengeHandler};
pub use jwt::{Claims, JwtService, TokenPair};
pub use keys::{load_or_generate_keypair, Ed25519KeyPair};
pub use rate_limit::RateLimiter;
pub use rbac::{check_permission, Action, Permission};

// ===== Core Types =====

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

impl From<String> for UserId {
    fn from(s: String) -> Self {
        UserId(s)
    }
}

impl From<&str> for UserId {
    fn from(s: &str) -> Self {
        UserId(s.to_string())
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Ed25519 public key (32 bytes)
pub type PublicKey = [u8; 32];

/// Ed25519 signature (64 bytes)
pub type Signature = [u8; 64];

// ===== AuthService Trait =====

/// Authentication service interface (per architecture docs)
#[async_trait]
pub trait AuthService: Send + Sync {
    /// Generate challenge for client
    fn create_challenge(&self) -> Challenge;

    /// Verify signed challenge
    fn verify_challenge_response(
        &self,
        challenge: &Challenge,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<UserId>;

    /// Issue JWT tokens
    fn issue_tokens(&self, user_id: &UserId) -> Result<TokenPair>;

    /// Verify JWT access token
    fn verify_access(&self, access: &str) -> Result<Claims>;

    /// Refresh access token using refresh token
    fn refresh_access(&self, refresh: &str) -> Result<TokenPair>;
}

// ===== Ed25519AuthService Implementation =====

/// Default authentication service using Ed25519 + JWT
pub struct Ed25519AuthService {
    challenge_handler: Ed25519ChallengeHandler,
    jwt_service: JwtService,
}

impl Ed25519AuthService {
    /// Create new auth service with Ed25519 keypair
    /// Per ADR-007: EdDSA algorithm for Phase 1 authentication
    pub fn new(keypair: &Ed25519KeyPair) -> Result<Self> {
        Ok(Self {
            challenge_handler: Ed25519ChallengeHandler::new(),
            jwt_service: JwtService::new(keypair)?,
        })
    }

    /// Create auth service with auto-generated/loaded keypair
    /// Loads from ~/.monoterminal/identity.key or generates new
    pub fn new_with_auto_keypair() -> Result<Self> {
        let keypair = load_or_generate_keypair()?;
        Self::new(&keypair)
    }
}

#[async_trait]
impl AuthService for Ed25519AuthService {
    fn create_challenge(&self) -> Challenge {
        self.challenge_handler.create_challenge()
    }

    fn verify_challenge_response(
        &self,
        challenge: &Challenge,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<UserId> {
        self.challenge_handler
            .verify_challenge_response(challenge, signature, public_key)
    }

    fn issue_tokens(&self, user_id: &UserId) -> Result<TokenPair> {
        self.jwt_service.issue_tokens(user_id)
    }

    fn verify_access(&self, access: &str) -> Result<Claims> {
        self.jwt_service.verify_access_token(access)
    }

    fn refresh_access(&self, refresh: &str) -> Result<TokenPair> {
        self.jwt_service.refresh_access_token(refresh)
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_id_conversions() {
        let user_id = UserId::from("alice@example.com");
        assert_eq!(user_id.as_ref(), "alice@example.com");

        let user_id2: UserId = "bob@example.com".into();
        assert_eq!(user_id2.0, "bob@example.com");
    }

    #[test]
    fn test_auth_service_creation() {
        let keypair = Ed25519KeyPair::from_bytes(&[0x42; 32]);
        let auth_service = Ed25519AuthService::new(&keypair);
        assert!(auth_service.is_ok());
    }

    #[test]
    fn test_auth_service_auto_keypair() {
        // This test creates real keys in ~/.monoterminal/
        // Only run in integration test environment
        // For unit tests, use test_auth_service_creation()
        let result = Ed25519AuthService::new_with_auto_keypair();
        assert!(result.is_ok());
    }
}
