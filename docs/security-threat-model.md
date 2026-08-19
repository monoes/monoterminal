# MONOTERMINAL Security Threat Model
**Version:** 1.0 (Phase 1)  
**Scope:** Windows master + Web client (PWA)  
**Date:** 2026-08-14  
**Owner:** security-engineer

## Executive Summary

This document identifies security threats, attack surfaces, and mitigations for MONOTERMINAL Phase 1. The threat model follows the STRIDE methodology (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege) and focuses on the trust boundaries introduced by network-accessible terminal sessions.

## System Overview

**Phase 1 Architecture:**
- **Master Daemon** (Windows): Rust process managing PTY sessions, WebSocket server with TLS 1.3
- **Web Client** (PWA): React + xterm.js browser application
- **Communication:** WebSocket over TLS 1.3, Protocol Buffers, Ed25519 + JWT authentication
- **Storage:** SQLite database (sessions, scrollback, permissions, audit logs)

**Out of Scope (Phase 1):**
- P2P WebRTC networking (Phase 2)
- Linux/macOS master (Phase 3)
- Enterprise SAML/OAuth (Phase 4)

## Trust Boundaries

```
┌─────────────────────────────────────────────────────────┐
│                    Untrusted Network                    │
│                                                         │
│  ┌──────────────┐         TLS 1.3         ┌──────────┐ │
│  │ Web Client   │ ◄──────────────────────► │  Master  │ │
│  │  (Browser)   │    Ed25519 + JWT Auth    │  Daemon  │ │
│  └──────────────┘                          └─────┬────┘ │
│                                                  │      │
└──────────────────────────────────────────────────┼──────┘
                                                   │
                           ┌───────────────────────┼──────────────────┐
                           │    Trusted Local System                  │
                           │                       ▼                  │
                           │              ┌─────────────────┐         │
                           │              │  PTY Processes  │         │
                           │              │  (ConPTY)       │         │
                           │              └─────────────────┘         │
                           │                       │                  │
                           │                       ▼                  │
                           │              ┌─────────────────┐         │
                           │              │ SQLite Database │         │
                           │              │ (sessions,      │         │
                           │              │  permissions)   │         │
                           │              └─────────────────┘         │
                           │                       │                  │
                           │                       ▼                  │
                           │              ┌─────────────────┐         │
                           │              │ Ed25519 Keys    │         │
                           │              │ (~/.ssh/)       │         │
                           │              └─────────────────┘         │
                           └──────────────────────────────────────────┘
```

**Trust Boundary 1:** Network Edge (Browser ↔ Master)
- **Threats:** MITM, eavesdropping, session hijacking
- **Controls:** TLS 1.3 only, Ed25519 auth, JWT with short TTL

**Trust Boundary 2:** Process Isolation (Master ↔ PTY)
- **Threats:** Command injection, privilege escalation
- **Controls:** PTY sandboxing, input sanitization

**Trust Boundary 3:** File System (Master ↔ SQLite/Keys)
- **Threats:** Unauthorized file access, data tampering
- **Controls:** File permissions (0600 for keys), SQLite PRAGMA secure_delete

## Threat Analysis (STRIDE)

### 1. Spoofing

**T1.1: Attacker impersonates legitimate user**
- **Attack:** Stolen Ed25519 private key
- **Impact:** HIGH - Full access to user's sessions
- **Likelihood:** MEDIUM
- **Mitigations:**
  - Private key file permissions: 0600 (owner-only)
  - Key passphrase encryption (future enhancement)
  - Audit logging of all auth attempts
- **Residual Risk:** MEDIUM

**T1.2: Attacker impersonates master daemon**
- **Attack:** MITM with fake TLS certificate
- **Impact:** HIGH - Capture credentials, session data
- **Likelihood:** LOW (TOFU model in Phase 1)
- **Mitigations:**
  - TLS 1.3 with certificate validation
  - TOFU (Trust On First Use) - client pins cert on first connection
  - Certificate mismatch warning to user
- **Residual Risk:** LOW
- **Note:** Production deployments should use Let's Encrypt (Phase 2+)

### 2. Tampering

**T2.1: Attacker modifies data in transit**
- **Attack:** MITM tampering with WebSocket messages
- **Impact:** HIGH - Inject commands, manipulate output
- **Likelihood:** LOW
- **Mitigations:**
  - TLS 1.3 with AEAD ciphers (GCM, ChaCha20-Poly1305)
  - Message sequence numbers (prevents replay)
  - Protobuf validation on deserialization
