# Certificate Management Guide
**Project:** MONOTERMINAL  
**Version:** 1.0  
**Scope:** Phase 1 (Development self-signed) + Phase 2+ (Production Let's Encrypt)

## Overview

MONOTERMINAL requires TLS 1.3 certificates for WebSocket server encryption. This guide covers certificate generation, deployment, and rotation for both development and production environments.

## Development: Self-Signed Certificates

### Automatic Generation

On first startup, MONOTERMINAL automatically generates a self-signed certificate if none exists.

**Storage Location:**
```
~/.monoterminal/
  ├── cert.pem          (Certificate - 0644, world readable)
  └── key.pem           (Private key - 0600, owner only)
```

**Certificate Properties:**
- **Algorithm:** RSA 2048-bit or Ed25519 (future)
- **Validity:** 365 days from generation
- **Subject:** CN=monoterminal-master
- **Usage:** TLS Server Authentication

### Manual Generation

Generate manually using OpenSSL:

```bash
# Generate private key
openssl genpkey -algorithm RSA -out key.pem -pkeyopt rsa_keygen_bits:2048

# Generate self-signed certificate (1 year validity)
openssl req -new -x509 -key key.pem -out cert.pem -days 365 \
  -subj "/CN=monoterminal-master"

# Set correct permissions
chmod 600 key.pem
chmod 644 cert.pem
```

Or using Rust's `rcgen` crate (preferred for cross-platform):

```rust
use rcgen::{Certificate, CertificateParams, DistinguishedName};

let mut params = CertificateParams::new(vec!["localhost".to_string()]);
params.distinguished_name = DistinguishedName::new();
params.distinguished_name.push(rcgen::DnType::CommonName, "monoterminal-master");

let cert = Certificate::from_params(params)?;
std::fs::write("cert.pem", cert.serialize_pem()?)?;
std::fs::write("key.pem", cert.serialize_private_key_pem())?;
```

### Client Trust (TOFU Model)

**Trust On First Use (TOFU):**

1. **First Connection:**
   - Client connects to master
   - Master presents self-signed certificate
   - Client displays certificate fingerprint (SHA-256)
   - User confirms "Accept and Remember"
   - Client stores cert fingerprint locally

2. **Subsequent Connections:**
   - Client checks if cert fingerprint matches stored value
   - If match → connection proceeds
   - If mismatch → **WARNING: Certificate changed!**
   - User must explicitly approve new certificate

**Certificate Fingerprint Display:**
```
┌─────────────────────────────────────────────┐
│  New Certificate Detected                   │
├─────────────────────────────────────────────┤
│  Server: monoterminal-master                │
│  Fingerprint (SHA-256):                     │
│  A3:4F:2B:... (truncated for display)       │
│                                             │
│  ⚠ Only accept if you trust this server    │
│                                             │
│  [ Accept Once ]  [ Accept & Remember ]    │
│  [ Reject ]                                 │
└─────────────────────────────────────────────┘
```

**Security Note:** TOFU is vulnerable to MITM on first connection. For production, use validated certificates (Let's Encrypt).

### Certificate Rotation (Development)

Self-signed certificates expire after 1 year. Rotation options:

**Option 1: Automatic Regeneration (Recommended)**
```rust
// Check certificate expiry on startup
if cert_expires_within(30_days) {
    warn!("Certificate expires in 30 days. Auto-regenerating...");
    regenerate_self_signed_cert()?;
}
```

**Option 2: Manual Rotation**
```bash
# Delete old certificates
rm ~/.monoterminal/cert.pem ~/.monoterminal/key.pem

# Restart master (will auto-generate new cert)
monoterminal-master restart
```

**Client Impact:**
- Clients will see "Certificate changed" warning
- Users must re-accept the new certificate
- Old fingerprints are invalidated

## Production: Let's Encrypt (Phase 2+)

### Prerequisites

1. **Public Domain:** Master must be accessible via a public domain (e.g., `terminal.example.com`)
2. **Port 80 Open:** ACME HTTP-01 challenge requires port 80 OR DNS-01 for DNS validation
3. **Static IP:** Or dynamic DNS (DuckDNS, No-IP) if master runs on residential connection

### ACME Client Integration

**Rust Crate:** `acme-client = "0.7"` or `instant-acme = "0.3"`

**Automatic Certificate Acquisition:**

```rust
use instant_acme::{Account, Identifier, NewOrder, OrderStatus};

async fn obtain_letsencrypt_cert(domain: &str) -> Result<(String, String)> {
    // 1. Create ACME account
    let account = Account::create_with_contact(
        &["mailto:admin@example.com"],
        "https://acme-v02.api.letsencrypt.org/directory"
    ).await?;

    // 2. Create order for domain
    let mut order = account.new_order(&[Identifier::Dns(domain.to_string())]).await?;

    // 3. Get authorization challenges
    let authorizations = order.authorizations().await?;
    for auth in authorizations {
        let challenge = auth.http_challenge(); // HTTP-01 challenge
        
        // 4. Serve challenge token at http://domain/.well-known/acme-challenge/<token>
        serve_acme_challenge(&challenge.token, &challenge.key_authorization).await?;
        
        // 5. Notify ACME server to validate
        challenge.validate().await?;
    }

    // 6. Wait for validation
    while order.status() == OrderStatus::Pending {
        tokio::time::sleep(Duration::from_secs(5)).await;
        order.refresh().await?;
    }

    // 7. Generate CSR and finalize order
    let (cert_pem, key_pem) = order.finalize_and_fetch_cert().await?;
    
    Ok((cert_pem, key_pem))
}
```

### Certificate Storage

**Production Paths:**
```
/etc/monoterminal/
  ├── letsencrypt/
  │   ├── live/
  │   │   └── terminal.example.com/
  │   │       ├── cert.pem       (Certificate chain)
  │   │       ├── privkey.pem    (Private key - 0600)
  │   │       └── fullchain.pem  (Cert + intermediate CA)
  │   └── renewal/
  │       └── terminal.example.com.conf
  └── renewal-hooks/
      ├── pre/                   (Run before renewal)
      └── post/                  (Run after renewal - restart master)
```

**Permissions:**
- Private key: `0600` (root-only or dedicated `monoterminal` user)
- Certificates: `0644` (world-readable)

### Automatic Renewal

Let's Encrypt certificates expire after **90 days**. Renewal should start at **60 days** (30-day buffer).

**Renewal Scheduler (systemd timer):**

```ini
# /etc/systemd/system/monoterminal-cert-renewal.service
[Unit]
Description=MONOTERMINAL Certificate Renewal
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/monoterminal-master --renew-cert
User=monoterminal
```

```ini
# /etc/systemd/system/monoterminal-cert-renewal.timer
[Unit]
Description=MONOTERMINAL Certificate Renewal Timer

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target
```

**Enable timer:**
```bash
sudo systemctl enable monoterminal-cert-renewal.timer
sudo systemctl start monoterminal-cert-renewal.timer
```

**Renewal Logic:**

```rust
async fn check_and_renew_cert() -> Result<()> {
    let cert = load_certificate("cert.pem")?;
    let days_until_expiry = cert.expires_in_days()?;
    
    if days_until_expiry <= 30 {
        info!("Certificate expires in {} days. Renewing...", days_until_expiry);
        let (new_cert, new_key) = obtain_letsencrypt_cert(&config.domain).await?;
        
        // Atomic replacement (write to temp, then rename)
        std::fs::write("/tmp/cert.pem.new", new_cert)?;
        std::fs::write("/tmp/key.pem.new", new_key)?;
        std::fs::rename("/tmp/cert.pem.new", "/etc/monoterminal/letsencrypt/live/.../cert.pem")?;
        std::fs::rename("/tmp/key.pem.new", "/etc/monoterminal/letsencrypt/live/.../privkey.pem")?;
        
        // Reload TLS config without disconnecting clients (graceful reload)
        reload_tls_config()?;
        
        info!("Certificate renewed successfully. New expiry: {} days", 90);
    }
    
    Ok(())
}
```

### Graceful Certificate Reload

**Zero-Downtime Reload:**

```rust
use tokio::sync::watch;

struct TlsConfigReloader {
    config: watch::Sender<Arc<ServerConfig>>,
}

impl TlsConfigReloader {
    pub fn reload(&self) -> Result<()> {
        let new_config = build_tls_config("cert.pem", "key.pem")?;
        self.config.send(Arc::new(new_config))?;
        Ok(())
    }
}

// In WebSocket server accept loop
let tls_config_rx = tls_reloader.subscribe();
loop {
    let stream = listener.accept().await?;
    let current_config = tls_config_rx.borrow().clone();
    
    // Use current_config for this connection
    let tls_stream = tokio_rustls::TlsAcceptor::from(current_config)
        .accept(stream).await?;
}
```

**Signal Handling (Unix):**
```bash
# Send SIGHUP to reload certificates
kill -HUP $(pidof monoterminal-master)
```

```rust
use tokio::signal::unix::{signal, SignalKind};

async fn handle_reload_signal(reloader: Arc<TlsConfigReloader>) {
    let mut sighup = signal(SignalKind::hangup()).unwrap();
    loop {
        sighup.recv().await;
        info!("SIGHUP received. Reloading TLS certificates...");
        if let Err(e) = reloader.reload() {
            error!("Failed to reload certificates: {}", e);
        }
    }
}
```

### Revocation and Emergency Response

**Scenario:** Private key compromised or leaked.

**Immediate Actions:**

1. **Revoke Certificate:**
   ```rust
   use instant_acme::Account;
   
   async fn revoke_certificate(cert_pem: &str) -> Result<()> {
       let account = Account::from_credentials(/* saved account key */)?;
       account.revoke_cert(cert_pem).await?;
       Ok(())
   }
   ```

2. **Generate New Key Pair:**
   ```bash
   # Delete compromised key
   rm /etc/monoterminal/letsencrypt/live/.../privkey.pem
   
   # Obtain new certificate (will generate new key)
   monoterminal-master --renew-cert --force
   ```

3. **Notify Users:**
   - Send in-app notification to all connected clients
   - Force re-authentication on next connection
   - Rotate JWT signing keys (separate from TLS certs)

**OCSP Stapling (Future Enhancement):**
- Serve OCSP responses to prove certificate validity
- Reduces client-side revocation check latency

## Certificate Monitoring

**Metrics to Track:**

1. **Days Until Expiry:**
   ```rust
   gauge!("monoterminal.tls.cert_expiry_days", cert.expires_in_days());
   ```

2. **Renewal Success/Failure:**
   ```rust
   counter!("monoterminal.tls.renewal_attempts");
   counter!("monoterminal.tls.renewal_failures");
   ```

3. **Client Cert Validation Failures:**
   ```rust
   counter!("monoterminal.tls.handshake_failures", 
       "reason" => "cert_invalid");
   ```

**Alerting Thresholds:**

| Metric | Warning | Critical |
|--------|---------|----------|
| Cert Expiry | <30 days | <7 days |
| Renewal Failures | 1 | 3 consecutive |
| Handshake Failures | >10/min | >100/min |

**Monitoring Integration:**
```rust
// Expose metrics via /metrics endpoint (Prometheus format)
use prometheus::{Encoder, TextEncoder, register_gauge};

let cert_expiry_gauge = register_gauge!(
    "monoterminal_tls_cert_expiry_days",
    "Days until TLS certificate expiration"
).unwrap();

// Update daily
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(86400));
    loop {
        interval.tick().await;
        let days = get_cert_expiry_days().unwrap_or(-1.0);
        cert_expiry_gauge.set(days);
    }
});
```

## Troubleshooting

### Self-Signed Certificate Issues

**Problem:** Client refuses to connect ("Certificate validation failed")

**Solution:**
1. Check certificate file exists: `ls -l ~/.monoterminal/cert.pem`
2. Verify certificate validity:
   ```bash
   openssl x509 -in ~/.monoterminal/cert.pem -text -noout
   ```
3. Check expiration date:
   ```bash
   openssl x509 -in ~/.monoterminal/cert.pem -enddate -noout
   ```
4. Regenerate if expired:
   ```bash
   rm ~/.monoterminal/{cert,key}.pem
   monoterminal-master restart
   ```

**Problem:** "Certificate changed" warning on every connection

**Cause:** Certificate regenerating on each startup (check clock skew or file permissions)

**Solution:**
1. Verify system clock is correct: `date`
2. Check file permissions: `ls -l ~/.monoterminal/key.pem` (should be 0600)
3. Check logs for errors during cert generation

### Let's Encrypt Issues

**Problem:** ACME challenge fails (domain validation timeout)

**Diagnostics:**
1. Verify domain resolves to master's public IP:
   ```bash
   dig terminal.example.com +short
   ```
2. Check port 80 is accessible:
   ```bash
   curl -I http://terminal.example.com/.well-known/acme-challenge/test
   ```
3. Review firewall rules:
   ```bash
   sudo ufw status
   sudo iptables -L -n
   ```

**Problem:** Rate limit exceeded (Let's Encrypt enforces limits)

**Limits:**
- 50 certificates per registered domain per week
- 5 duplicate certificates per week

**Solution:**
- Use staging environment for testing:
  ```rust
  let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
  ```
- Wait 7 days for rate limit reset
- See https://letsencrypt.org/docs/rate-limits/

**Problem:** Renewal fails silently

**Diagnostics:**
1. Check renewal timer status:
   ```bash
   sudo systemctl status monoterminal-cert-renewal.timer
   ```
2. Manually trigger renewal:
   ```bash
   sudo systemctl start monoterminal-cert-renewal.service
   ```
3. Review logs:
   ```bash
   sudo journalctl -u monoterminal-cert-renewal.service
   ```

## Security Best Practices

1. **Never commit private keys to git**
   - Add `*.pem`, `*.key` to `.gitignore`
   - Use environment variables or file references in config

2. **Restrict private key permissions**
   ```bash
   chmod 600 key.pem
   chown monoterminal:monoterminal key.pem
   ```

3. **Use separate keys for TLS and JWT signing**
   - TLS key: RSA 2048+ or ECDSA P-256
   - JWT signing key: Ed25519 (different key pair)

4. **Rotate certificates before expiry**
   - Self-signed: 365 days → rotate at 30 days remaining
   - Let's Encrypt: 90 days → rotate at 30 days remaining

5. **Monitor certificate expiry**
   - Set up alerts at 30 days and 7 days before expiry
   - Test renewal process in staging environment

6. **Backup private keys securely**
   - Encrypt backups with strong passphrase
   - Store offline (USB drive, hardware security module)
   - Document recovery procedure

## Future Enhancements (Phase 2+)

- [ ] **Hardware Security Module (HSM) integration** for private key storage
- [ ] **Certificate pinning** beyond TOFU (public key pinning)
- [ ] **ACME DNS-01 challenge** for wildcard certificates
- [ ] **Certificate transparency log monitoring** (CT logs)
- [ ] **Automated key rotation** (separate from cert rotation)
- [ ] **Multi-certificate support** (SNI for multiple domains)

---

**Questions or Issues?**
- File an issue: https://github.com/monoterminal/monoterminal/issues
- Security concerns: security@monoterminal.dev (not created yet - placeholder)
