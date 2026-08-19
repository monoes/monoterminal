// Ed25519 Key Management
// SRS §3.2.2: Ed25519 SSH Keys + JWT Authentication
// ADR-007: EdDSA Algorithm for Phase 1 Authentication

use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Ed25519 keypair for JWT signing
#[derive(Clone, Debug)]
pub struct Ed25519KeyPair {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Ed25519KeyPair {
    /// Create keypair from raw bytes (32-byte signing key)
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Generate new random keypair
    pub fn generate() -> Self {
        use rand::{rngs::OsRng, RngCore};

        // Generate random 32-byte secret
        let mut secret_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut secret_bytes);

        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Get signing key bytes (private - 32 bytes)
    pub fn signing_bytes(&self) -> &[u8; 32] {
        self.signing_key.as_bytes()
    }

    /// Get verifying key bytes (public - 32 bytes)
    pub fn verifying_bytes(&self) -> &[u8; 32] {
        self.verifying_key.as_bytes()
    }

    /// Convert to PKCS#8 DER format for jsonwebtoken
    /// Returns (private_pkcs8_der, public_spki_der)
    pub fn to_der(&self) -> (Vec<u8>, Vec<u8>) {
        // PKCS#8 private key: manually construct DER
        let private_der = encode_pkcs8_private_key(self.signing_bytes());

        // SubjectPublicKeyInfo: manually construct DER
        let public_der = encode_spki_public_key(self.verifying_bytes());

        (private_der, public_der)
    }

    /// Convert to PEM format for jsonwebtoken (PROVEN WORKING)
    /// Returns (private_pem, public_pem)
    pub fn to_pem(&self) -> (String, String) {
        // Get DER encoding first
        let (private_der, public_der) = self.to_der();

        // Wrap in PEM format (base64 + headers)
        let private_pem = encode_pem(&private_der, "PRIVATE KEY");
        let public_pem = encode_pem(&public_der, "PUBLIC KEY");

        (private_pem, public_pem)
    }
}

/// Load or generate Ed25519 keypair
///
/// Storage pattern (SRS §3.2.2):
/// - Private key: ~/.monoterminal/identity.key (0600 permissions)
/// - Public key: Derived from private (no separate storage)
///
/// Security:
/// - Creates directory with 0700 permissions
/// - Creates key file with 0600 permissions (owner read/write only)
/// - Generates new keypair if file doesn't exist
/// - Loads existing keypair if file exists
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

/// Get path to identity key file
fn get_identity_key_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow!("Failed to determine home directory"))?;

    let monoterminal_dir = home.join(".monoterminal");

    // Create directory if it doesn't exist
    if !monoterminal_dir.exists() {
        fs::create_dir_all(&monoterminal_dir)
            .context("Failed to create .monoterminal directory")?;

        // Set directory permissions to 0700 (owner rwx only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            fs::set_permissions(&monoterminal_dir, perms)
                .context("Failed to set directory permissions")?;
        }
    }

    Ok(monoterminal_dir.join("identity.key"))
}

/// Load keypair from file
fn load_keypair(path: &Path) -> Result<Ed25519KeyPair> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .context("Failed to open identity key file")?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .context("Failed to read identity key file")?;

    if bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid identity key file: expected 32 bytes, got {}",
            bytes.len()
        ));
    }

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);

    Ok(Ed25519KeyPair::from_bytes(&key_bytes))
}

/// Save keypair to file with secure permissions
fn save_keypair(path: &Path, keypair: &Ed25519KeyPair) -> Result<()> {
    // Create file with restricted permissions
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

    // Write signing key bytes
    file.write_all(keypair.signing_bytes())
        .context("Failed to write identity key file")?;

    file.sync_all()
        .context("Failed to sync identity key file")?;

    Ok(())
}

