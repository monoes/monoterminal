# Phase 3: Service Management Architecture Design

**Date:** 2026-08-19  
**Author:** rust-backend-lead  
**Status:** Architecture Planning (Week 3, implementation in Weeks 4-5)  
**Task:** task-54

## 1. Overview

### 1.1 Objectives

Design a unified service management architecture for MONOTERMINAL master daemon across Linux (systemd) and macOS (launchd) platforms, with a consistent CLI interface for installation and lifecycle management.

**Goals:**
1. System service installation on Linux (systemd) and macOS (launchd)
2. Automatic startup on boot
3. Auto-restart on failure
4. Unified CLI commands across platforms
5. Integration with Phase 3 cross-platform file paths (task-53)

**Non-goals (Week 3):**
- Socket activation (deferred per Phase 3 architecture)
- User-mode services (system-wide only for Phase 3)
- Windows Service integration (already implemented in Phase 1)

### 1.2 Architecture Principles

1. **Platform-native** - Use systemd on Linux, launchd on macOS (no abstraction layers)
2. **Unified CLI** - Same commands work across all platforms with platform detection
3. **Idempotent** - Safe to run install/uninstall multiple times
4. **Self-contained** - No external dependencies beyond OS service managers
5. **Standards-compliant** - Follow systemd and launchd best practices

---

## 2. systemd Architecture (Linux)

### 2.1 Service Type

**Type:** `notify` (with sd-notify support)

**Rationale:**
- `notify` provides lifecycle feedback to systemd
- Daemon notifies systemd when fully initialized
- Enables accurate service status reporting
- Required for socket activation (future Phase 4)

**Alternative considered:** `simple` (rejected - less precise lifecycle management)

### 2.2 Unit File Design

**Location:** `/etc/systemd/system/monoterminal.service`

```ini
[Unit]
Description=MONOTERMINAL Master Daemon
Documentation=https://github.com/monoterminal/monoterminal
After=network.target

# Phase 3: Basic system service
# Phase 4: Add socket activation (After=monoterminal.socket)

[Service]
Type=notify
ExecStart=/usr/local/bin/monoterminal-master --systemd
Restart=on-failure
RestartSec=5s

# User and group (system service)
User=monoterminal
Group=monoterminal

# Working directory and state
WorkingDirectory=/var/lib/monoterminal
StateDirectory=monoterminal
LogsDirectory=monoterminal
ConfigurationDirectory=monoterminal

# File paths (from task-53)
# StateDirectory creates /var/lib/monoterminal (data_dir)
# LogsDirectory creates /var/log/monoterminal (log_dir)

# Security hardening (systemd sandboxing)
# Phase 3: Basic restrictions
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/monoterminal /var/log/monoterminal

# Phase 4: Additional hardening
# ProtectKernelTunables=true
# ProtectControlGroups=true
# RestrictRealtime=true

# Resource limits
LimitNOFILE=65536
LimitNPROC=512

# Notification timeout (for Type=notify)
NotifyAccess=main
TimeoutStartSec=60s
TimeoutStopSec=30s

[Install]
WantedBy=multi-user.target
```

### 2.3 systemd Integration

**sd-notify Protocol:**

The daemon must notify systemd of readiness:

```rust
// Pseudo-code for systemd notification
use std::os::unix::net::UnixDatagram;

fn notify_systemd_ready() -> Result<()> {
    if let Ok(socket_path) = env::var("NOTIFY_SOCKET") {
        let socket = UnixDatagram::unbound()?;
        socket.send_to(b"READY=1", &socket_path)?;
        tracing::info!("Notified systemd: READY");
    }
    Ok(())
}

// In main():
// 1. Initialize services
// 2. Bind server socket
// 3. notify_systemd_ready()
// 4. Enter event loop
```

**Dependencies:**
- `libsystemd` (optional, fallback to socket-based notification)
- OR direct UnixDatagram to `$NOTIFY_SOCKET` (no external deps)

