use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, StatusCode};
use axum::Json;
use jsonwebtoken as jwt;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::SharedState;

const SESSION_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn issue_session(signing_key: &[u8], user_id: i64, email: &str) -> anyhow::Result<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp: (now + SESSION_TTL_SECONDS) as usize,
    };
    let encoding_key = jwt::EncodingKey::from_secret(signing_key);
    let encoded = jwt::encode(&jwt::Header::default(), &claims, &encoding_key)?;
    Ok(encoded)
}

pub fn verify_session(signing_key: &[u8], encoded: &str) -> anyhow::Result<Claims> {
    let decoding_key = jwt::DecodingKey::from_secret(signing_key);
    let validation = jwt::Validation::default();
    let data = jwt::decode::<Claims>(encoded, &decoding_key, &validation)?;
    Ok(data.claims)
}

/// Loads the JWT signing key from the environment, or generates a random
/// one for the lifetime of this process. The generated key is not
/// persisted, so sessions issued before a restart stop validating — fine
/// for dev, callers should set the env var in production.
pub fn load_or_generate_signing_key() -> Vec<u8> {
    const ENV_VAR: &str = "JWT_SECRET";
    if let Ok(value) = std::env::var(ENV_VAR) {
        if !value.is_empty() {
            return value.into_bytes();
        }
    }

    tracing::warn!(env_var = ENV_VAR, "signing key env var not set, generating an ephemeral one for this process — sessions will not survive a restart");
    let mut bytes = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
}

pub struct AuthUser {
    pub user_id: i64,
    pub email: String,
}

fn unauthorized() -> (StatusCode, Json<Value>) {
    (StatusCode::UNAUTHORIZED, Json(json!({"error": "missing or invalid authorization credential"})))
}

#[axum::async_trait]
impl FromRequestParts<Arc<SharedState>> for AuthUser {
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, shared: &Arc<SharedState>) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(unauthorized)?;

        let credential = header_value
            .strip_prefix("Bearer ")
            .ok_or_else(unauthorized)?;

        let claims = verify_session(&shared.jwt_signing_key, credential).map_err(|_| unauthorized())?;
        let user_id: i64 = claims.sub.parse().map_err(|_| unauthorized())?;

        Ok(AuthUser {
            user_id,
            email: claims.email,
        })
    }
}
