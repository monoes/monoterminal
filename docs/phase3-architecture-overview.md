# Phase 3 Architecture Overview: Linux + macOS Expansion

**Version:** 1.0  
**Date:** 2026-08-19  
**Author:** principal-architect  
**Status:** Draft — Scoping for Phase 3 Implementation

---

## Executive Summary

Phase 3 extends MONOTERMINAL from Windows-only to full cross-platform support (Linux + macOS), completing the master daemon's platform coverage. The web client (PWA) already works cross-platform, so **zero client work required** — all effort focuses on master daemon platform abstractions.

**Key Scope:**
- **PTY abstraction:** Windows ConPTY → Unix PTY (Linux/macOS)
- **Service management:** Windows Service → systemd (Linux), launchd (macOS)
- **Build matrix:** Windows + Ubuntu + macOS CI testing
- **Distribution:** apt/rpm (Linux), Homebrew (macOS)

**Effort Estimate:** 3 months, 1.5 engineers (per SRS §7.3)

**Success Criteria:**
- Feature parity with Windows master (same protocol, same web client)
- Ubuntu 22.04+, Debian 11+, Fedora 38+, macOS 12+ support
- 80% test coverage across all platforms

---

## 1. SRS §7.3 Requirements Review

### 1.1 Acceptance Criteria (from SRS §7.3)

**Platform Support:**

| Platform | Versions | PTY Backend | Rendering | Service Management |
|----------|----------|-------------|-----------|-------------------|
| **Linux** | Ubuntu 22.04+, Debian 11+, Fedora 38+ | `posix_openpt` + `grantpt` | wgpu (Vulkan) | systemd Type=notify + socket activation |
| **macOS** | macOS 12+ (Monterey) | `openpty()` (BSD) | wgpu (Metal) | launchd socket activation |
| **Windows** | Windows 10 1809+ | ConPTY (already implemented) | wgpu (DirectX 12) | Windows Service (already implemented) |

**Feature Parity Requirements:**
- ✅ Same protocol version (Protobuf messages unchanged)
- ✅ Same web client works unmodified against any platform
- ✅ Cross-platform CI testing (all 3 platforms tested on every PR)
- ✅ 80% test coverage maintained

**Distribution Channels:**
- Linux: apt (Ubuntu/Debian), rpm (Fedora/RHEL), standalone binary
- macOS: Homebrew formula, standalone binary
- Windows: Already has standalone binary + Windows Service installer

---

## 2. Architecture Gaps Analysis

### 2.1 PTY Layer (CRITICAL GAP)

**Current State (Phase 2):**
- ✅ Windows ConPTY implementation complete (`crates/master/src/pty/conpty.rs`, 1,372 LOC)
- ✅ `PtyBackend` trait exists (abstraction layer already designed)
- ❌ Unix PTY implementation missing (Linux/macOS)

**Gap Analysis:**

```rust
// Existing abstraction (from conpty.rs line 14)
pub trait PtyBackend: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> PtyResult<usize>;
    async fn write(&mut self, buf: &[u8]) -> PtyResult<usize>;
    async fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()>;
    async fn terminate(&mut self) -> PtyResult<()>;
    fn shell_pid(&self) -> u32;
}
```

**Unix PTY Requirements (SRS §2.1.2):**

**Linux Implementation:**
```c
// POSIX PTY workflow (from SRS §2.1.2.1)
int primary_fd = posix_openpt(O_RDWR | O_NOCTTY);
grantpt(primary_fd);   // Set ownership
unlockpt(primary_fd);  // Allow replica access
char *replica = ptsname(primary_fd);

pid_t pid = fork();
if (pid == 0) {
    setsid();          // New session
    dup2(replica_fd, 0);  // stdin
    dup2(replica_fd, 1);  // stdout
    dup2(replica_fd, 2);  // stderr
    exec("/bin/bash");
}
```

**macOS Implementation:**
```c
// BSD openpty() combines posix_openpt + grantpt + unlockpt (from SRS §2.1.2.2)
int master, replica;
pid_t pid;
openpty(&master, &replica, NULL, NULL, NULL);

pid = fork();
if (pid == 0) {
    login_tty(replica);  // Sets controlling terminal + closes master
    exec("/bin/zsh");
}
```