**Recommendation:** Direct socket approach (no external library dependency)

### 2.4 User and Group Management

**System user:** `monoterminal`  
**System group:** `monoterminal`

**Creation (during install):**
```bash
sudo useradd --system --no-create-home \
    --shell /usr/sbin/nologin \
    --comment "MONOTERMINAL master daemon" \
    monoterminal
```

**Rationale:**
- System user (not human user)
- No home directory (uses /var/lib/monoterminal)
- No login shell (security)
- Dedicated group for file permissions

### 2.5 File Paths Integration (task-53)

**systemd directives map to task-53 paths:**

| systemd Directive | task-53 Function | Path |
|-------------------|------------------|------|
| `StateDirectory=monoterminal` | `data_dir()` | `/var/lib/monoterminal` |
| `LogsDirectory=monoterminal` | `log_dir()` | `/var/log/monoterminal` |
| `ConfigurationDirectory=monoterminal` | N/A | `/etc/monoterminal` |

**Note:** systemd automatically creates these directories with correct ownership (`User=monoterminal`)

### 2.6 Lifecycle Commands

**Installation:**
```bash
# 1. Copy binary
sudo cp monoterminal-master /usr/local/bin/
sudo chmod 755 /usr/local/bin/monoterminal-master

# 2. Create system user
sudo useradd --system --no-create-home monoterminal

# 3. Install unit file
sudo cp monoterminal.service /etc/systemd/system/
sudo chmod 644 /etc/systemd/system/monoterminal.service

# 4. Reload systemd
sudo systemctl daemon-reload

# 5. Enable (auto-start on boot)
sudo systemctl enable monoterminal

# 6. Start service
sudo systemctl start monoterminal
```

**Status checking:**
```bash
sudo systemctl status monoterminal
sudo journalctl -u monoterminal -f
```

**Uninstallation:**
```bash
# 1. Stop service
sudo systemctl stop monoterminal

# 2. Disable auto-start
sudo systemctl disable monoterminal

# 3. Remove unit file
sudo rm /etc/systemd/system/monoterminal.service

# 4. Reload systemd
sudo systemctl daemon-reload

# 5. Remove binary
sudo rm /usr/local/bin/monoterminal-master

# 6. (Optional) Remove user
# sudo userdel monoterminal
# Note: Keep user if data exists in /var/lib/monoterminal
```

---

## 3. launchd Architecture (macOS)

### 3.1 Service Type

**Type:** Launch Daemon (system-wide service)

**Rationale:**
- Runs as root initially, then drops privileges
- System-wide availability (not per-user)
- Starts at boot (LaunchDaemons run before user login)

**Alternative considered:** Launch Agent (rejected - per-user scope, not system-wide)

### 3.2 Property List Design

**Location:** `/Library/LaunchDaemons/com.monoterminal.master.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Service identification -->
    <key>Label</key>
    <string>com.monoterminal.master</string>

    <!-- Program and arguments -->
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/monoterminal-master</string>
        <string>--launchd</string>
    </array>

    <!-- Working directory -->
    <key>WorkingDirectory</key>
    <string>/Library/Application Support/MONOTERMINAL</string>

    <!-- Auto-start on boot -->
    <key>RunAtLoad</key>
    <true/>

    <!-- Keep alive (auto-restart on crash) -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>

    <!-- Restart throttle -->
    <key>ThrottleInterval</key>
    <integer>5</integer>

    <!-- Standard output/error -->
    <key>StandardOutPath</key>
    <string>/Library/Logs/MONOTERMINAL/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/Library/Logs/MONOTERMINAL/stderr.log</string>

    <!-- User (drops privileges from root) -->
    <key>UserName</key>
    <string>_monoterminal</string>
    <key>GroupName</key>
    <string>_monoterminal</string>

    <!-- Resource limits -->
    <key>SoftResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>65536</integer>
        <key>NumberOfProcesses</key>
        <integer>512</integer>
    </dict>

    <!-- Process type (Adaptive for daemons) -->
    <key>ProcessType</key>
    <string>Adaptive</string>

    <!-- Phase 4: Socket listeners (deferred) -->
    <!--
    <key>Sockets</key>
    <dict>
        <key>Listeners</key>
        <dict>
            <key>SockServiceName</key>
            <string>5000</string>
            <key>SockType</key>
            <string>stream</string>
            <key>SockFamily</key>
            <string>IPv4</string>
        </dict>
    </dict>
    -->
</dict>
</plist>
```

