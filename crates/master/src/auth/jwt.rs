// JWT management with Ed25519 signing
// SRS §3.2.2: Access (15min) + Refresh (30d) with rotation
// ADR-007: EdDSA Algorithm for Phase 1 Authentication

use super::{Ed25519KeyPair, UserId};
use anyhow::{anyhow, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct TokenPair {
    pub access: String,
    pub refresh: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub exp: i64,
    pub iat: i64,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

pub struct JwtService {
    enc: EncodingKey,
    dec: DecodingKey,
    issuer: String,
    used: Arc<Mutex<HashSet<String>>>,
}

impl JwtService {
    /// Create JWT service with Ed25519 keypair
    /// Per ADR-007: EdDSA algorithm for Phase 1 authentication
    pub fn new(keypair: &Ed25519KeyPair) -> Result<Self> {
        // Use PEM format (proven working in jsonwebtoken tests)
        let (private_pem, public_pem) = keypair.to_pem();

        Ok(Self {
            enc: EncodingKey::from_ed_pem(private_pem.as_bytes())
                .map_err(|e| anyhow!("Failed to create encoding key: {}", e))?,
            dec: DecodingKey::from_ed_pem(public_pem.as_bytes())
                .map_err(|e| anyhow!("Failed to create decoding key: {}", e))?,
            issuer: "monoterminal-master".to_string(),
            used: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn issue_tokens(&self, user_id: &UserId) -> Result<TokenPair> {
        let now = timestamp();
        let access = Claims {
            sub: user_id.0.clone(),
            iss: self.issuer.clone(),
            exp: now + 900,
            iat: now,
            scope: "session:attach session:create input:write".into(),
            jti: Some(gen_jti()), // Each access token unique for revocation
        };
        let refresh = Claims {
            sub: user_id.0.clone(),
            iss: self.issuer.clone(),
            exp: now + 2592000,
            iat: now,
            scope: "token:refresh".into(),
            jti: Some(gen_jti()),
        };
        Ok(TokenPair {
            access: self.build(&access)?,
            refresh: self.build(&refresh)?,
        })
    }

    pub fn verify_access_token(&self, tok: &str) -> Result<Claims> {
        let c = self.parse(tok)?;
        // Access tokens now have JTI for revocation support
        if !c.scope.contains("session:") {
            return Err(anyhow!("Invalid: missing session scope"));
        }
        Ok(c)
    }

    pub fn refresh_access_token(&self, tok: &str) -> Result<TokenPair> {
        let c = self.parse(tok)?;
        if c.scope != "token:refresh" {
            return Err(anyhow!("Not a refresh token"));
        }
        let jti = c.jti.as_ref().ok_or(anyhow!("Missing JTI"))?;
        {
            let mut u = self.used.lock().unwrap();
            if u.contains(jti) {
                return Err(anyhow!("Reuse detected: {}", c.sub));
            }
            u.insert(jti.clone());
        }
        self.issue_tokens(&UserId(c.sub))
    }

    fn build(&self, c: &Claims) -> Result<String> {
        // ADR-007: EdDSA (Ed25519) asymmetric signing
        encode(&Header::new(Algorithm::EdDSA), c, &self.enc)
            .map_err(|e| anyhow!("Encode failed: {}", e))
    }

    fn parse(&self, tok: &str) -> Result<Claims> {
        // ADR-007: EdDSA (Ed25519) asymmetric verification
        let mut v = Validation::new(Algorithm::EdDSA);
        v.set_issuer(&[&self.issuer]);
        decode::<Claims>(tok, &self.dec, &v)
            .map(|d| d.claims)
            .map_err(|e| anyhow!("Decode failed: {}", e))
    }

    #[cfg(test)]
    pub fn clear(&self) {
        self.used.lock().unwrap().clear();
    }
}

fn timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn gen_jti() -> String {
    use rand::Rng;
    hex::encode(rand::thread_rng().gen::<[u8; 16]>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc() -> JwtService {
        let keypair = Ed25519KeyPair::from_bytes(&[0x42; 32]);
        JwtService::new(&keypair).unwrap()
    }

    #[test]
    fn test_new() {
        let keypair = Ed25519KeyPair::from_bytes(&[0x42; 32]);
        assert!(JwtService::new(&keypair).is_ok());
    }

    #[test]
    fn test_issue() {
        let s = svc();
        let p = s.issue_tokens(&UserId("a@b.com".into())).unwrap();
        assert!(!p.access.is_empty());
        assert!(!p.refresh.is_empty());
    }

    #[test]
    fn test_verify() {
        let s = svc();
        let p = s.issue_tokens(&UserId("user".into())).unwrap();
        let c = s.verify_access_token(&p.access).unwrap();
        assert_eq!(c.sub, "user");
    }

    #[test]
    fn test_refresh() {
        let s = svc();
        let p1 = s.issue_tokens(&UserId("u".into())).unwrap();
        let p2 = s.refresh_access_token(&p1.refresh).unwrap();
        assert_ne!(p1.access, p2.access);
    }

    #[test]
    fn test_reuse() {
        let s = svc();
        let p = s.issue_tokens(&UserId("u".into())).unwrap();
        assert!(s.refresh_access_token(&p.refresh).is_ok());
        assert!(s.refresh_access_token(&p.refresh).is_err());
    }
}
