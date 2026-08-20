# Phase 3 Documentation Plan

**Task:** task-60  
**Date:** 2026-08-20  
**Owner:** technical-writer (planned by qa-lead)  
**Phase:** Phase 3 Weeks 11-12  
**Status:** Planning

---

## Executive Summary

This document defines the comprehensive documentation strategy for Phase 3 (Linux + macOS platform expansion), covering user-facing installation guides, developer onboarding materials, architecture documentation updates, and migration guides for cross-platform deployment.

**Scope:** 15+ documentation deliverables across 3 categories (User, Developer, Architecture)

**Timeline:** Weeks 11-12 (parallel with integration testing)

---

## 1. Documentation Objectives

### 1.1 Primary Goals

1. **Enable Self-Service Installation**
   - Users can install on any platform without support
   - Clear, platform-specific installation guides
   - Troubleshooting guides for common issues

2. **Onboard Contributors**
   - Developers can contribute cross-platform code
   - Clear development environment setup
   - Platform-specific quirks documented

3. **Document Cross-Platform Architecture**
   - ADRs for cross-platform design decisions
   - PTY abstraction design
   - Service management design

4. **Support Migration**
   - Users can migrate from Windows to Linux/macOS
   - Session data migration guide
   - Configuration migration guide

---

## 2. Documentation Categories

### 2.1 User Documentation (10 documents)

| Document | Platform | Audience | Priority |
|----------|----------|----------|----------|
| Installation Guide (Ubuntu) | Linux | End Users | HIGH |
| Installation Guide (Debian) | Linux | End Users | HIGH |
| Installation Guide (Fedora) | Linux | End Users | HIGH |
| Installation Guide (macOS) | macOS | End Users | HIGH |
| Service Management Guide (systemd) | Linux | System Admins | HIGH |
| Service Management Guide (launchd) | macOS | System Admins | HIGH |
| Troubleshooting Guide (Linux) | Linux | End Users | MEDIUM |
| Troubleshooting Guide (macOS) | macOS | End Users | MEDIUM |
| Migration Guide (Windows → Linux) | Cross-platform | End Users | MEDIUM |
| FAQ (Cross-Platform) | All | End Users | LOW |

### 2.2 Developer Documentation (8 documents)

| Document | Platform | Audience | Priority |
|----------|----------|----------|----------|
| Development Environment Setup (Linux) | Linux | Contributors | HIGH |
| Development Environment Setup (macOS) | macOS | Contributors | HIGH |
| Cross-Platform Development Guide | All | Contributors | HIGH |
| PTY Abstraction Guide | All | Core Contributors | HIGH |
| Testing Guide (Cross-Platform) | All | Contributors | HIGH |
| Contributing Guide (Cross-Platform) | All | New Contributors | MEDIUM |
| Code Review Checklist (Cross-Platform) | All | Reviewers | MEDIUM |
| Release Process (Cross-Platform) | All | Maintainers | MEDIUM |

### 2.3 Architecture Documentation (5 documents)

| Document | Platform | Audience | Priority |
|----------|----------|----------|----------|
| Phase 3 Architecture Overview | All | Architects | HIGH |
| ADR-015: Cross-Platform PTY Abstraction | All | Engineers | HIGH |
| ADR-016: Service Management Design | All | Engineers | HIGH |
| ADR-017: Distribution Packaging Strategy | All | DevOps | HIGH |
| Cross-Platform Design Decisions | All | Engineers | MEDIUM |

---

## 3. User Documentation Details

### 3.1 Installation Guide (Ubuntu)

**File:** `docs/installation/ubuntu.md`  
**Length:** ~800 words  
**Sections:**

1. **Prerequisites**
   - Ubuntu 22.04 or later
   - `sudo` access
   - Internet connection

2. **Installation Methods**
   - **Method 1: APT Repository (Recommended)**
     ```bash
     # Add monoterminal repository
     curl -fsSL https://packages.monoterminal.dev/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/monoterminal-archive-keyring.gpg
     echo "deb [signed-by=/usr/share/keyrings/monoterminal-archive-keyring.gpg] https://packages.monoterminal.dev/ubuntu $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/monoterminal.list
     
     # Install
     sudo apt update
     sudo apt install monoterminal
     ```
   
   - **Method 2: .deb Package**
     ```bash
     wget https://github.com/monoterminal/monoterminal/releases/download/v0.1.0/monoterminal_0.1.0_amd64.deb
     sudo dpkg -i monoterminal_0.1.0_amd64.deb
     sudo apt-get install -f  # Install dependencies
     ```
   
   - **Method 3: Build from Source**
     ```bash
     git clone https://github.com/monoterminal/monoterminal.git
     cd monoterminal
     cargo build --release
     sudo cp target/release/monoterminal /usr/local/bin/
     ```