### 3.3 User and Group Management (macOS)

**System user:** `_monoterminal` (underscore prefix for system accounts)  
**System group:** `_monoterminal`

**Creation (during install):**
```bash
# macOS uses dscl for user management
sudo dscl . -create /Users/_monoterminal
sudo dscl . -create /Users/_monoterminal UserShell /usr/bin/false
sudo dscl . -create /Users/_monoterminal RealName "MONOTERMINAL Daemon"
sudo dscl . -create /Users/_monoterminal UniqueID 400
sudo dscl . -create /Users/_monoterminal PrimaryGroupID 400
sudo dscl . -create /Users/_monoterminal NFSHomeDirectory /var/empty

# Create group
sudo dscl . -create /Groups/_monoterminal
sudo dscl . -create /Groups/_monoterminal PrimaryGroupID 400
```

**Note:** UID/GID 400 is in the system range (< 500) on macOS

### 3.4 File Paths Integration (task-53)

**launchd directives map to task-53 paths:**

| launchd Key | task-53 Function | Path |
|-------------|------------------|------|
| `WorkingDirectory` | `data_dir()` | `/Library/Application Support/MONOTERMINAL` |
| `StandardOutPath` | `log_dir()` | `/Library/Logs/MONOTERMINAL/stdout.log` |
| `StandardErrorPath` | `log_dir()` | `/Library/Logs/MONOTERMINAL/stderr.log` |

**Directory creation:**
```bash
# Create data directory
sudo mkdir -p "/Library/Application Support/MONOTERMINAL"
sudo chown _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL"
sudo chmod 755 "/Library/Application Support/MONOTERMINAL"

# Create log directory
sudo mkdir -p "/Library/Logs/MONOTERMINAL"
sudo chown _monoterminal:_monoterminal "/Library/Logs/MONOTERMINAL"
sudo chmod 755 "/Library/Logs/MONOTERMINAL"
```

### 3.5 Lifecycle Commands

**Installation:**
```bash
# 1. Copy binary
sudo cp monoterminal-master /usr/local/bin/
sudo chmod 755 /usr/local/bin/monoterminal-master

# 2. Create system user
sudo dscl . -create /Users/_monoterminal
# (full user creation from 3.3)

# 3. Create directories
sudo mkdir -p "/Library/Application Support/MONOTERMINAL"
sudo mkdir -p "/Library/Logs/MONOTERMINAL"
sudo chown -R _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL"
sudo chown -R _monoterminal:_monoterminal "/Library/Logs/MONOTERMINAL"

# 4. Install plist
sudo cp com.monoterminal.master.plist /Library/LaunchDaemons/
sudo chmod 644 /Library/LaunchDaemons/com.monoterminal.master.plist
sudo chown root:wheel /Library/LaunchDaemons/com.monoterminal.master.plist

# 5. Load service (starts immediately)
sudo launchctl load /Library/LaunchDaemons/com.monoterminal.master.plist

# 6. Enable auto-start (RunAtLoad=true does this)
```

**Status checking:**
```bash
sudo launchctl list | grep monoterminal
sudo tail -f "/Library/Logs/MONOTERMINAL/stdout.log"
sudo tail -f "/Library/Logs/MONOTERMINAL/stderr.log"
```

