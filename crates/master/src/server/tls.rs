// TLS 1.3 configuration
// Implements SRS §3.2.1 (Transport Security)

use rustls::server::AllowAnyAuthenticatedClient;
use rustls::version::TLS13;
use rustls::{Certificate, PrivateKey, ServerConfig};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;

use super::error::{Result, ServerError};

/// TLS configuration for WebSocket server
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Path to TLS certificate (PEM format)
    pub cert_path: PathBuf,

    /// Path to TLS private key (PEM format)
    pub key_path: PathBuf,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: PathBuf::from("certs/server.crt"),
            key_path: PathBuf::from("certs/server.key"),
        }
    }
}

impl TlsConfig {
    /// Create TLS configuration with custom paths
    pub fn new(cert_path: impl Into<PathBuf>, key_path: impl Into<PathBuf>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: key_path.into(),
        }
    }

    /// Build TLS acceptor with TLS 1.3 only
    pub fn build_acceptor(&self) -> Result<TlsAcceptor> {
        // Load certificates
        let certs = load_certs(&self.cert_path)?;
        let key = load_private_key(&self.key_path)?;

        // Build rustls ServerConfig - TLS 1.3 only
        let config = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&TLS13])
            .map_err(|e| ServerError::Internal(format!("Failed to set TLS 1.3 only: {}", e)))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| ServerError::Internal(format!("Failed to configure TLS: {}", e)))?;

        Ok(TlsAcceptor::from(Arc::new(config)))
    }

    /// Build TLS acceptor for dev/test mode with in-memory self-signed certificate
    /// WARNING: For testing only - uses hardcoded test certificate
    pub fn build_dev_acceptor() -> Result<TlsAcceptor> {
        // Generate in-memory self-signed certificate for tests
        let cert_pem = include_bytes!("../../../../certs/server.crt");
        let key_pem = include_bytes!("../../../../certs/server.key");

        let certs: Vec<Certificate> = rustls_pemfile::certs(&mut &cert_pem[..])
            .map_err(|e| ServerError::Internal(format!("Failed to parse test certificate: {}", e)))?
            .into_iter()
            .map(Certificate)
            .collect();

        let keys = rustls_pemfile::pkcs8_private_keys(&mut &key_pem[..])
            .map_err(|e| ServerError::Internal(format!("Failed to parse test key: {}", e)))?;

        if keys.is_empty() {
            return Err(ServerError::Internal(
                "No private key found in test key".to_string(),
            ));
        }

        let key = PrivateKey(keys[0].clone());

        // Build rustls ServerConfig - TLS 1.3 only
        let config = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&TLS13])
            .map_err(|e| ServerError::Internal(format!("Failed to set TLS 1.3 only: {}", e)))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| ServerError::Internal(format!("Failed to configure TLS: {}", e)))?;

        Ok(TlsAcceptor::from(Arc::new(config)))
    }
}

/// Load certificates from PEM file
fn load_certs(path: &Path) -> Result<Vec<Certificate>> {
    let file = File::open(path).map_err(|e| {
        ServerError::Internal(format!(
            "Failed to open cert file {}: {}",
            path.display(),
            e
        ))
    })?;
    let mut reader = BufReader::new(file);

    let certs: Vec<Certificate> = rustls_pemfile::certs(&mut reader)
        .map_err(|e| ServerError::Internal(format!("Failed to parse certificates: {}", e)))?
        .into_iter()
        .map(Certificate)
        .collect();

    if certs.is_empty() {
        Err(ServerError::Internal(
            "No certificates found in file".to_string(),
        ))
    } else {
        Ok(certs)
    }
}

/// Load private key from PEM file
fn load_private_key(path: &Path) -> Result<PrivateKey> {
    let file = File::open(path).map_err(|e| {
        ServerError::Internal(format!("Failed to open key file {}: {}", path.display(), e))
    })?;
    let mut reader = BufReader::new(file);

    // Try PKCS8 first, then RSA
    let keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
        .map_err(|e| ServerError::Internal(format!("Failed to parse private key: {}", e)))?;

    if !keys.is_empty() {
        return Ok(PrivateKey(keys[0].clone()));
    }

    // Try RSA format
    let file = File::open(path).map_err(|e| {
        ServerError::Internal(format!("Failed to open key file {}: {}", path.display(), e))
    })?;
    let mut reader = BufReader::new(file);

    let keys = rustls_pemfile::rsa_private_keys(&mut reader)
        .map_err(|e| ServerError::Internal(format!("Failed to parse RSA private key: {}", e)))?;

    if !keys.is_empty() {
        return Ok(PrivateKey(keys[0].clone()));
    }

    Err(ServerError::Internal(
        "No private key found in file".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert_eq!(config.cert_path, PathBuf::from("certs/server.crt"));
        assert_eq!(config.key_path, PathBuf::from("certs/server.key"));
    }

    #[test]
    fn test_tls_config_custom() {
        let config = TlsConfig::new("/path/to/cert.pem", "/path/to/key.pem");
        assert_eq!(config.cert_path, PathBuf::from("/path/to/cert.pem"));
        assert_eq!(config.key_path, PathBuf::from("/path/to/key.pem"));
    }
}