- **Residual Risk:** LOW

**T2.2: Attacker modifies SQLite database**
- **Attack:** Direct file system access to database
- **Impact:** HIGH - Modify permissions, inject scrollback, delete audit logs
- **Likelihood:** MEDIUM (if attacker has local access)
- **Mitigations:**
  - Database file permissions: 0600
  - SQLite PRAGMA secure_delete=ON (prevent data recovery)
  - Integrity checks on critical tables (checksums)
- **Residual Risk:** MEDIUM
- **Note:** Local access is assumed compromise - focus on detection

**T2.3: Attacker modifies Ed25519 private key**
- **Attack:** Replace legitimate key with attacker-controlled key
- **Impact:** HIGH - Persistent backdoor
- **Likelihood:** LOW
- **Mitigations:**
  - Key file permissions: 0600
  - File integrity monitoring (future)
  - Public key fingerprint display in client
- **Residual Risk:** LOW

### 3. Repudiation

**T3.1: User denies performing privileged action**
- **Attack:** User claims "I didn't kill that session"
- **Impact:** MEDIUM - Accountability gap
- **Likelihood:** MEDIUM
- **Mitigations:**
  - Comprehensive audit logging (user, action, timestamp, result)
  - Tamper-evident logs (append-only, cryptographic hashing future)
  - Retention policy: 90 days minimum
- **Residual Risk:** LOW

**T3.2: Attacker deletes audit logs**
- **Attack:** Direct database access to delete evidence
- **Impact:** HIGH - No forensic trail
- **Likelihood:** MEDIUM (requires local access)
- **Mitigations:**
  - Audit log forwarding to external SIEM (future)
  - Database file permissions: 0600
  - Daily backup of audit logs
- **Residual Risk:** MEDIUM

### 4. Information Disclosure

**T4.1: Eavesdropping on terminal sessions**
- **Attack:** Network packet capture
- **Impact:** HIGH - Expose credentials, sensitive data
- **Likelihood:** MEDIUM (public WiFi, corporate networks)
- **Mitigations:**
  - TLS 1.3 mandatory (no plaintext fallback)
  - Forward secrecy (ephemeral keys)
  - Encrypted SNI
- **Residual Risk:** LOW

**T4.2: Scrollback data leakage**
- **Attack:** Read SQLite database or memory dumps
- **Impact:** HIGH - Historical command history, secrets
- **Likelihood:** MEDIUM (requires local access or memory dump)
- **Mitigations:**
  - Database encryption (future - SQLCipher)
  - Memory zeroing for sensitive buffers
  - Scrollback TTL/size limits
- **Residual Risk:** MEDIUM
- **Future:** Implement scrollback encryption in Phase 2

**T4.3: JWT token leakage**
- **Attack:** XSS in web client, token theft via browser storage
- **Impact:** HIGH - Session hijacking
- **Likelihood:** MEDIUM
- **Mitigations:**
  - HttpOnly cookies for refresh tokens (web client best practice)
  - Short TTL for access tokens (15 min)
  - Refresh token rotation with reuse detection
  - Content Security Policy (CSP) headers
- **Residual Risk:** LOW

**T4.4: Ed25519 private key disclosure**
- **Attack:** Read key file, memory dump, shoulder surfing
- **Impact:** HIGH - Permanent credential compromise
- **Likelihood:** LOW
- **Mitigations:**
  - File permissions: 0600
  - No key logging in debug output
  - Memory protection (mlock for key material - future)
- **Residual Risk:** LOW

### 5. Denial of Service

**T5.1: Connection flood**
- **Attack:** Open thousands of connections rapidly
- **Impact:** MEDIUM - Legitimate users can't connect
- **Likelihood:** HIGH (trivial attack)
- **Mitigations:**
  - Rate limiting: 100 new connections/min per IP
  - Connection limit: 1000 total concurrent
  - Per-session limit: 50 clients
- **Residual Risk:** LOW

**T5.2: Authentication brute force**
- **Attack:** Attempt many Ed25519 signatures with different keys
- **Impact:** LOW (computationally infeasible to guess key)
- **Likelihood:** MEDIUM (automated attacks)
- **Mitigations:**
  - Rate limiting: 5 auth attempts/hour per IP
  - 15-minute temporary ban after 5 failures
  - Exponential backoff (future)