**Uninstallation:**
```bash
# 1. Unload service
sudo launchctl unload /Library/LaunchDaemons/com.monoterminal.master.plist

# 2. Remove plist
sudo rm /Library/LaunchDaemons/com.monoterminal.master.plist

# 3. Remove binary
sudo rm /usr/local/bin/monoterminal-master

# 4. (Optional) Remove user
# sudo dscl . -delete /Users/_monoterminal
# sudo dscl . -delete /Groups/_monoterminal
# Note: Keep if data exists
```

---

## 4. Unified CLI Design

### 4.1 Command Structure

**Install command:**
```bash
monoterminal-master install-service [OPTIONS]
```

**Uninstall command:**
```bash
monoterminal-master uninstall-service [OPTIONS]
```

**Options:**
```
--system          Install as system service (default, only option in Phase 3)
--user            Install as user service (Phase 4, not implemented)
--dry-run         Show what would be done without executing
--force           Force reinstall if already installed
--no-start        Install but don't start service
```

### 4.2 Platform Detection

**Detection logic:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceManager {
    Systemd,    // Linux
    Launchd,    // macOS
    WindowsSCM, // Windows (already implemented Phase 1)
}

pub fn detect_service_manager() -> Result<ServiceManager> {
    #[cfg(target_os = "linux")]
    {
        // Check if systemd is running (PID 1)
        if std::path::Path::new("/run/systemd/system").exists() {
            Ok(ServiceManager::Systemd)
        } else {
            bail!("systemd not detected (required for Linux service management)")
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS always uses launchd
        Ok(ServiceManager::Launchd)
    }

    #[cfg(windows)]
    {
        // Windows Service Control Manager (already implemented)
        Ok(ServiceManager::WindowsSCM)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        bail!("Unsupported platform for service management")
    }
}
```

### 4.3 Privilege Escalation

**Detection:**
```rust
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    {
        // Check if running as Administrator (already implemented Phase 1)
        true // Placeholder
    }
}

pub fn require_root() -> Result<()> {
    if !is_root() {
        bail!("This command requires root privileges. Run with sudo.");
    }
    Ok(())
}
```

**User guidance:**
```rust
// In install-service command
if !is_root() {
    eprintln!("Error: Installation requires root privileges");
    eprintln!("");
    eprintln!("Please run with sudo:");
    
    #[cfg(target_os = "linux")]
    eprintln!("  sudo monoterminal-master install-service");
    
    #[cfg(target_os = "macos")]
    eprintln!("  sudo monoterminal-master install-service");
    
    std::process::exit(1);
}
```

### 4.4 Installation Flow

**High-level algorithm:**

```
1. Detect platform (Linux/macOS/Windows)
2. Check for root privileges
3. Detect service manager (systemd/launchd/SCM)
4. Check if already installed
   - If yes and --force: uninstall first
   - If yes and not --force: error
5. Create system user/group
6. Create directories (data, logs)
7. Copy binary to system location
8. Install service file (unit/plist/service)
9. Reload service manager
10. Enable auto-start
11. Start service (unless --no-start)
12. Verify service started
13. Print status and logs location
```

**Error handling:**
- Rollback on failure (remove files, user, service)
- Detailed error messages with remediation steps
- Dry-run mode shows all steps without executing

### 4.5 Uninstallation Flow

**High-level algorithm:**

```
1. Detect platform
2. Check for root privileges
3. Detect service manager
4. Check if installed
   - If not: warn and exit 0 (idempotent)
5. Stop service
6. Disable auto-start
7. Unload/remove service file
8. Reload service manager
9. Remove binary
10. Prompt: Remove data directories? (default: no)
    - If yes: Remove /var/lib/monoterminal, /var/log/monoterminal
    - If no: Keep data for future reinstall
11. Prompt: Remove system user? (default: no)
    - If yes and data removed: Delete user/group
    - If no or data kept: Keep user (owns data)