3. **Service Setup**
   ```bash
   # Enable and start service
   sudo systemctl enable monoterminal
   sudo systemctl start monoterminal
   
   # Verify service status
   systemctl status monoterminal
   ```

4. **First Connection**
   - Open browser: `https://localhost:5000`
   - Accept self-signed certificate (or configure TLS)
   - Create first session

5. **Troubleshooting**
   - Service won't start: Check logs with `journalctl -u monoterminal -n 50`
   - Connection refused: Check firewall, verify port 5000 open
   - Permission denied: Ensure user in `monoterminal` group

6. **Uninstallation**
   ```bash
   sudo systemctl stop monoterminal
   sudo systemctl disable monoterminal
   sudo apt remove monoterminal
   ```

---

### 3.2 Installation Guide (macOS)

**File:** `docs/installation/macos.md`  
**Length:** ~700 words  
**Sections:**

1. **Prerequisites**
   - macOS 12 (Monterey) or later
   - Homebrew installed (https://brew.sh)

2. **Installation via Homebrew**
   ```bash
   # Add tap
   brew tap monoterminal/tap
   
   # Install
   brew install monoterminal
   
   # Start service
   brew services start monoterminal
   ```

3. **Manual Installation**
   ```bash
   # Download .pkg installer
   curl -LO https://github.com/monoterminal/monoterminal/releases/download/v0.1.0/monoterminal-0.1.0-macos-universal.pkg
   
   # Install
   sudo installer -pkg monoterminal-0.1.0-macos-universal.pkg -target /
   ```

4. **Service Management (launchd)**
   ```bash
   # Start service
   launchctl load ~/Library/LaunchAgents/com.monoterminal.daemon.plist
   
   # Check status
   launchctl list | grep monoterminal
   
   # Stop service
   launchctl unload ~/Library/LaunchAgents/com.monoterminal.daemon.plist
   ```

5. **First Connection**
   - Open Safari: `https://localhost:5000`
   - Accept self-signed certificate
   - Create first session

6. **macOS-Specific Notes**
   - **Apple Silicon (M1/M2):** Universal binary (Intel + ARM64)
   - **HiDPI (Retina):** Automatically scales to 2x/3x
   - **Gatekeeper:** Right-click → Open if unsigned

7. **Troubleshooting**
   - Service won't start: Check Console.app logs
   - Permission denied: Grant Terminal.app Full Disk Access (System Preferences → Security)
   - Firewall blocking: Allow monoterminal in System Preferences → Security → Firewall

---

### 3.3 Service Management Guide (systemd)

**File:** `docs/service-management/systemd.md`  
**Length:** ~600 words  
**Sections:**

1. **Service Overview**
   - systemd Type=notify service
   - Socket activation support
   - Automatic restart on failure

2. **Service Unit File**
   ```ini
   [Unit]
   Description=Monoterminal Master Daemon
   After=network.target
   
   [Service]
   Type=notify
   ExecStart=/usr/bin/monoterminal daemon
   Restart=always
   RestartSec=5s
   User=monoterminal
   Group=monoterminal
   Environment="RUST_LOG=info"
   
   # Resource limits
   LimitNOFILE=65536
   LimitNPROC=512
   
   [Install]
   WantedBy=multi-user.target
   ```

3. **Service Management Commands**
   ```bash
   # Start/stop/restart
   sudo systemctl start monoterminal
   sudo systemctl stop monoterminal
   sudo systemctl restart monoterminal
   
   # Enable/disable (auto-start on boot)
   sudo systemctl enable monoterminal
   sudo systemctl disable monoterminal
   
   # Status and logs
   systemctl status monoterminal
   journalctl -u monoterminal -f  # Follow logs
   journalctl -u monoterminal -n 100  # Last 100 lines
   ```

4. **Socket Activation**
   - Service starts on first connection (not boot)
   - Saves resources on idle systems
   - Configured via `monoterminal.socket` unit

5. **Troubleshooting**
   - Service fails to start: Check `systemctl status` and `journalctl`
   - Permission errors: Verify `monoterminal` user exists
   - Port conflicts: Check if port 5000 already in use

---

### 3.4 Migration Guide (Windows → Linux)

**File:** `docs/migration/windows-to-linux.md`  
**Length:** ~500 words  
**Sections:**

1. **Overview**
   - Migrate sessions, configuration, and data from Windows to Linux
   - Zero downtime migration (sessions preserved)

2. **Pre-Migration Checklist**
   - [ ] Backup Windows data directory: `C:\Users\<user>\.monoterminal\`
   - [ ] Note active sessions: `monoterminal-cli list-sessions`
   - [ ] Export configuration: `monoterminal-cli config export > config.toml`

3. **Migration Steps**
   
   **Step 1: Export from Windows**
   ```powershell
   # Export SQLite database
   Copy-Item "$env:USERPROFILE\.monoterminal\monoterminal.db" -Destination "monoterminal-backup.db"
   
   # Export configuration
   Copy-Item "$env:USERPROFILE\.monoterminal\config.toml" -Destination "config-backup.toml"
   ```
   
   **Step 2: Transfer to Linux**
   ```bash
   # SCP from Windows to Linux
   scp monoterminal-backup.db user@linux-host:~/
   scp config-backup.toml user@linux-host:~/
   ```
   
   **Step 3: Import on Linux**
   ```bash
   # Stop daemon
   sudo systemctl stop monoterminal
   
   # Restore database
   cp ~/monoterminal-backup.db ~/.monoterminal/monoterminal.db
   
   # Restore configuration
   cp ~/config-backup.toml ~/.monoterminal/config.toml
   
   # Start daemon
   sudo systemctl start monoterminal
   ```

4. **Session Recovery**
   - All sessions will be in DETACHED state
   - Reconnect via web client
   - Sessions will resume from last state

5. **Configuration Adjustments**
   - Update paths: `C:\` → `/home/user/`
   - Update shell: `cmd.exe` → `/bin/bash`
   - Update TLS cert paths (if custom certs used)

6. **Verification**
   ```bash
   # List recovered sessions
   monoterminal-cli list-sessions
   
   # Verify configuration
   monoterminal-cli config show
   ```

---

### 3.5 Troubleshooting Guide (Linux)

**File:** `docs/troubleshooting/linux.md`  
**Length:** ~1000 words  
**Sections:**

1. **Service Won't Start**
   - **Symptom:** `systemctl start monoterminal` fails
   - **Diagnosis:** Check logs: `journalctl -u monoterminal -n 50`
   - **Common Causes:**
     - Port 5000 already in use (check with `sudo lsof -i :5000`)
     - Missing dependencies (reinstall with `apt install -f`)
     - Permission issues (verify `monoterminal` user exists)
   - **Fix:** Kill conflicting process, reinstall dependencies, create user

2. **Connection Refused**
   - **Symptom:** Browser shows "Connection refused" at `localhost:5000`
   - **Diagnosis:**
     - Verify service running: `systemctl status monoterminal`
     - Check port binding: `sudo ss -tlnp | grep 5000`
     - Check firewall: `sudo ufw status`
   - **Fix:** Start service, open port 5000 in firewall

3. **PTY Errors**
   - **Symptom:** "Failed to create PTY" error
   - **Diagnosis:** Check `/dev/pts` mounted, permissions on `/dev/ptmx`
   - **Fix:**
     ```bash
     # Verify /dev/pts mounted
     mount | grep devpts
     
     # Fix permissions
     sudo chmod 666 /dev/ptmx
     ```

4. **Rendering Issues**
   - **Symptom:** Blank screen, low FPS, graphical glitches
   - **Diagnosis:**
     - Check GPU: `lspci | grep VGA`
     - Check Vulkan: `vulkaninfo`
     - Check driver: `glxinfo | grep "OpenGL renderer"`
   - **Fix:** Install Vulkan drivers, update GPU drivers

5. **High Memory Usage**
   - **Symptom:** RSS >500MB with few sessions
   - **Diagnosis:** Check session count, scrollback size
   - **Fix:** Reduce scrollback size in config, restart daemon

---

## 4. Developer Documentation Details

### 4.1 Cross-Platform Development Guide

**File:** `docs/development/cross-platform.md`  
**Length:** ~1200 words  
**Sections:**

1. **Development Environment Setup**
   
   **Prerequisites (All Platforms):**
   - Rust 1.97+ (stable)
   - Node.js 20+ (for web client)
   - Git 2.30+
   
   **Linux:**
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install dependencies
   sudo apt install build-essential pkg-config libssl-dev libvulkan-dev
   
   # Clone and build
   git clone https://github.com/monoterminal/monoterminal.git
   cd monoterminal
   cargo build
   ```
   
   **macOS:**
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   
   # Install Xcode Command Line Tools
   xcode-select --install
   
   # Clone and build
   git clone https://github.com/monoterminal/monoterminal.git
   cd monoterminal
   cargo build
   ```

2. **Platform Abstraction Layers**
   
   **PTY Abstraction:**
   - `src/pty/mod.rs` - Platform-agnostic trait
   - `src/pty/unix.rs` - Linux/macOS implementation (openpty)
   - `src/pty/conpty.rs` - Windows implementation (ConPTY)
   
   **Service Management:**
   - `src/service/systemd.rs` - Linux systemd integration
   - `src/service/launchd.rs` - macOS launchd integration
   - `src/service/windows.rs` - Windows Service integration

3. **Platform-Specific Code Patterns**
   ```rust
   // Use cfg attributes for platform-specific code
   #[cfg(unix)]
   use crate::pty::unix::UnixPty;
   
   #[cfg(windows)]
   use crate::pty::conpty::ConPty;
   
   // Platform-specific implementations
   #[cfg(unix)]
   fn create_pty() -> Box<dyn Pty> {
       Box::new(UnixPty::new())
   }
   
   #[cfg(windows)]
   fn create_pty() -> Box<dyn Pty> {
       Box::new(ConPty::new())
   }
   ```

4. **Cross-Platform Testing**
   ```bash
   # Run tests on all platforms
   cargo test --all
   
   # Platform-specific tests
   cargo test --target x86_64-unknown-linux-gnu
   cargo test --target x86_64-apple-darwin
   cargo test --target x86_64-pc-windows-msvc
   ```

5. **CI/CD Integration**
   - GitHub Actions matrix for all platforms
   - Automated testing on PR
   - Cross-platform build verification

6. **Common Pitfalls**
   - **File paths:** Use `std::path::Path` for cross-platform paths
   - **Line endings:** Use `.gitattributes` to normalize CRLF/LF
   - **Process spawning:** Different shell commands per platform
   - **Permissions:** Unix-specific (chmod, chown don't exist on Windows)

---

### 4.2 PTY Abstraction Guide

**File:** `docs/development/pty-abstraction.md`  
**Length:** ~1000 words  
**Sections:**

1. **Overview**
   - PTY abstraction design pattern
   - Platform-specific implementations
   - Common trait interface

2. **Pty Trait Interface**
   ```rust
   pub trait Pty {
       fn spawn(&mut self, cmd: &str, args: &[&str]) -> Result<()>;
       fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
       fn write(&self, buf: &[u8]) -> Result<usize>;
       fn resize(&self, rows: u16, cols: u16) -> Result<()>;
       fn kill(&mut self) -> Result<()>;
   }
   ```

3. **Unix Implementation (openpty)**
   ```rust
   // crates/master/src/pty/unix.rs
   pub struct UnixPty {
       master_fd: RawFd,
       child_pid: Pid,
   }
   
   impl Pty for UnixPty {
       fn spawn(&mut self, cmd: &str, args: &[&str]) -> Result<()> {
           // openpty() + fork() + exec()
       }
   }
   ```

4. **Windows Implementation (ConPTY)**
   ```rust
   // crates/master/src/pty/conpty.rs
   pub struct ConPty {
       conpty_handle: HPCON,
       process_handle: HANDLE,
   }
   
   impl Pty for ConPty {
       fn spawn(&mut self, cmd: &str, args: &[&str]) -> Result<()> {
           // CreatePseudoConsole() + CreateProcess()
       }
   }
   ```

5. **Usage Example**
   ```rust
   #[cfg(unix)]
   let mut pty = UnixPty::new()?;
   
   #[cfg(windows)]
   let mut pty = ConPty::new()?;
   
   pty.spawn("/bin/bash", &[])?;
   pty.resize(24, 80)?;
   pty.write(b"echo hello\\n")?;
   ```

6. **Testing Strategy**
   - Unit tests per platform
   - Integration tests via trait interface
   - Platform-specific edge cases

---

### 4.3 Testing Guide (Cross-Platform)

**File:** `docs/development/testing.md`  
**Length:** ~800 words  
**Sections:**

1. **Test Organization**
   - Unit tests: `#[cfg(test)] mod tests { ... }`
   - Integration tests: `tests/*.rs`
   - Platform-specific tests: `#[cfg(unix)]`, `#[cfg(windows)]`

2. **Running Tests**
   ```bash
   # All tests
   cargo test
   
   # Specific test
   cargo test test_pty_spawn
   
   # Platform-specific
   cargo test --features unix-pty
   ```

3. **Coverage Measurement**
   ```bash
   # tarpaulin (Linux/macOS)
   cargo tarpaulin --out Html --output-dir coverage/
   
   # Manual projection (when tools hang)
   # See task-47 for methodology
   ```

4. **Cross-Platform Test Patterns**
   ```rust
   #[test]
   #[cfg(unix)]
   fn test_unix_pty() {
       // Unix-specific test
   }
   
   #[test]
   #[cfg(windows)]
   fn test_conpty() {
       // Windows-specific test
   }
   
   #[test]
   fn test_cross_platform() {
       // Works on all platforms
   }
   ```

5. **CI Matrix**
   - Ubuntu 22.04, Debian 11, Fedora 38
   - macOS 13 (Intel), macOS 14 (Apple Silicon)
   - Windows 2022, Windows latest

6. **Performance Tests**
   - Benchmarks in `benches/*.rs`
   - `cargo bench` for performance regression detection

---

## 5. Architecture Documentation Details

### 5.1 Phase 3 Architecture Overview

**File:** `docs/architecture/phase3-overview.md`  
**Length:** ~1200 words  
**Sections:**

1. **Phase 3 Goals**
   - Platform expansion (Linux + macOS)
   - Feature parity with Windows
   - Service management integration
   - Distribution packaging

2. **Architecture Diagram**
   ```
   ┌─────────────────────────────────────────┐
   │         Web Client (PWA)                │
   │   React + xterm.js + Protobuf           │
   └─────────────────┬───────────────────────┘
                     │ WebSocket + TLS
   ┌─────────────────▼───────────────────────┐
   │     Master Daemon (Rust)                │
   │  ┌────────────┬────────────┬──────────┐ │
   │  │ PTY Layer  │  Rendering │  Storage │ │
   │  │ (Unix/Win) │  (wgpu)    │ (SQLite) │ │
   │  └────────────┴────────────┴──────────┘ │
   └─────────────────┬───────────────────────┘
                     │
   ┌─────────────────▼───────────────────────┐
   │       Platform Services                 │
   │  systemd (Linux) | launchd (macOS)      │
   │  Windows Service                        │
   └─────────────────────────────────────────┘
   ```

3. **Cross-Platform Components**
   - **PTY Abstraction:** `src/pty/mod.rs`, `unix.rs`, `conpty.rs`
   - **Service Management:** `src/service/systemd.rs`, `launchd.rs`, `windows.rs`
   - **Rendering:** `src/rendering/mod.rs` (wgpu backends: Vulkan, Metal, DirectX 12)

4. **Platform-Specific Integration**
   - **Linux:** systemd Type=notify, socket activation, journald logging
   - **macOS:** launchd socket activation, syslog, HiDPI support
   - **Windows:** Windows Service, Event Log, ConPTY (baseline)

5. **Distribution Packaging**
   - **Linux:** .deb (apt), .rpm (yum/dnf)
   - **macOS:** .pkg (installer), Homebrew tap
   - **Windows:** .exe (installer), MSI (existing)

6. **Testing Strategy**
   - 405 tests across 6 platforms
   - 95% automation, 20-minute CI execution
   - See `phase3-integration-testing-strategy.md`

---

### 5.2 ADR-015: Cross-Platform PTY Abstraction

**File:** `docs/architecture/adr/015-pty-abstraction.md`  
**Length:** ~600 words  
**Sections:**

1. **Context**
   - Need cross-platform PTY support (Linux, macOS, Windows)
   - Different APIs per platform (openpty vs ConPTY)
   - Same interface required for all platforms

2. **Decision**
   - Use trait-based abstraction (`Pty` trait)
   - Platform-specific implementations behind trait
   - Compile-time selection via `cfg` attributes

3. **Implementation**
   ```rust
   pub trait Pty {
       fn spawn(&mut self, cmd: &str, args: &[&str]) -> Result<()>;
       fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
       fn write(&self, buf: &[u8]) -> Result<usize>;
       fn resize(&self, rows: u16, cols: u16) -> Result<()>;
   }
   
   #[cfg(unix)]
   pub type PlatformPty = UnixPty;
   
   #[cfg(windows)]
   pub type PlatformPty = ConPty;
   ```

4. **Consequences**
   - ✅ Clean, testable interface
   - ✅ Platform-specific optimizations possible
   - ✅ Easy to mock for testing
   - ❌ Some platform quirks leak through trait
   - ❌ Signal handling differs (Unix SIGWINCH vs Windows events)

5. **Alternatives Considered**
   - **Option 1:** Conditional compilation everywhere (rejected: messy)
   - **Option 2:** Runtime dispatch via `Box<dyn Pty>` (rejected: performance)
   - **Option 3:** Trait abstraction (selected: best balance)

---

### 5.3 ADR-016: Service Management Design

**File:** `docs/architecture/adr/016-service-management.md`  
**Length:** ~700 words  
**Sections:**

1. **Context**
   - Need daemon to run as system service
   - Different service managers per platform
   - Must support socket activation, auto-restart, logging

2. **Decision**
   - Platform-specific service managers:
     - Linux: systemd Type=notify + socket activation
     - macOS: launchd + socket activation
     - Windows: Windows Service API (existing)
   - Graceful shutdown (SIGTERM → 10s → sessions saved → exit)
   - Structured logging to system journal/syslog/Event Log

3. **systemd Integration**
   ```ini
   [Unit]
   Description=Monoterminal Master Daemon
   
   [Service]
   Type=notify
   ExecStart=/usr/bin/monoterminal daemon
   Restart=always
   
   [Install]
   WantedBy=multi-user.target
   ```

4. **launchd Integration**
   ```xml
   <key>Label</key>
   <string>com.monoterminal.daemon</string>
   <key>ProgramArguments</key>
   <array>
       <string>/usr/local/bin/monoterminal</string>
       <string>daemon</string>
   </array>
   <key>RunAtLoad</key>
   <true/>
   ```

5. **Consequences**
   - ✅ Native integration per platform
   - ✅ Auto-restart on crash
   - ✅ Socket activation (Linux/macOS)
   - ❌ Different configuration per platform
   - ❌ Must test on all platforms

---

### 5.4 ADR-017: Distribution Packaging Strategy

**File:** `docs/architecture/adr/017-distribution-packaging.md`  
**Length:** ~600 words  
**Sections:**

1. **Context**
   - Need native packages for all platforms
   - Users expect platform-native installation
   - Must support auto-updates

2. **Decision**
   - **Linux:**
     - .deb for Ubuntu/Debian (via `cargo-deb`)
     - .rpm for Fedora/RHEL (via `cargo-generate-rpm`)
     - APT/YUM repositories for updates
   - **macOS:**
     - Homebrew tap (primary)
     - .pkg installer (secondary)
   - **Windows:**
     - .exe installer (existing, via NSIS)

3. **Package Contents**
   - Binary: `/usr/bin/monoterminal` (Linux/macOS), `C:\Program Files\Monoterminal\` (Windows)
   - Service file: `/etc/systemd/system/monoterminal.service`, `~/Library/LaunchAgents/`
   - Configuration: `/etc/monoterminal/`, `~/.monoterminal/`

4. **Signing**
   - Linux: GPG-signed packages
   - macOS: Apple Developer ID signing
   - Windows: Authenticode signing (existing)

5. **Consequences**
   - ✅ Native installation experience
   - ✅ Auto-updates via package managers
   - ✅ Dependency management
   - ❌ Signing complexity
   - ❌ Must maintain multiple build pipelines

---

## 6. Timeline and Ownership

### 6.1 Week 11: User + Developer Documentation

| Day | Activity | Owner | Deliverable |
|-----|----------|-------|-------------|
| Mon | Installation guides (Ubuntu, Debian, Fedora, macOS) | technical-writer | 4 guides complete |
| Tue | Service management guides (systemd, launchd) | technical-writer | 2 guides complete |
| Wed | Troubleshooting guides (Linux, macOS) | technical-writer | 2 guides complete |
| Thu | Cross-Platform Development Guide | technical-writer + qa-lead | 1 guide complete |
| Fri | PTY Abstraction Guide + Testing Guide | technical-writer + qa-lead | 2 guides complete |

**Total Week 11:** 10 user + developer docs complete

### 6.2 Week 12: Architecture Documentation + Review

| Day | Activity | Owner | Deliverable |
|-----|----------|-------|-------------|
| Mon | Phase 3 Architecture Overview | principal-architect | 1 overview complete |
| Tue | ADR-015 (PTY Abstraction) | principal-architect | 1 ADR complete |
| Wed | ADR-016 (Service Management) | principal-architect | 1 ADR complete |
| Thu | ADR-017 (Distribution Packaging) | principal-architect | 1 ADR complete |
| Fri | Documentation review + publish | eng-director + qa-lead | Docs published |

**Total Week 12:** 5 architecture docs complete

---

## 7. Documentation Quality Standards

### 7.1 Writing Standards

1. **Clarity**
   - Use active voice ("Install the package" not "The package should be installed")
   - Keep sentences short (<25 words)
   - Define acronyms on first use

2. **Consistency**
   - Use same terminology across all docs
   - Follow existing style guide
   - Use code blocks for all commands

3. **Completeness**
   - Every guide includes Prerequisites, Steps, Verification, Troubleshooting
   - Every code example includes expected output
   - Every troubleshooting entry includes Symptom → Diagnosis → Fix

4. **Accuracy**
   - Test all commands on target platforms
   - Verify all file paths exist
   - Validate all code examples compile/run

### 7.2 Review Checklist

- [ ] All commands tested on target platform
- [ ] All file paths verified to exist
- [ ] All code examples compile and run
- [ ] All screenshots captured at 1920x1080
- [ ] All links verified (no 404s)
- [ ] All acronyms defined
- [ ] Consistent terminology used
- [ ] Grammar/spelling checked
- [ ] Peer reviewed by 1+ engineer

---

## 8. Success Metrics

### 8.1 Documentation Completeness

| Category | Target | Actual | Status |
|----------|--------|--------|--------|
| User Docs | 10 documents | TBD | ⏳ Pending |
| Developer Docs | 8 documents | TBD | ⏳ Pending |
| Architecture Docs | 5 documents | TBD | ⏳ Pending |
| **Total** | **23 documents** | **TBD** | **⏳ Pending** |

### 8.2 User Success Metrics

- ✅ Users can install on any platform without support tickets
- ✅ <5 installation issues per 100 users
- ✅ <10 service management issues per 100 users
- ✅ >90% user satisfaction with documentation (survey)

### 8.3 Developer Success Metrics

- ✅ New contributors can build on all platforms within 30 minutes
- ✅ >80% of PRs have correct cross-platform code patterns
- ✅ <3 platform-specific bugs per release

---

## 9. Phase 3 Gate Documentation Criteria

**Phase 3 documentation is COMPLETE when:**

1. ✅ All 23 documents created and reviewed
2. ✅ All commands tested on all platforms
3. ✅ All code examples validated
4. ✅ All screenshots captured
5. ✅ All documentation published to docs.monoterminal.dev
6. ✅ User installation guides available for all platforms
7. ✅ Developer onboarding guide complete
8. ✅ Architecture ADRs reviewed by principal-architect

**Escalation:** If documentation incomplete, escalate to eng-director for gate decision

---

## 10. Summary

**Documentation Plan Overview:**

- **23 total documents** across 3 categories
- **Week 11:** User + Developer docs (10 guides)
- **Week 12:** Architecture docs (5 ADRs + overview)
- **Timeline:** 2 weeks parallel with integration testing
- **Ownership:** technical-writer (user/dev docs), principal-architect (architecture docs), qa-lead (testing guides)

**Key Deliverables:**
1. Installation guides for all platforms (Ubuntu, Debian, Fedora, macOS)
2. Service management guides (systemd, launchd)
3. Cross-platform development guide
4. PTY abstraction guide
5. Architecture overview + 3 ADRs

**Phase 3 Gate:** Documentation complete when all 23 docs published and validated

---

**Document Status:** DRAFT  
**Next Review:** After task-60 completion  
**Approval Required:** eng-director + principal-architect

---

**End of Documentation Plan**