- **Residual Risk:** LOW

**T5.3: Session create flood**
- **Attack:** Create thousands of sessions to exhaust resources
- **Impact:** HIGH - System OOM, disk full
- **Likelihood:** MEDIUM
- **Mitigations:**
  - Rate limiting: 20 session creates/min
  - Max sessions per user (configurable, default 100)
  - Session TTL and auto-cleanup
- **Residual Risk:** LOW

**T5.4: Input flood**
- **Attack:** Send megabytes of input to overwhelm PTY
- **Impact:** MEDIUM - High CPU, unresponsive sessions
- **Likelihood:** HIGH
- **Mitigations:**
  - Input rate limiting: 10 KB/s per session
  - Backpressure on slow PTY consumers
  - Circuit breaker for runaway sessions
- **Residual Risk:** LOW

**T5.5: SQLite database lock contention**
- **Attack:** Concurrent write floods to lock database
- **Impact:** MEDIUM - Legitimate operations blocked
- **Likelihood:** LOW
- **Mitigations:**
  - WAL mode (concurrent readers)
  - Connection pooling
  - Async writes for non-critical data
- **Residual Risk:** LOW

### 6. Elevation of Privilege

**T6.1: RBAC bypass**
- **Attack:** ReadOnly user sends input to session
- **Impact:** HIGH - Unauthorized command execution
- **Likelihood:** LOW (requires code bug)
- **Mitigations:**
  - Permission checks on every action (Attach, Input, Resize, Kill)
  - Comprehensive unit tests for all action×role combinations
  - Fail-closed design (deny by default)
- **Residual Risk:** LOW

**T6.2: JWT scope manipulation**
- **Attack:** Modify JWT claims to grant higher permissions
- **Impact:** HIGH - Admin access
- **Likelihood:** LOW (requires breaking EdDSA signature)
- **Mitigations:**
  - Cryptographic signature validation (EdDSA)
  - No client-side trust of claims without signature check
  - Scope validation against known roles
- **Residual Risk:** LOW

**T6.3: Session owner override**
- **Attack:** Non-owner user kills or modifies session
- **Impact:** HIGH - Disrupt other users' work
- **Likelihood:** LOW (requires RBAC bug)
- **Mitigations:**
  - Owner check before privileged operations
  - Session ownership immutable (set at creation)
  - Audit logging of all permission changes
- **Residual Risk:** LOW

**T6.4: Command injection via PTY**
- **Attack:** Inject shell metacharacters through unsanitized input
- **Impact:** HIGH - Arbitrary command execution
- **Likelihood:** LOW (ConPTY handles escaping)
- **Mitigations:**
  - ConPTY API usage (not raw shell commands)
  - No direct shell interpolation of user input
  - PTY runs as user's normal permissions (not root)
- **Residual Risk:** LOW
- **Note:** Phase 1 runs master as user process, not system service

## Attack Surface Analysis

### Network Services

**WebSocket Server (TLS 1.3)**
- **Endpoint:** `wss://localhost:8443` (configurable)
- **Authentication:** Ed25519 challenge-response + JWT
- **Attack Vectors:**
  - TLS downgrade (mitigated: TLS 1.3 only)
  - Certificate validation bypass (mitigated: TOFU)
  - JWT token theft (mitigated: short TTL, rotation)
- **Surface Area:** MEDIUM

### File System

**Ed25519 Private Key**
- **Path:** `~/.ssh/monoterminal_ed25519`
- **Permissions:** 0600
- **Attack Vectors:**
  - File read by other users (mitigated: permissions)
  - Backup/sync leakage (user responsibility)
  - Memory dump (future: mlock)
- **Surface Area:** HIGH (most critical asset)

**SQLite Database**
- **Path:** `~/.monoterminal/sessions.db`
- **Permissions:** 0600
- **Attack Vectors:**
  - Direct file modification (mitigated: permissions, checksums)
  - SQL injection (mitigated: parameterized queries, rusqlite)
  - Data recovery from deleted records (mitigated: secure_delete)
- **Surface Area:** MEDIUM

### Process Boundaries

**PTY Processes**
- **Isolation:** ConPTY (Windows pseudo-console)
- **Privileges:** User-level (not SYSTEM)
- **Attack Vectors:**
  - Command injection (mitigated: ConPTY API, no shell interpolation)
  - Resource exhaustion (mitigated: rate limits)
  - Output parsing vulnerabilities (mitigated: binary-safe handling)