**Rust Implementation Strategy:**

**Option A: Use `portable-pty` crate (RECOMMENDED)**
- ✅ Mature crate (~40k downloads/month)
- ✅ Cross-platform (Linux, macOS, Windows ConPTY)
- ✅ Already implements `posix_openpt` + `openpty()` workflows
- ❌ May need custom fork to match our `PtyBackend` trait

**Option B: Implement from scratch**
- ✅ Full control, matches our existing trait design
- ❌ ~1,500 LOC of unsafe FFI code (error-prone)
- ❌ Platform-specific edge cases (signal handling, file descriptor leaks)

**Recommendation:** Option A (portable-pty) with thin adapter to our `PtyBackend` trait.

**Estimated Effort:** 2 weeks (Week 1-2 of Phase 3)

---

### 2.2 Service Management (HIGH-PRIORITY GAP)

**Current State (Phase 2):**
- ✅ Windows Service implementation (`crates/master/src/service.rs`)
- ❌ systemd integration missing (Linux)
- ❌ launchd integration missing (macOS)

**Linux systemd Requirements (SRS §7.3):**

**Service Type:** `Type=notify` (daemon signals readiness)
```ini
# /etc/systemd/system/monoterminal.service
[Unit]
Description=MONOTERMINAL Master Daemon
After=network.target

[Service]
Type=notify
ExecStart=/usr/bin/monoterminal-master serve
Restart=always
RestartSec=5s

# Socket activation (optional, deferred to Phase 3.5)
# Sockets=monoterminal.socket

[Install]
WantedBy=multi-user.target
```

**Rust Integration:**
```rust
use sd_notify::NotifyState;

async fn daemon_main() -> Result<()> {
    // 1. Initialize daemon
    let daemon = MasterDaemon::new().await?;
    
    // 2. Signal systemd readiness
    sd_notify::notify(true, &[NotifyState::Ready])?;
    
    // 3. Run event loop
    daemon.run().await?;
    
    // 4. Graceful shutdown (SIGTERM handler)
    daemon.shutdown().await?;
    
    Ok(())
}
```

**macOS launchd Requirements (SRS §7.3):**

**Service Type:** `KeepAlive=true` (restart on crash)
```xml
<!-- ~/Library/LaunchAgents/com.monoterminal.master.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" ...>
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.monoterminal.master</string>
    <key>Program</key>
    <string>/usr/local/bin/monoterminal-master</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/monoterminal-master</string>
        <string>serve</string>
    </array>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
```

**Socket Activation (Deferred to Phase 3.5):**
- SRS §7.3 mentions "socket activation" for Linux/macOS
- SRS §7.1 notes "Windows has no launchd/systemd-style socket activation"
- **Decision:** Defer to Phase 3.5 or Phase 4 (not blocking feature parity)
- **Phase 3 MVP:** Always-running daemon (same as Windows Phase 1)

**Estimated Effort:** 1 week (Week 3 of Phase 3)

---

### 2.3 Rendering (LOW-PRIORITY GAP)

**Current State (Phase 2):**
- ✅ wgpu rendering engine (`crates/master/src/renderer/`, DirectX 12 on Windows)
- ✅ wgpu is cross-platform by design

**Gap Analysis:**

**wgpu Backend Support:**
| Platform | Backend | Status | Work Required |
|----------|---------|--------|---------------|
| Windows | DirectX 12 | ✅ Implemented | None |
| Linux | Vulkan | ✅ Supported by wgpu | Verify drivers, test on Ubuntu/Fedora |
| macOS | Metal | ✅ Supported by wgpu | Verify M1/M2 Silicon, test on macOS 12+ |

**No Code Changes Required:**
- wgpu automatically selects backend at runtime (DX12/Vulkan/Metal)
- Shaders already written in WGSL (cross-platform)

**Testing Required:**
- Verify Vulkan drivers on Ubuntu 22.04 / Fedora 38
- Verify Metal rendering on macOS 12+ (Intel + Apple Silicon)
- Verify 60 FPS target met across platforms (SRS §5.1)

**Estimated Effort:** 3 days (Week 4, parallel with other work)

---

### 2.4 File Paths & Platform APIs (MEDIUM-PRIORITY GAP)

