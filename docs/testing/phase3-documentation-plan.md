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
   cargo test --