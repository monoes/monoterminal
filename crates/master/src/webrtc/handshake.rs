// PeerHandshake protocol with Ed25519 challenge-response
// ADR-011 §7.1: Ed25519 Peer Authentication
// Per protocol-phase2-design.md: fields 29-30

use crate::webrtc::error::{Result, WebRtcError};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Protocol version for Phase 2
const PROTOCOL_VERSION: u32 = 2;

/// PeerHandshake message (Protocol field 29)
/// Client → Master: Initiate P2P connection with Ed25519 signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerHandshake {
    /// Protocol version (must be 2 for Phase 2)
    pub protocol_version: u32,

    /// Ed25519 public key (hex-encoded)
    pub peer_id: String,

    /// Timestamp (milliseconds since UNIX epoch)
    pub timestamp_ms: u64,

    /// Ed25519 signature over "MONOTERMINAL-P2P-HANDSHAKE:{version}:{peer_id}:{timestamp}"
    pub signature: Vec<u8>,
}

impl PeerHandshake {
    /// Create a new PeerHandshake message signed with the provided key
    pub fn new(signing_key: &SigningKey) -> Result<Self> {
        let peer_id = hex::encode(signing_key.verifying_key().to_bytes());
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WebRtcError::Internal(format!("System time error: {}", e)))?
            .as_millis() as u64;

        // Construct payload to sign
        let payload = format!(
            "MONOTERMINAL-P2P-HANDSHAKE:{}:{}:{}",
            PROTOCOL_VERSION, peer_id, timestamp_ms
        );

        // Sign with Ed25519
        let signature = signing_key.sign(payload.as_bytes());

        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            peer_id,
            timestamp_ms,
            signature: signature.to_bytes().to_vec(),
        })
    }

    /// Verify the handshake signature and timestamp
    pub fn verify(&self) -> Result<()> {
        // Step 1: Check protocol version
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(WebRtcError::ProtocolVersionMismatch {
                got: self.protocol_version,
                expected: PROTOCOL_VERSION,
            });
        }

        // Step 2: Check timestamp (prevent replay attacks - ADR-011 §7.1)
        // Allow ±30 seconds clock skew
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WebRtcError::Internal(format!("System time error: {}", e)))?
            .as_millis() as u64;

        let delta = (now as i64 - self.timestamp_ms as i64).abs();
        if delta > 30_000 {
            return Err(WebRtcError::ChallengeExpired(delta));
        }

        // Step 3: Verify Ed25519 signature
        let public_key_bytes = hex::decode(&self.peer_id)
            .map_err(|e| WebRtcError::HandshakeVerificationFailed(format!("Invalid peer_id hex: {}", e)))?;

        let verifying_key = VerifyingKey::from_bytes(
            public_key_bytes
                .as_slice()
                .try_into()
                .map_err(|_| WebRtcError::HandshakeVerificationFailed("Invalid key length".to_string()))?,
        )
        .map_err(|e| WebRtcError::HandshakeVerificationFailed(format!("Invalid public key: {}", e)))?;

        let signature = Signature::from_bytes(
            self.signature
                .as_slice()
                .try_into()
                .map_err(|_| WebRtcError::HandshakeVerificationFailed("Invalid signature length".to_string()))?,
        );

        // Reconstruct payload
        let payload = format!(
            "MONOTERMINAL-P2P-HANDSHAKE:{}:{}:{}",
            self.protocol_version, self.peer_id, self.timestamp_ms
        );

        verifying_key
            .verify(payload.as_bytes(), &signature)
            .map_err(|_| WebRtcError::InvalidSignature)?;

        Ok(())
    }
}

/// PeerHandshakeResponse message (Protocol field 30)
/// Master → Client: Challenge + nonce for subsequent WebRTC negotiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerHandshakeResponse {
    /// Random challenge nonce (32 bytes, hex-encoded)
    pub nonce: String,

    /// Server timestamp (milliseconds since UNIX epoch)
    pub timestamp_ms: u64,

    /// Whether handshake was accepted
    pub accepted: bool,

    /// Optional error message if rejected
    pub error_message: Option<String>,
}