**Current State (Phase 2):**
- ✅ Windows-specific paths used (`%ProgramData%`, `%LOCALAPPDATA%`)
- ❌ Unix paths needed (`/var/lib`, `/home/<user>/.local`)

**Platform-Specific Path Mapping:**

| Platform | Service Mode | Console Mode | Database Path | Config Path |
|----------|--------------|--------------|---------------|-------------|
| **Windows** | `%ProgramData%\MONOTERMINAL\` | `%LOCALAPPDATA%\monoterminal\` | `data\monoterminal.db` | `config\config.json` |
| **Linux** | `/var/lib/monoterminal/` | `~/.local/share/monoterminal/` | `data/monoterminal.db` | `~/.config/monoterminal/config.json` |
| **macOS** | `/Library/Application Support/MONOTERMINAL/` | `~/Library/Application Support/MONOTERMINAL/` | `data/monoterminal.db` | `~/Library/Preferences/monoterminal.json` |

**Rust Implementation (using `dirs` crate):**

```rust
use dirs::{data_dir, config_dir};

pub fn get_data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        // Windows Service: %ProgramData%\MONOTERMINAL
        PathBuf::from(env::var("ProgramData").unwrap_or("C:\\ProgramData".into()))
            .join("MONOTERMINAL")
    } else if cfg!(target_os = "linux") {
        // Linux systemd: /var/lib/monoterminal
        PathBuf::from("/var/lib/monoterminal")
    } else if cfg!(target_os = "macos") {
        // macOS launchd: /Library/Application Support/MONOTERMINAL
        PathBuf::from("/Library/Application Support/MONOTERMINAL")
    } else {
        panic!("Unsupported platform");
    }
}

pub fn get_user_data_dir() -> PathBuf {
    data_dir()
        .expect("Cannot determine user data directory")
        .join("monoterminal")
}
```

**Estimated Effort:** 2 days (Week 1 of Phase 3)

---

### 2.5 Build Matrix & CI (HIGH-PRIORITY GAP)

**Current State (Phase 2):**
- ✅ Windows CI testing (GitHub Actions)
- ❌ Linux CI missing
- ❌ macOS CI missing

**Required CI Matrix (SRS §6.2):**

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-22.04, macos-13, windows-latest]
        arch: [x86_64, aarch64]  # macOS M1/M2 coverage
        rust: [stable, beta]
        exclude:
          - os: ubuntu-22.04
            arch: aarch64  # Skip ARM64 Linux for Phase 3 MVP
          - os: windows-latest
            arch: aarch64  # Skip ARM64 Windows for Phase 3 MVP
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: ${{ matrix.rust }}
          targets: ${{ matrix.arch }}-unknown-linux-gnu  # (Linux only)
      
      - name: Install platform dependencies
        run: |
          # Ubuntu: Vulkan drivers
          sudo apt-get install -y libvulkan-dev vulkan-tools  # (Linux only)
          
          # macOS: Metal already included in Xcode
          
          # Windows: DirectX 12 already included
      
      - name: Build
        run: cargo build --release --all-targets
      
      - name: Test
        run: cargo test --all-targets
      
      - name: Integration Tests
        run: cargo test --test '*' -- --ignored
```

**Coverage Targets (SRS §7.3):**
- 80% test coverage across all platforms
- Platform-specific tests for PTY, service management, file paths

**Estimated Effort:** 1 week (Week 5 of Phase 3)

---

## 3. Proposed Architecture Decisions

### 3.1 ADR-015: Cross-Platform PTY Abstraction

**Decision:** Implement Unix PTY backend using `portable-pty` crate with thin adapter to existing `PtyBackend` trait.

**Status:** Draft  
**Affected Components:** `crates/master/src/pty/`

**Alternatives Considered:**
- **Option A (CHOSEN):** `portable-pty` crate + adapter
  - ✅ Mature, tested implementation
  - ✅ Faster time-to-market (2 weeks vs 4-5 weeks)
  - ❌ External dependency (acceptable, well-maintained)

- **Option B:** From-scratch implementation
  - ✅ Full control, matches our trait design exactly
  - ❌ ~1,500 LOC unsafe FFI (error-prone, high maintenance)
  - ❌ Platform edge cases (signal handling, zombie processes)