12. Print uninstall summary
```

**Safety:**
- Always prompt before deleting data
- Keep user if data exists (prevents orphaned files)
- Idempotent (safe to run multiple times)

### 4.6 Status Command

**Command:**
```bash
monoterminal-master service-status
```

**Output:**
```
MONOTERMINAL Service Status
===========================

Platform: Linux (systemd)
Service: monoterminal.service

Status: ● active (running) since Mon 2026-08-19 14:30:00 UTC; 2h 15min ago
  Main PID: 1234 (monoterminal-master)
     Tasks: 12 (limit: 4915)
    Memory: 45.2M
       CPU: 1min 23.456s

Logs:
  /var/log/monoterminal/monoterminal.log

Commands:
  View logs:    sudo journalctl -u monoterminal -f
  Restart:      sudo systemctl restart monoterminal
  Stop:         sudo systemctl stop monoterminal
  Uninstall:    sudo monoterminal-master uninstall-service
```

**Implementation:**
```rust
pub fn service_status() -> Result<()> {
    require_root()?;
    
    let manager = detect_service_manager()?;
    
    match manager {
        ServiceManager::Systemd => {
            // Run: systemctl status monoterminal
            Command::new("systemctl")
                .args(&["status", "monoterminal"])
                .status()?;
        }
        ServiceManager::Launchd => {
            // Run: launchctl list | grep monoterminal
            // Parse output and format nicely
        }
        ServiceManager::WindowsSCM => {
            // Already implemented Phase 1
        }
    }
    
    Ok(())
}
```

---

## 5. Implementation Roadmap

### 5.1 Week 3 (Current): Architecture & Planning ✅

**Deliverables:**
- [x] Architecture design document (this document)
- [ ] Service file templates (systemd, launchd)
- [ ] CLI command specification

**Status:** In progress (Day 2 of Week 3)

### 5.2 Week 4: systemd Implementation

**Tasks:**
1. Implement `install-service` command (Linux)
   - Platform detection
   - Privilege checking
   - User/group creation
   - Directory setup
   - Unit file installation
   - systemd integration

2. Implement sd-notify support
   - Direct socket notification (no libsystemd dependency)
   - Integration in main event loop

3. Testing
   - Manual testing on Ubuntu 22.04
   - Verification of auto-start
   - Crash recovery testing

**Estimated:** 5-7 days

### 5.3 Week 5: launchd Implementation

**Tasks:**
1. Implement `install-service` command (macOS)
   - Platform detection (macOS-specific)
   - Privilege checking
   - User/group creation (dscl)
   - Directory setup
   - plist installation
   - launchd integration

2. Implement `uninstall-service` command (both platforms)
   - Safe data removal prompts
   - User cleanup logic

3. Implement `service-status` command (both platforms)

4. Testing
   - Manual testing on macOS 13+
   - Verification of auto-start
   - Crash recovery testing

**Estimated:** 5-7 days

### 5.4 Week 6: Integration & Documentation

**Tasks:**
1. Integration with existing codebase
   - Update main.rs to detect --systemd/--launchd flags
   - Integration with task-53 file paths

2. Documentation
   - Installation guide (Linux/macOS)
   - Service management guide
   - Troubleshooting

3. CI/CD
   - Package creation (deb, rpm for Linux)
   - DMG creation (macOS)

**Estimated:** 3-5 days

---

## 6. Security Considerations

### 6.1 Privilege Separation

**systemd (Linux):**
- Daemon runs as dedicated `monoterminal` user (not root)
- systemd drops privileges automatically (`User=monoterminal`)
- No sudo elevation after start

**launchd (macOS):**
- Launch Daemon runs as `_monoterminal` user
- launchd drops privileges from root (`UserName=_monoterminal`)
- No sudo elevation after start

**Rationale:** Minimize attack surface by running with least privilege

### 6.2 File Permissions

**Data directory:**
- Owner: `monoterminal:monoterminal` (Linux) or `_monoterminal:_monoterminal` (macOS)
- Permissions: `0755` (rwxr-xr-x)
- Only service user can write

**Log directory:**
- Owner: `monoterminal:monoterminal` (Linux) or `_monoterminal:_monoterminal` (macOS)
- Permissions: `0755`
- Only service user can write

**Database file:**
- Owner: `monoterminal:monoterminal` (Linux) or `_monoterminal:_monoterminal` (macOS)
- Permissions: `0644` (rw-r--r--)
- Only service user can write

### 6.3 systemd Sandboxing

**Phase 3 (Basic):**
- `NoNewPrivileges=true` - Prevent privilege escalation
- `PrivateTmp=true` - Private /tmp directory
- `ProtectSystem=strict` - Read-only /usr, /boot, /efi
- `ProtectHome=true` - No access to /home

**Phase 4 (Enhanced):**
- `ProtectKernelTunables=true`
- `ProtectControlGroups=true`
- `RestrictRealtime=true`
- `RestrictNamespaces=true`

**Rationale:** Defense-in-depth via systemd security features

### 6.4 Resource Limits

**File descriptors:** 65536 (for 1000-session target, SRS §5.1.1)  
**Processes:** 512 (reasonable limit for PTY spawning)

**Prevents:**
- File descriptor exhaustion
- Fork bomb attacks
- Runaway resource consumption

---

## 7. Testing Strategy

### 7.1 Unit Tests

**Rust unit tests:**
- Platform detection (`detect_service_manager()`)
- Privilege checking (`is_root()`)
- Service file template generation
- Path integration (task-53 paths in service files)

**Estimated:** ~150 LOC tests

### 7.2 Integration Tests

**systemd (Linux):**
1. Install service on fresh Ubuntu 22.04 VM
2. Verify service starts
3. Verify auto-start after reboot
4. Verify crash recovery (kill -9, check auto-restart)
5. Verify logs in /var/log/monoterminal
6. Verify uninstall cleanup

**launchd (macOS):**
1. Install service on macOS 13+ VM
2. Verify service starts
3. Verify auto-start after reboot
4. Verify crash recovery (kill -9, check auto-restart)
5. Verify logs in /Library/Logs/MONOTERMINAL
6. Verify uninstall cleanup

### 7.3 Manual Testing Checklist

**Installation:**
- [ ] Install without sudo fails with clear error
- [ ] Install with sudo succeeds
- [ ] Service starts automatically
- [ ] Logs appear in correct location
- [ ] Database created in correct location
- [ ] Reinstall with --force works
- [ ] Reinstall without --force fails

**Service lifecycle:**
- [ ] Service auto-starts on boot
- [ ] Service restarts on crash (kill -9)
- [ ] Service stops cleanly (systemctl stop / launchctl unload)
- [ ] Logs are continuous across restarts

**Uninstallation:**
- [ ] Uninstall stops service
- [ ] Uninstall removes binary
- [ ] Uninstall prompts for data deletion
- [ ] Uninstall preserves data if declined
- [ ] Uninstall is idempotent (run twice)

---

## 8. Future Work (Phase 4+)

### 8.1 Socket Activation (systemd)

**Deferred to Phase 4** per Phase 3 architecture.

**Design notes:**
- `monoterminal.socket` unit file
- Binds to port 5000
- systemd passes socket FD to daemon
- Daemon accepts connections from passed FD

**Benefits:**
- On-demand activation (no constant daemon)
- Zero-downtime restarts
- systemd handles bind() before privilege drop

### 8.2 User-Mode Services

**Deferred to Phase 4+**

**systemd user services:**
- Unit file in `~/.config/systemd/user/`
- `systemctl --user enable monoterminal`
- Runs as regular user (no root)

**launchd user agents:**
- plist in `~/Library/LaunchAgents/`
- Per-user session scope

**Use case:** Personal terminal server (not system-wide)

### 8.3 Service Monitoring

**Deferred to Phase 4+**

- Health check endpoint
- systemd watchdog support
- Prometheus metrics export
- Integration with monitoring systems

---

## 9. Dependencies

### 9.1 Internal Dependencies

**Phase 3 task-53 (file paths):** ✅ COMPLETE
- `data_dir()`, `user_data_dir()`, `log_dir()` used in service files
- Service files reference paths from task-53

**Phase 3 task-52 (Unix PTY):** ✅ COMPLETE
- PTY backend used by daemon
- Service environment suitable for PTY operation

### 9.2 External Dependencies

**Linux (systemd):**
- systemd (version 230+, Ubuntu 22.04 has 249)
- No additional Rust crates needed (direct socket notification)

**macOS (launchd):**
- macOS 10.13+ (High Sierra or later)
- No additional Rust crates needed (use launchctl commands)

**Rust crates (potential):**
- `nix` - Unix user/group management (already in dependency tree)
- None new required (use std::process::Command for shell commands)

---

## 10. Success Criteria

**Week 3 (Architecture):**
- [x] Architecture document complete
- [ ] Service file templates created
- [ ] CLI command specification complete
- [ ] Design review approved by eng-director

**Weeks 4-5 (Implementation):**
- [ ] `install-service` command works on Linux (systemd)
- [ ] `install-service` command works on macOS (launchd)
- [ ] `uninstall-service` command works on both platforms
- [ ] `service-status` command works on both platforms
- [ ] Service auto-starts on boot (both platforms)
- [ ] Service auto-restarts on crash (both platforms)
- [ ] Manual testing checklist 100% pass
- [ ] Integration tests pass on CI (Ubuntu + macOS)

**Documentation:**
- [ ] Installation guide (Linux/macOS)
- [ ] Service management guide
- [ ] Troubleshooting guide
- [ ] Security considerations documented

---

## 11. Open Questions

1. **systemd notification:** Direct socket vs libsystemd?
   - **Recommendation:** Direct socket (no external dependency)

2. **User cleanup:** Remove system user on uninstall?
   - **Recommendation:** Prompt user, default to keep if data exists

3. **Package format:** deb/rpm for Linux, DMG for macOS?
   - **Recommendation:** Week 6 (deferred to Phase 3 Week 9-10 per roadmap)

4. **Log rotation:** systemd journald vs manual log rotation?
   - **Recommendation:** Use journald (Linux), manual rotation (macOS)

5. **Service restart delay:** 5 seconds vs exponential backoff?
   - **Recommendation:** 5 seconds fixed (simple, predictable)

---

## 12. Appendix

### 12.1 systemd Unit File Template

See Section 2.2 above.

### 12.2 launchd plist Template

See Section 3.2 above.

### 12.3 References

**systemd:**
- [systemd.service(5)](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
- [systemd.exec(5)](https://www.freedesktop.org/software/systemd/man/systemd.exec.html)
- [sd-notify(3)](https://www.freedesktop.org/software/systemd/man/sd_notify.html)

**launchd:**
- [launchd.plist(5)](https://www.manpagez.com/man/5/launchd.plist/)
- [launchctl(1)](https://www.manpagez.com/man/1/launchctl/)
- [TN2083: Daemons and Agents](https://developer.apple.com/library/archive/technotes/tn2083/)

**Security:**
- [systemd Security Hardening](https://www.freedesktop.org/software/systemd/man/systemd.exec.html#Sandboxing)
- [macOS Daemon Security](https://developer.apple.com/library/archive/documentation/Security/Conceptual/SecureCodingGuide/)

---

**Status:** Architecture design COMPLETE  
**Next:** Service file templates + CLI specification (remainder of Week 3)  
**Implementation:** Weeks 4-5 per roadmap