- **Surface Area:** LOW (well-defined OS API)

## Security Controls Summary

| Control | Type | Coverage |
|---------|------|----------|
| **TLS 1.3** | Preventive | T1.2, T2.1, T4.1 |
| **Ed25519 Authentication** | Preventive | T1.1, T6.2 |
| **JWT Short TTL** | Preventive | T4.3 |
| **Refresh Token Rotation** | Preventive | T4.3 |
| **Rate Limiting** | Preventive | T5.1, T5.2, T5.3, T5.4 |
| **RBAC** | Preventive | T6.1, T6.3 |
| **File Permissions** | Preventive | T2.2, T2.3, T4.2, T4.4 |
| **Audit Logging** | Detective | T3.1, T3.2 |
| **Input Validation** | Preventive | T6.4 |
| **Message Sequence Numbers** | Preventive | T2.1 (replay) |

## Assumptions

1. **Local System Security:** Master daemon runs on a trustworthy OS. If attacker has local admin/root, game over.
2. **Browser Security:** Web client runs in a modern, patched browser with standard XSS protections.
3. **User Key Management:** Users protect their Ed25519 private keys (not shared, not committed to git).
4. **Network Perimeter:** Phase 1 assumes private networks or VPN. Public internet exposure requires additional hardening (Phase 2+).
5. **Physical Security:** No protection against physical access to the master machine (disk encryption user's responsibility).

## Residual Risks

| Risk | Impact | Likelihood | Acceptance Rationale |
|------|--------|------------|----------------------|
| **Local file access** | HIGH | MEDIUM | Mitigated by OS permissions; Phase 1 MVP acceptable |
| **SQLite data recovery** | MEDIUM | LOW | Secure_delete enabled; full encryption in Phase 2 |
| **TOFU certificate trust** | MEDIUM | LOW | Let's Encrypt in production (Phase 2+); dev TOFU acceptable |
| **Browser XSS** | HIGH | LOW | CSP + modern framework protections; pentest in Phase 2 |

## Security Roadmap

**Phase 1 (Current):**
- ✅ TLS 1.3 enforcement
- ✅ Ed25519 + JWT authentication
- ✅ RBAC
- ✅ Rate limiting
- ✅ Audit logging

**Phase 2:**
- [ ] SQLite database encryption (SQLCipher)
- [ ] Security penetration testing
- [ ] Bug bounty program
- [ ] Let's Encrypt ACME integration
- [ ] Scrollback encryption at rest

**Phase 3:**
- [ ] Multi-factor authentication (TOTP)
- [ ] Hardware key support (YubiKey, FIDO2)
- [ ] Certificate pinning (beyond TOFU)

**Phase 4:**
- [ ] SAML/OAuth SSO
- [ ] FIPS 140-2 compliance validation
- [ ] SOC 2 audit preparation

## Incident Response Plan

**Detection:**
1. Monitor audit logs for:
   - Failed auth attempts (>5 per hour)
   - Permission denied errors (potential RBAC bypass)
   - Unexpected session kills
   - Database integrity check failures

**Response:**
1. **Severity Assessment:** Critical (auth bypass) vs Medium (DoS) vs Low (single failed login)
2. **Containment:**
   - Revoke compromised JWT tokens
   - Rotate Ed25519 keys if private key compromised
   - Temporarily ban attacking IPs
3. **Investigation:**
   - Export audit logs
   - Analyze SQLite database for tampering
   - Review system logs for lateral movement
4. **Recovery:**
   - Restore from backups if database corrupted
   - Re-issue tokens to legitimate users
   - Apply security patches
5. **Lessons Learned:**
   - Post-mortem document
   - Update threat model
   - Implement additional controls

## Compliance Notes

**Phase 1 Compliance:**
- ❌ SOC 2 (no formal audit yet)
- ❌ FIPS 140-2 (optional feature flag, not validated)
- ✅ GDPR readiness (audit logs, data minimization)

**Enterprise Readiness (Phase 4+):**
- SOC 2 Type II audit
- FIPS 140-2 validated crypto modules
- HIPAA technical safeguards

## Review and Update

**Frequency:** Quarterly or after major architecture changes  
**Next Review:** 2026-11-14 (90 days)  
**Reviewers:** security-engineer, principal-architect, external security consultant (future)

---

**Document Status:** DRAFT - awaiting task-1 (architecture) completion and implementation start
