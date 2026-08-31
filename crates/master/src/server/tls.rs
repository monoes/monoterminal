// TLS 1.3 configuration
// Implements SRS §3.2.1 (Transport Security)

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

    /// Get a `TlsConfig` for `dir`, generating a self-signed certificate
    /// there first if one doesn't already exist.
    ///
    /// Outside of `--dev-mode` (which uses a hardcoded, compiled-in test
    /// cert), production runs had no certificate provisioning at all: the
    /// default `cert_path`/`key_path` are the relative path `certs/`, which
    /// only resolves when the process's cwd happens to be the source
    /// checkout. Any real deployment — an installed service, or even just
    /// `monoterminal-master` run from an arbitrary directory — would fail
    /// at startup with "Failed to open cert file certs/server.crt: No such
    /// file or directory" and never get a chance to serve anything. This
    /// makes every such case self-sufficient: point it at a real directory
    /// (see `platform::paths::system_cert_dir`/`user_cert_dir`) and it
    /// creates what it needs on first run, matching how `gen-tls-cert.ps1`
    /// bootstraps the dev certificate.
    #[allow(clippy::result_large_err)]
    pub fn ensure_self_signed(dir: &Path) -> Result<Self> {
        let cert_path = dir.join("server.crt");
        let key_path = dir.join("server.key");

        if !cert_path.exists() || !key_path.exists() {
            std::fs::create_dir_all(dir).map_err(|e| {
                ServerError::Internal(format!(
                    "Failed to create TLS cert directory {}: {}",
                    dir.display(),
                    e
                ))
            })?;

            let cert = rcgen::generate_simple_self_signed(vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
            ])
            .map_err(|e| {
                ServerError::Internal(format!("Failed to generate self-signed certificate: {}", e))
            })?;

            let cert_pem = cert.serialize_pem().map_err(|e| {
                ServerError::Internal(format!("Failed to serialize certificate: {}", e))
            })?;
            let key_pem = cert.serialize_private_key_pem();

            std::fs::write(&cert_path, cert_pem).map_err(|e| {
                ServerError::Internal(format!(
                    "Failed to write certificate {}: {}",
                    cert_path.display(),
                    e
                ))
            })?;
            std::fs::write(&key_path, &key_pem).map_err(|e| {
                ServerError::Internal(format!(
                    "Failed to write private key {}: {}",
                    key_path.display(),
                    e
                ))
            })?;

            // Private key: owner read/write only (0600 — Unix only, matches
            // the identity key's permission convention; not meaningful on
            // Windows, whose ACL model set_permissions doesn't map to).
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| {
                        ServerError::Internal(format!(
                            "Failed to set private key permissions: {}",
                            e
                        ))
                    })?;
            }

            tracing::info!(
                "Generated self-signed TLS certificate: {}",
                cert_path.display()
            );
        }

        Ok(Self::new(cert_path, key_path))
    }

    /// Build TLS acceptor with TLS 1.3 only
    #[allow(clippy::result_large_err)]
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

    /// Build TLS acceptor for dev/test mode with in-memory self-signed certificate.
    /// WARNING: For testing only - uses hardcoded test certificate.
    ///
    /// Deliberately available in every build profile: `--dev-mode` is a
    /// runtime flag the caller opts into explicitly (and which itself warns
    /// loudly not to use in production), not a build-time concern — gating
    /// this by `debug_assertions`/`test` previously made it work in exactly
    /// one profile and silently error in the other depending on which way
    /// the cfg was set, which is what caused this to regress twice.
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