/// Encode Ed25519 private key to PKCS#8 DER format
/// Manually constructs minimal PKCS#8 structure for Ed25519
fn encode_pkcs8_private_key(private_bytes: &[u8; 32]) -> Vec<u8> {
    // PKCS#8 structure for Ed25519:
    // SEQUENCE {
    //   INTEGER 0 (version)
    //   SEQUENCE { OID 1.3.101.112 } (algorithm)
    //   OCTET STRING { OCTET STRING <32-byte-key> } (privateKey)
    // }

    let mut der = Vec::with_capacity(48);

    // SEQUENCE tag + length
    der.push(0x30); // SEQUENCE
    der.push(0x2e); // Length: 46 bytes

    // INTEGER 0 (version)
    der.extend_from_slice(&[0x02, 0x01, 0x00]);

    // SEQUENCE { OID }
    der.push(0x30); // SEQUENCE
    der.push(0x05); // Length: 5 bytes
    der.extend_from_slice(&[0x06, 0x03, 0x2b, 0x65, 0x70]); // OID 1.3.101.112

    // OCTET STRING { OCTET STRING <key> }
    der.push(0x04); // OCTET STRING
    der.push(0x22); // Length: 34 bytes
    der.push(0x04); // Inner OCTET STRING
    der.push(0x20); // Length: 32 bytes
    der.extend_from_slice(private_bytes);

    der
}

/// Encode Ed25519 public key to SubjectPublicKeyInfo DER format
fn encode_spki_public_key(public_bytes: &[u8; 32]) -> Vec<u8> {
    // SubjectPublicKeyInfo structure for Ed25519:
    // SEQUENCE {
    //   SEQUENCE { OID 1.3.101.112 } (algorithm)
    //   BIT STRING <32-byte-key> (subjectPublicKey)
    // }

    let mut der = Vec::with_capacity(44);

    // SEQUENCE tag + length
    der.push(0x30); // SEQUENCE
    der.push(0x2a); // Length: 42 bytes

    // SEQUENCE { OID }
    der.push(0x30); // SEQUENCE
    der.push(0x05); // Length: 5 bytes
    der.extend_from_slice(&[0x06, 0x03, 0x2b, 0x65, 0x70]); // OID 1.3.101.112

    // BIT STRING
    der.push(0x03); // BIT STRING
    der.push(0x21); // Length: 33 bytes
    der.push(0x00); // No unused bits
    der.extend_from_slice(public_bytes);

    der
}

/// Encode DER to PEM format
/// PEM format: -----BEGIN {label}-----\nbase64(der)\n-----END {label}-----
fn encode_pem(der: &[u8], label: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let base64_data = STANDARD.encode(der);

    // Split base64 into 64-character lines (PEM standard)
    let mut pem = format!("-----BEGIN {}-----\n", label);
    for chunk in base64_data.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).unwrap());
        pem.push('\n');
    }
    pem.push_str(&format!("-----END {}-----\n", label));

    pem
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_keypair_generation() {
        let keypair = Ed25519KeyPair::generate();
        assert_eq!(keypair.signing_bytes().len(), 32);
        assert_eq!(keypair.verifying_bytes().len(), 32);
    }

    #[test]
    fn test_keypair_from_bytes() {
        let bytes = [0x42u8; 32];
        let keypair = Ed25519KeyPair::from_bytes(&bytes);
        assert_eq!(keypair.signing_bytes(), &bytes);
    }

    #[test]
    fn test_keypair_to_der() {
        let keypair = Ed25519KeyPair::generate();
        let (private_der, public_der) = keypair.to_der();
        // PKCS#8 private key: 48 bytes
        assert_eq!(private_der.len(), 48, "PKCS#8 private key should be 48 bytes");
        // SubjectPublicKeyInfo: 44 bytes
        assert_eq!(public_der.len(), 44, "SPKI public key should be 44 bytes");

        // Verify DER structure (SEQUENCE tag)
        assert_eq!(private_der[0], 0x30, "Private DER should start with SEQUENCE");
        assert_eq!(public_der[0], 0x30, "Public DER should start with SEQUENCE");
    }

    #[test]
    fn test_save_and_load_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("test.key");

        let original = Ed25519KeyPair::generate();
        save_keypair(&key_path, &original).unwrap();

        let loaded = load_keypair(&key_path).unwrap();
        assert_eq!(loaded.signing_bytes(), original.signing_bytes());
        assert_eq!(loaded.verifying_bytes(), original.verifying_bytes());
    }

    #[test]
    fn test_load_invalid_key_size() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("invalid.key");

        // Write invalid size (not 32 bytes)
        fs::write(&key_path, &[0u8; 16]).unwrap();

        let result = load_keypair(&key_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected 32 bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn test_key_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("perms.key");

        let keypair = Ed25519KeyPair::generate();
        save_keypair(&key_path, &keypair).unwrap();

        let metadata = fs::metadata(&key_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "Key file should have 0600 permissions");
    }
}