**Implementation Plan:**

**Week 1-2: Unix PTY Backend**
1. Add `portable-pty` dependency to `Cargo.toml`
2. Implement `UnixPtyBackend` struct:
   ```rust
   // crates/master/src/pty/unix.rs
   use portable_pty::{native_pty_system, CommandBuilder, PtySize};
   
   pub struct UnixPtyBackend {
       pty_pair: Box<dyn portable_pty::MasterPty>,
       child: Box<dyn portable_pty::Child>,
       reader: BufReader<Box<dyn std::io::Read + Send>>,
       writer: Box<dyn std::io::Write + Send>,
   }
   
   #[async_trait]
   impl PtyBackend for UnixPtyBackend {
       async fn read(&mut self, buf: &mut [u8]) -> PtyResult<usize> {
           // Wrap blocking read with tokio::task::spawn_blocking
           // (Same pattern as ConPTY for Phase 3 MVP)
       }
       
       async fn write(&mut self, buf: &[u8]) -> PtyResult<usize> {
           // Wrap blocking write with spawn_blocking
       }
       
       async fn resize(&mut self, rows: u16, cols: u16) -> PtyResult<()> {
           self.pty_pair.resize(PtySize { rows, cols, ..Default::default() })?;
           Ok(())
       }
       
       async fn terminate(&mut self) -> PtyResult<()> {
           self.child.kill()?;
           Ok(())
       }
       
       fn shell_pid(&self) -> u32 {
           self.child.process_id().unwrap_or(0) as u32
       }
   }
   ```

3. Add platform detection to `PtyBackend::new()`:
   ```rust
   // crates/master/src/pty/mod.rs
   pub async fn create_pty(config: PtyConfig) -> PtyResult<Box<dyn PtyBackend>> {
       #[cfg(target_os = "windows")]
       {
           Ok(Box::new(ConPtyBackend::new(config).await?))
       }
       
       #[cfg(any(target_os = "linux", target_os = "macos"))]
       {
           Ok(Box::new(UnixPtyBackend::new(config).await?))
       }
   }
   ```

4. Write unit tests for Unix PTY (echo test, resize test, terminate test)
5. Add integration tests (full session lifecycle on Linux/macOS)

**Testing Strategy:**
- Unit tests: Mock PTY I/O, verify read/write/resize/terminate
- Integration tests: Spawn real shell, send commands, verify output
- CI: Run on Ubuntu 22.04 + macOS 13 (GitHub Actions)

**Acceptance:** All existing ConPTY tests pass on Unix PTY implementation

---

### 3.2 ADR-016: Cross-Platform Service Management

**Decision:** Implement platform-specific service management with unified CLI interface.

**Status:** Draft  
**Affected Components:** `crates/master/src/service/`, `crates/master/src/main.rs`

**Service Management Matrix:**

| Platform | Technology | Lifecycle | Install Command |
|----------|------------|-----------|-----------------|
| Windows | Windows Service API | SERVICE_AUTO_START, manual stop/start | `monoterminal-master install-service` |
| Linux | systemd Type=notify | Auto-start on boot, systemctl stop/start | `systemctl enable monoterminal` |
| macOS | launchd KeepAlive | Auto-start on login, launchctl stop/start | `launchctl load ~/Library/LaunchAgents/com.monoterminal.master.plist` |

**CLI Interface (unified across platforms):**

```bash
# Install service/daemon (platform-specific installation)
monoterminal-master install-service

# Uninstall service/daemon
monoterminal-master uninstall-service

# Start daemon (foreground, for testing)
monoterminal-master serve

# Check status
monoterminal-master status
```

**Implementation Plan:**

**Week 3: systemd Integration (Linux)**
1. Add `systemd` crate dependency
2. Implement `sd_notify` readiness signaling:
   ```rust
   // crates/master/src/service/systemd.rs
   use sd_notify::NotifyState;
   
   pub async fn run_systemd_service() -> Result<()> {
       let daemon = MasterDaemon::new().await?;
       
       // Signal readiness to systemd
       sd_notify::notify(true, &[NotifyState::Ready])?;
       tracing::info!("systemd service ready");
       
       daemon.run().await?;
       
       Ok(())
   }
   ```

