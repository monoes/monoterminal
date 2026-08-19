# Engineering Decisions — Phase 1

**Date:** 2026-08-14  
**Source:** eng-director  
**Status:** APPROVED

---

## Windows Service Configuration

**Decision:** MANUAL start (not AUTO)

**Rationale:**
- Less intrusive for development and testing
- Users can manually start service when needed
- Can change to AUTO in Phase 2+ based on user feedback

**Implementation:**
```powershell
# Service registration
sc.exe create MonoTerminal `
    binPath= "C:\Program Files\MonoTerminal\monoterminal-master.exe" `
    start= demand `  # MANUAL, not auto
    DisplayName= "MONOTERMINAL Master Daemon"
```

**Documentation:** Include in docs/DEVELOPMENT.md how to enable AUTO start if desired.

---

## Default Shell Detection

**Decision:** Detect in this order:
1. PowerShell 7+ (`pwsh.exe`) if installed
2. Fallback to `cmd.exe` (guaranteed on all Windows)
3. Skip PowerShell 5 (`powershell.exe`) - legacy, avoid

**Rationale:**
- `pwsh.exe` is modern, cross-platform PowerShell
- `cmd.exe` is universal fallback (always present)
- PowerShell 5 is legacy (avoid if possible)

**Implementation:**
```rust
fn detect_default_shell() -> PathBuf {
    // 1. Try PowerShell 7+
    if let Some(pwsh) = which::which("pwsh.exe").ok() {
        return pwsh;
    }
    
    // 2. Fallback to cmd.exe
    PathBuf::from("C:\\Windows\\System32\\cmd.exe")
}
```

**Configuration:** Must be overridable in `config.toml`:
```toml
[terminal]
shell = "C:\\Program Files\\PowerShell\\7\\pwsh.exe"  # Override detection
```

---

## Logging Configuration

**Decision:** File logging for Phase 1

**Location:** `%LOCALAPPDATA%\monoterminal\logs\master.log`

**Rotation:**
- Max file size: 10 MB
- Max files: 5 (master.log, master.log.1, ..., master.log.4)
- Total max disk usage: 50 MB

**Log Levels:**
- Default: INFO
- Verbose mode: DEBUG (via `--verbose` flag or config)

**Rationale:**
- Easiest for users to access (no Event Viewer required)
- Works for Windows Service (stdout not visible)
- Windows Event Log can be Phase 2+ feature

**Implementation:**
```rust
use tracing_subscriber::fmt;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

let file_appender = RollingFileAppender::builder()
    .rotation(Rotation::NEVER)  // Manual rotation at 10MB
    .max_log_files(5)
    .filename_prefix("master")
    .filename_suffix("log")
    .build(log_dir)?;

tracing_subscriber::fmt()
    .with_writer(file_appender)
    .with_max_level(tracing::Level::INFO)
    .init();
```

---

## TLS Certificate Strategy

**Decision:** Auto-generate self-signed certificate for localhost on first run

**Location:** `%LOCALAPPDATA%\monoterminal\certs\`
- `cert.pem` (certificate)
- `key.pem` (private key)

**Rationale:**
- Minimize setup friction for Phase 1 local testing
- Self-signed is acceptable for localhost connections
- Let's Encrypt integration can be Phase 2+ for production deployments

**Implementation:**
```rust
use rcgen::{generate_simple_self_signed, CertifiedKey};

fn ensure_tls_cert(cert_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");
    
    if cert_path.exists() && key_path.exists() {
        return Ok((cert_path, key_path));
    }
    
    // Generate self-signed cert
    let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    let cert = generate_simple_self_signed(subject_alt_names)?;
    
    fs::write(&cert_path, cert.serialize_pem()?)?;
    fs::write(&key_path, cert.serialize_private_key_pem())?;
    
    Ok((cert_path, key_path))
}
```

**User Documentation:** Document browser warning workflow:
> **First connection warning:** Your browser will show a security warning because the certificate is self-signed. Click "Advanced" → "Proceed to localhost (unsafe)" to continue. This is expected behavior for local development.

**Phase 2+ Production:** Add Let's Encrypt integration for public-facing deployments.

---

## Summary

| Decision | Phase 1 | Phase 2+ |
|----------|---------|----------|
| **Service Start** | MANUAL | AUTO (based on feedback) |
| **Default Shell** | pwsh.exe → cmd.exe | Same (may add bash/WSL) |
| **Logging** | File (10MB rotation) | File + Windows Event Log |
| **TLS Cert** | Auto-gen self-signed | Let's Encrypt option |

---

**Implementation Teams:**

These decisions are binding for Phase 1 implementation. All teams (rust-backend-lead, security-engineer, devops-lead) should implement accordingly.

**Change Process:**

To change any of these decisions, escalate to eng-director with:
- Rationale for change
- Impact on timeline
- Affected teams

---

**END OF DOCUMENT**