impl PeerHandshakeResponse {
    /// Create a successful response with a random nonce
    pub fn accept() -> Result<Self> {
        let nonce = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let bytes: [u8; 32] = rng.gen();
            hex::encode(bytes)
        };

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WebRtcError::Internal(format!("System time error: {}", e)))?
            .as_millis() as u64;

        Ok(Self {
            nonce,
            timestamp_ms,
            accepted: true,
            error_message: None,
        })
    }

    /// Create a rejection response
    pub fn reject(reason: String) -> Result<Self> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WebRtcError::Internal(format!("System time error: {}", e)))?
            .as_millis() as u64;

        Ok(Self {
            nonce: String::new(),
            timestamp_ms,
            accepted: false,
            error_message: Some(reason),
        })
    }
}

/// HandshakeVerifier - Stateful verifier for PeerHandshake protocol
pub struct HandshakeVerifier {
    /// Accepted peer IDs (for tracking who's been verified)
    verified_peers: std::collections::HashSet<String>,
}

impl HandshakeVerifier {
    pub fn new() -> Self {
        Self {
            verified_peers: std::collections::HashSet::new(),
        }
    }

    /// Verify and record a peer handshake
    pub fn verify(&mut self, handshake: &PeerHandshake) -> Result<PeerHandshakeResponse> {
        // Verify signature and timestamp
        handshake.verify()?;

        // Record verified peer
        self.verified_peers.insert(handshake.peer_id.clone());

        // Return acceptance with nonce
        PeerHandshakeResponse::accept()
    }

    /// Check if a peer has been verified
    pub fn is_verified(&self, peer_id: &str) -> bool {
        self.verified_peers.contains(peer_id)
    }

    /// Remove a peer from verified set (e.g., on disconnect)
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.verified_peers.remove(peer_id);
    }
}

impl Default for HandshakeVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_create_and_verify() {
        use rand::rngs::OsRng;

        // Generate a signing key
        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);

        // Create handshake
        let handshake = PeerHandshake::new(&signing_key).unwrap();

        // Verify it
        assert!(handshake.verify().is_ok());
        assert_eq!(handshake.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn test_handshake_wrong_protocol_version() {
        use rand::rngs::OsRng;

        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let mut handshake = PeerHandshake::new(&signing_key).unwrap();

        // Tamper with protocol version
        handshake.protocol_version = 1;

        // Verification should fail
        assert!(matches!(
            handshake.verify(),
            Err(WebRtcError::ProtocolVersionMismatch { .. })
        ));
    }

    #[test]
    fn test_handshake_expired_timestamp() {
        use rand::rngs::OsRng;

        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let mut handshake = PeerHandshake::new(&signing_key).unwrap();

        // Set timestamp to 60 seconds ago (outside 30s window)
        handshake.timestamp_ms -= 60_000;

        // Verification should fail
        assert!(matches!(
            handshake.verify(),
            Err(WebRtcError::ChallengeExpired(_))
        ));
    }

    #[test]
    fn test_handshake_invalid_signature() {
        use rand::rngs::OsRng;

        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let mut handshake = PeerHandshake::new(&signing_key).unwrap();

        // Corrupt signature
        handshake.signature[0] ^= 0xFF;

        // Verification should fail
        assert!(matches!(
            handshake.verify(),
            Err(WebRtcError::InvalidSignature)
        ));
    }

    #[test]
    fn test_handshake_verifier() {
        use rand::rngs::OsRng;

        use rand::RngCore;
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let handshake = PeerHandshake::new(&signing_key).unwrap();
        let peer_id = handshake.peer_id.clone();

        let mut verifier = HandshakeVerifier::new();

        // Verify handshake
        let response = verifier.verify(&handshake).unwrap();
        assert!(response.accepted);
        assert!(!response.nonce.is_empty());

        // Check peer is tracked
        assert!(verifier.is_verified(&peer_id));

        // Remove peer
        verifier.remove_peer(&peer_id);
        assert!(!verifier.is_verified(&peer_id));
    }

    #[test]
    fn test_handshake_response_accept() {
        let response = PeerHandshakeResponse::accept().unwrap();
        assert!(response.accepted);
        assert!(response.error_message.is_none());
        assert_eq!(response.nonce.len(), 64); // 32 bytes hex = 64 chars
    }

    #[test]
    fn test_handshake_response_reject() {
        let response = PeerHandshakeResponse::reject("Test error".to_string()).unwrap();
        assert!(!response.accepted);
        assert_eq!(response.error_message, Some("Test error".to_string()));
    }
}
