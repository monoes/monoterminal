// Discovery error types
// ADR-011 §4: Discovery Services

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DiscoveryError>;

#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("mDNS service registration failed: {0}")]
    MdnsRegistrationFailed(String),

    #[error("mDNS service discovery failed: {0}")]
    MdnsDiscoveryFailed(String),

    #[error("mDNS service not found: {0}")]
    ServiceNotFound(String),

    #[error("Directory service registration failed: {0}")]
    DirectoryRegistrationFailed(String),

    #[error("Directory service lookup failed: {0}")]
    DirectoryLookupFailed(String),

    #[error("Directory service unavailable: {0}")]
    DirectoryUnavailable(String),

    #[error("Invalid service endpoint: {0}")]
    InvalidEndpoint(String),

    #[error("Ed25519 signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("Discovery timeout after {0:?}")]
    DiscoveryTimeout(std::time::Duration),

    #[error("No discovery methods available")]
    NoDiscoveryMethods,

    #[error("All discovery methods failed")]
    AllMethodsFailed,

    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("JSON serialization/deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