3. Create systemd unit file template:
   ```ini
   # install-files/linux/monoterminal.service
   [Unit]
   Description=MONOTERMINAL Master Daemon
   After=network.target
   
   [Service]
   Type=notify
   ExecStart=/usr/bin/monoterminal-master serve
   Restart=always
   RestartSec=5s
   User=monoterminal
   Group=monoterminal
   
   [Install]
   WantedBy=multi-user.target
   ```

4. Implement `install-service` command (copies unit file, enables service)

**Week 3: launchd Integration (macOS)**
1. Create launchd plist template:
   ```xml
   <!-- install-files/macos/com.monoterminal.master.plist -->
   <dict>
       <key>Label</key>
       <string>com.monoterminal.master</string>
       <key>ProgramArguments</key>
       <array>
           <string>/usr/local/bin/monoterminal-master</string>
           <string>serve</string>
       </array>
       <key>KeepAlive</key>
       <true/>
       <key>RunAtLoad</key>
       <true/>
       <key>StandardOutPath</key>
       <string>/tmp/monoterminal.stdout.log</string>
       <key>StandardErrorPath</key>
       <string>/tmp/monoterminal.stderr.log</string>
   </dict>
   ```

2. Implement `install-service` command (copies plist, loads via launchctl)

**Deferred to Phase 3.5:**
- Socket activation (systemd + launchd)
- Not required for feature parity (Windows doesn't have it per SRS §7.1)

---

### 3.3 ADR-017: Cross-Platform Distribution Strategy

**Decision:** Multi-channel distribution per platform.

**Status:** Draft  
**Affected Components:** Build scripts, CI/CD pipelines

**Distribution Channels:**

**Linux:**
1. **apt (Ubuntu/Debian):**
   - Create `.deb` package via `cargo-deb`
   - Host on GitHub Releases + custom apt repository
   - Installation: `sudo apt install monoterminal`

2. **rpm (Fedora/RHEL):**
   - Create `.rpm` package via `cargo-generate-rpm`
   - Host on GitHub Releases
   - Installation: `sudo dnf install monoterminal-1.0.0.rpm`

3. **Standalone binary:**
   - Tarball with binary + systemd unit file
   - Installation: `tar -xzf monoterminal-linux.tar.gz && sudo ./install.sh`

**macOS:**
1. **Homebrew:**
   - Create Homebrew formula (Ruby DSL)
   - Submit to homebrew-core or host in custom tap
   - Installation: `brew install monoterminal`

2. **Standalone binary:**
   - DMG with binary + launchd plist
   - Installation: Drag to /Applications, run install script

**Windows (already implemented):**
- Standalone binary with Windows Service installer
- Installation: `monoterminal-master install-service`

**Build Automation:**

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build-linux:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v3
      - run: cargo install cargo-deb cargo-generate-rpm
      - run: cargo deb --target x86_64-unknown-linux-gnu
      - run: cargo generate-rpm --target x86_64-unknown-linux-gnu
      - uses: actions/upload-artifact@v3
        with:
          name: linux-packages
          path: target/debian/*.deb
  
  build-macos:
    runs-on: macos-13
    steps:
      - uses: actions/checkout@v3
      - run: cargo build --release --target x86_64-apple-darwin
      - run: cargo build --release --target aarch64-apple-darwin
      - run: lipo -create target/{x86_64,aarch64}-apple-darwin/release/monoterminal-master -output monoterminal-universal
      - run: ./scripts/build-dmg.sh
      - uses: actions/upload-artifact@v3
        with:
          name: macos-dmg
          path: dist/monoterminal-macos.dmg
```

---

## 4. Implementation Timeline

### 4.1 12-Week Roadmap (3 months, 1.5 engineers)

**Weeks 1-2: Core PTY Abstraction**
- [ ] Add `portable-pty` dependency
- [ ] Implement `UnixPtyBackend` (Linux + macOS)
- [ ] Write unit tests (echo, resize, terminate)
- [ ] Integration tests (real shell sessions)
- [ ] Verify on Ubuntu 22.04 + macOS 13

**Owner:** rust-backend-lead  
**Milestone:** PTY abstraction complete, 80% test coverage

---

**Week 3: File Paths & Platform APIs**
- [ ] Implement cross-platform path resolution (`dirs` crate)
- [ ] Update database paths (Linux: `/var/lib`, macOS: `~/Library`)
- [ ] Update config paths (XDG Base Directory on Linux)
- [ ] Test on all 3 platforms

**Owner:** rust-engineer-storage  
**Milestone:** Platform-specific paths working

---

**Weeks 4-5: Service Management**
- [ ] systemd integration (Linux, Type=notify)
- [ ] launchd integration (macOS, KeepAlive)
- [ ] Unified `install-service` CLI command
- [ ] Test service lifecycle (install, start, stop, uninstall)

**Owner:** rust-backend-lead + devops-lead  
**Milestone:** Service management working on all platforms

---

**Week 6: Rendering Validation**
- [ ] Verify wgpu Vulkan backend on Ubuntu/Fedora
- [ ] Verify wgpu Metal backend on macOS (Intel + M1/M2)
- [ ] Performance testing (60 FPS target, SRS §5.1)
- [ ] Fix any platform-specific rendering bugs

**Owner:** gpu-rendering-engineer  
**Milestone:** Rendering working on Linux + macOS

---

**Weeks 7-8: CI & Build Matrix**
- [ ] Add Linux CI (Ubuntu 22.04, Fedora 38)
- [ ] Add macOS CI (macOS 13, Intel + Apple Silicon)
- [ ] Configure cross-platform test matrix
- [ ] Verify 80% coverage across all platforms

**Owner:** devops-lead + qa-lead  
**Milestone:** CI passing on all platforms

---

**Weeks 9-10: Distribution Packages**
- [ ] Create `.deb` package (Ubuntu/Debian)
- [ ] Create `.rpm` package (Fedora/RHEL)
- [ ] Create Homebrew formula (macOS)
- [ ] Create standalone tarballs (Linux + macOS)
- [ ] Test installation on clean VMs

**Owner:** devops-lead  
**Milestone:** Distribution packages ready

---

**Weeks 11-12: Integration Testing & Documentation**
- [ ] End-to-end tests on all platforms
- [ ] Performance benchmarks (latency, throughput, memory)
- [ ] Update documentation (installation guides per platform)
- [ ] Release notes + migration guide

**Owner:** qa-lead + technical-writer  
**Milestone:** Phase 3 acceptance criteria met

---

### 4.2 Parallelization Strategy

**Parallel Tracks:**
- **Track A (Weeks 1-3):** PTY + File Paths (rust-backend-lead + rust-engineer-storage)
- **Track B (Weeks 4-6):** Service Management + Rendering (rust-backend-lead + gpu-rendering-engineer)
- **Track C (Weeks 7-10):** CI + Distribution (devops-lead, parallel with integration testing)
- **Track D (Weeks 11-12):** Integration testing + Documentation (qa-lead + technical-writer)

**Critical Path:** PTY abstraction (Weeks 1-2) → Service management (Weeks 4-5) → Integration testing (Weeks 11-12)

**Total Calendar Time:** 12 weeks (3 months, aligns with SRS §7.3 estimate)

---

### 4.3 Dependencies on Phase 2

**Blockers (must complete before Phase 3):**
- ✅ Phase 2 core implementation complete (already done)
- ✅ Protocol schema stable (no breaking changes in Phase 3)
- ✅ Web client works cross-platform (already true, PWA design)

**Nice-to-Have (can proceed in parallel):**
- ⚠️ Phase 2 coverage measurement (in progress, ETA 1.5-2 hours)
- ⚠️ Phase 2 gate passage (deferred, not blocking Phase 3 planning)

**Decision:** Phase 3 planning can proceed now, implementation can start upon Phase 2 gate passage.

---

## 5. Risk Assessment

### 5.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **portable-pty incompatibility** | Low | High | Test early (Week 1), fallback to from-scratch if needed |
| **systemd socket activation complexity** | Medium | Low | Defer to Phase 3.5, not required for feature parity |
| **macOS notarization issues** | Medium | Medium | Test early, budget 1 week for Apple Developer account setup |
| **Vulkan driver issues on Linux** | Low | Medium | Test on multiple distros (Ubuntu, Fedora), document driver requirements |
| **M1/M2 Metal incompatibility** | Very Low | High | wgpu already supports Apple Silicon, test early |
| **Cross-platform test flakiness** | Medium | Medium | Isolate tests, retry logic, platform-specific timeouts |

### 5.2 Schedule Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **PTY implementation takes >2 weeks** | Low | Medium | portable-pty reduces risk, but have 1-week buffer |
| **CI matrix setup takes >1 week** | Medium | Low | Use existing GitHub Actions, well-documented |
| **Distribution packaging blockers** | Medium | Low | Defer to post-Phase 3 if needed, binaries work day 1 |

---

## 6. Success Metrics

### 6.1 Phase 3 Acceptance Criteria (SRS §7.3)

**Platform Coverage:**
- ✅ Ubuntu 22.04+, Debian 11+, Fedora 38+ support
- ✅ macOS 12+ (Monterey) support (Intel + Apple Silicon)
- ✅ Feature parity with Windows master

**Test Coverage:**
- ✅ 80% test coverage across all platforms
- ✅ CI passing on Linux + macOS + Windows
- ✅ Integration tests on all 3 platforms

**Distribution:**
- ✅ apt/rpm packages (Linux)
- ✅ Homebrew formula (macOS)
- ✅ Installation guides per platform

### 6.2 Performance Targets (carried over from Phase 1/2)

**Must maintain SRS §5.1 targets across all platforms:**
- ✅ 60 FPS rendering (wgpu on Vulkan/Metal/DX12)
- ✅ <30ms p95 latency (LAN WebSocket)
- ✅ 1000 concurrent sessions tested
- ✅ <10s reconnect after mobile backgrounding

---

## 7. Open Questions

### 7.1 For eng-director

**Q1: Socket Activation Priority**
- SRS §7.3 mentions "socket activation" for Linux/macOS
- SRS §7.1 says Windows doesn't have it (always-running service)
- Should we defer socket activation to Phase 3.5 for feature parity?

**Q2: Distribution Timeline**
- apt/rpm/Homebrew packages add ~2 weeks to timeline
- Can we ship binaries-only first, add packages in Phase 3.5?

**Q3: ARM64 Support**
- SRS §6.2 shows ARM64 Windows/Linux as "optional"
- macOS M1/M2 is "required" (Apple Silicon)
- Should Phase 3 cover macOS ARM64 only, defer Linux/Windows ARM64?

### 7.2 For rust-backend-lead

**Q4: portable-pty vs From-Scratch**
- Is portable-pty mature enough for production?
- Have we validated it works with our `PtyBackend` trait design?

**Q5: Async I/O Strategy**
- ConPTY uses `spawn_blocking` for Phase 1 simplicity
- Should Unix PTY use the same pattern, or implement native tokio async?

---

## 8. Next Steps

**Immediate Actions (this session):**
1. ✅ Review SRS §7.3 requirements (complete)
2. ✅ Identify architecture gaps (complete)
3. ✅ Draft ADR-015 through ADR-017 (complete)
4. ✅ Estimate timeline (12 weeks, complete)

**Pending eng-director Review:**
- Approve Phase 3 architecture overview
- Answer open questions (Q1-Q3)
- Approve timeline and resource allocation

**Next Session (Phase 3 Kickoff):**
- Finalize ADR-015, ADR-016, ADR-017
- Assign Week 1-2 implementation tasks (PTY abstraction)
- Set up Linux + macOS CI infrastructure

---

## References

- **SRS §7.3:** Phase 3 acceptance criteria, platform requirements
- **SRS §2.1.2:** PTY implementation (Linux, macOS, Windows)
- **SRS §6.2:** CI test matrix (Ubuntu, macOS, Windows)
- **ADR-005:** Daemon lifecycle (Windows Service, will extend to systemd/launchd)
- **ADR-011:** P2P networking (already cross-platform, no changes needed)
- **ADR-012:** Persistence layer (SQLite paths need platform-specific updates)

---

**Status:** Ready for review (eng-director, rust-backend-lead, devops-lead)

**Timeline:** 1-2 hours to complete (parallel with Phase 2 coverage measurement)

**Next:** Await eng-director approval, then finalize ADRs and begin Week 1 implementation.
