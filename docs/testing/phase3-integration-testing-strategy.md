# Phase 3 Integration Testing Strategy

**Task:** task-60  
**Date:** 2026-08-20  
**Owner:** qa-lead  
**Phase:** Phase 3 Weeks 11-12  
**Status:** Planning

---

## Executive Summary

This document defines the comprehensive integration testing strategy for Phase 3 (Linux + macOS platform expansion), covering cross-platform test scenarios, performance benchmarks, feature parity validation, and Phase 3 gate acceptance criteria per SRS §7.3.

**Scope:** 3 platforms (Windows, Linux, macOS) × 5 feature areas = 15+ integration test combinations

**Timeline:** Weeks 11-12 (integration testing + documentation)

---

## 1. Phase 3 Testing Objectives

### 1.1 Primary Goals

1. **Cross-Platform Feature Parity**
   - Validate Windows features work identically on Linux and macOS
   - Same protocol, same web client, same user experience
   - No platform-specific bugs or regressions

2. **Service Management Integration**
   - systemd integration (Linux): Type=notify, socket activation
   - launchd integration (macOS): socket activation
   - Windows Service baseline (existing, regression testing only)

3. **PTY Abstraction Validation**
   - Linux: openpty + termios
   - macOS: openpty + termios (BSD variant)
   - Windows: ConPTY (existing baseline)
   - Consistent behavior across all three

4. **Rendering Consistency**
   - 60 FPS target on all platforms
   - Visual parity (font rendering, colors, cursor)
   - wgpu backend validation: Vulkan (Linux), Metal (macOS), DirectX 12 (Windows)

5. **Distribution Package Validation**
   - .deb (Ubuntu/Debian): installation, upgrade, removal
   - .rpm (Fedora/RHEL): installation, upgrade, removal
   - Homebrew (macOS): installation, upgrade, removal
   - Tarball (universal fallback): manual installation

---

## 2. Test Matrix

### 2.1 Platform × Feature Matrix

| Platform | PTY | Service Mgmt | Rendering | Distribution | Integration | Total Tests |
|----------|-----|--------------|-----------|--------------|-------------|-------------|
| **Ubuntu 22.04** | 15 | 12 | 20 | 8 | 10 | 65 |
| **Debian 11** | 15 | 12 | 20 | 8 | 10 | 65 |
| **Fedora 38** | 15 | 12 | 20 | 8 | 10 | 65 |
| **macOS 13 (Intel)** | 15 | 12 | 20 | 6 | 10 | 63 |
| **macOS 14 (Apple Silicon)** | 15 | 12 | 20 | 6 | 10 | 63 |
| **Windows 10/11** | 15 | 10 | 20 | 4 | 10 | 59 (baseline) |
| **Cross-Platform** | - | - | - | - | 25 | 25 |
| **Total** | 90 | 70 | 120 | 40 | 85 | **405 tests** |

### 2.2 Test Type Breakdown

| Test Type | Count | Platforms | Automation | Execution Time |
|-----------|-------|-----------|------------|----------------|
| Unit Tests | 176 | All | ✅ Automated | 5 min |
| Integration Tests | 150 | Per-platform | ✅ Automated | 15 min per platform |
| E2E Tests | 50 | All | ✅ Automated | 10 min per platform |
| Performance Tests | 20 | All | ✅ Automated | 20 min per platform |
| Manual Tests | 25 | All | ❌ Manual | 60 min per platform |
| **Total** | **421** | **6 platforms** | **95% automated** | **~3 hours full suite** |

---

## 3. Cross-Platform Integration Test Scenarios

### 3.1 PTY Integration Tests (15 tests × 6 platforms = 90 tests)

#### T1.1: PTY Creation and Spawning
**Test:** Spawn shell via platform-specific PTY
**Platforms:** All
**Steps:**
1. Create PTY pair (master/slave)
2. Spawn shell process (/bin/bash, /bin/zsh, cmd.exe)
3. Verify process created with correct PID
4. Verify shell prompt received
5. Cleanup: terminate shell, close PTY

**Expected:**
- Linux: openpty() success, child process via fork/exec
- macOS: openpty() success, child process via fork/exec
- Windows: ConPTY success, child process via CreateProcess

**Pass Criteria:** Shell spawns within 100ms, prompt received within 500ms

---

#### T1.2: PTY I/O (Input/Output)
**Test:** Bidirectional I/O through PTY
**Platforms:** All
**Steps:**
1. Spawn shell via PTY
2. Write "echo hello\\n" to PTY master
3. Read output from PTY master
4. Verify "hello" in output

**Expected:**
- Linux: read/write via Unix file descriptors
- macOS: read/write via Unix file descriptors
- Windows: ReadFile/WriteFile via ConPTY handles

**Pass Criteria:** Round-trip I/O <10ms, output matches input

---

#### T1.3: PTY Resize
**Test:** Terminal window resize via SIGWINCH
**Platforms:** All
**Steps:**
1. Spawn shell with 80x24 size
2. Resize to 120x30
3. Verify $COLUMNS and $LINES in shell
4. Resize to 100x40
5. Verify again

**Expected:**
- Linux: ioctl(TIOCSWINSZ) + SIGWINCH signal
- macOS: ioctl(TIOCSWINSZ) + SIGWINCH signal
- Windows: SetConsoleScreenBufferSize + ConPTY resize

**Pass Criteria:** Resize propagates within 50ms, shell variables update

---

#### T1.4: PTY Environment Variables
**Test:** Environment variables passed to child shell
**Platforms:** All
**Steps:**
1. Set custom env vars (TEST_VAR=value)
2. Spawn shell with env vars
3. Read $TEST_VAR from shell
4. Verify value matches

**Expected:**
- All platforms: env vars passed via execve/CreateProcess

**Pass Criteria:** All env vars present and correct

---

#### T1.5: PTY Signal Handling (Ctrl+C, Ctrl+Z)
**Test:** Signal forwarding to child process
**Platforms:** Linux, macOS (Windows: different model)
**Steps:**
1. Spawn long-running process (sleep 60)
2. Send SIGINT (Ctrl+C)
3. Verify process terminates
4. Spawn another process
5. Send SIGTSTP (Ctrl+Z)
6. Verify process suspends

**Expected:**
- Linux/macOS: tcsetpgrp + signal forwarding
- Windows: GenerateConsoleCtrlEvent

**Pass Criteria:** Signals delivered within 10ms, process state changes

---

#### T1.6-T1.15: Additional PTY Tests
- **T1.6:** PTY error handling (slave closed, master broken pipe)
- **T1.7:** PTY character encoding (UTF-8, special characters)
- **T1.8:** PTY flow control (stop/start)
- **T1.9:** PTY termios settings (echo, canonical mode)
- **T1.10:** PTY session leadership (process groups)
- **T1.11:** PTY concurrent I/O (multiple writes)
- **T1.12:** PTY large buffer handling (>4KB writes)
- **T1.13:** PTY non-blocking I/O
- **T1.14:** PTY cleanup on daemon shutdown
- **T1.15:** PTY recovery after temporary failure

---

### 3.2 Service Management Integration Tests (12 tests × 6 platforms = 70 tests)

#### T2.1: Service Installation
**Test:** Install daemon as system service
**Platforms:** All
**Steps:**
1. Install package (systemctl enable, launchctl load, sc create)
2. Verify service file created
3. Verify service registered with system
4. Check service status (should be inactive/loaded)

**Expected:**
- Linux: /etc/systemd/system/monoterminal.service created
- macOS: ~/Library/LaunchAgents/com.monoterminal.daemon.plist created
- Windows: Service registered in SCM

**Pass Criteria:** Service file exists, service registered, status correct

---

#### T2.2: Service Start
**Test:** Start daemon service
**Platforms:** All
**Steps:**
1. Start service (systemctl start, launchctl start, sc start)
2. Wait 2 seconds
3. Check service status (should be active/running)
4. Verify PID file created
5. Verify process running

**Expected:**
- Linux: systemd Type=notify, sd_notify("READY=1")
- macOS: launchd receives RunAtLoad signal
- Windows: SERVICE_RUNNING state

**Pass Criteria:** Service starts within 2s, PID valid, process healthy

---

#### T2.3: Service Socket Activation (Linux, macOS only)
**Test:** Daemon starts on first connection (not boot)
**Platforms:** Linux, macOS
**Steps:**
1. Stop daemon if running
2. Ensure socket unit enabled (systemd) or socket loaded (launchd)
3. Connect to socket (Unix domain socket)
4. Verify daemon auto-starts
5. Verify connection succeeds

**Expected:**
- Linux: systemd socket activation via sd-daemon
- macOS: launchd socket activation via Sockets key

**Pass Criteria:** Daemon starts within 1s of connection, no manual start needed

---

#### T2.4: Service Stop
**Test:** Graceful shutdown
**Platforms:** All
**Steps:**
1. Start service
2. Create 2 active sessions
3. Stop service (systemctl stop, launchctl stop, sc stop)
4. Verify graceful shutdown (sessions persisted)
5. Verify process terminated
6. Verify cleanup (PID file removed)

**Expected:**
- All platforms: SIGTERM received, 10s graceful shutdown
- Sessions saved to SQLite before exit

**Pass Criteria:** Shutdown within 10s, sessions persisted, no resource leaks

---

#### T2.5: Service Restart
**Test:** Service restart preserves sessions
**Platforms:** All
**Steps:**
1. Start service, create 3 sessions
2. Restart service
3. Verify sessions recovered
4. Verify session state intact (scrollback, env vars)

**Expected:**
- SQLite persistence layer recovers sessions
- All sessions transition to DETACHED → RUNNING

**Pass Criteria:** All sessions recovered within 5s of restart

---

#### T2.6-T2.12: Additional Service Tests
- **T2.6:** Service auto-restart on crash (systemd Restart=always)
- **T2.7:** Service logs to system journal (journalctl, syslog, Event Log)
- **T2.8:** Service runs as correct user (monoterminal user, not root)
- **T2.9:** Service respects resource limits (systemd LimitNOFILE)
- **T2.10:** Service uninstallation (package removal, service cleanup)
- **T2.11:** Service upgrade (in-place upgrade, no downtime)
- **T2.12:** Service status reporting (systemctl status, launchctl list)

---

### 3.3 Rendering Integration Tests (20 tests × 6 platforms = 120 tests)

#### T3.1: 60 FPS Rendering (SRS §1.3 target)
**Test:** Sustained 60 FPS rendering
**Platforms:** All
**Steps:**
1. Start daemon with local UI
2. Create terminal session
3. Run high-throughput command (cat large file)
4. Measure FPS over 60 seconds
5. Calculate mean, p50, p95, p99

**Expected:**
- Linux: wgpu + Vulkan backend, 60 FPS sustained
- macOS: wgpu + Metal backend, 60 FPS sustained  
- Windows: wgpu + DirectX 12 backend, 60 FPS sustained (baseline)

**Pass Criteria:** Mean ≥60 FPS, p95 ≥55 FPS, p99 ≥50 FPS

---

#### T3.2: Visual Parity (Cross-Platform)
**Test:** Identical rendering across platforms
**Platforms:** All
**Steps:**
1. Render identical terminal content on all platforms
2. Capture screenshots (1920x1080, 24-bit color)
3. Compare pixel-by-pixel
4. Calculate difference percentage

**Expected:**
- Font rendering: identical (same font: JetBrains Mono)
- Colors: identical (same color scheme)
- Cursor: identical position and style

**Pass Criteria:** <1% pixel difference (accounting for font hinting differences)

---

#### T3.3: GPU Backend Validation
**Test:** wgpu backend per platform
**Platforms:** All
**Steps:**
1. Query wgpu adapter
2. Verify backend type
3. Verify feature support
4. Run rendering test

**Expected:**
- Linux: wgpu::Backend::Vulkan
- macOS: wgpu::Backend::Metal
- Windows: wgpu::Backend::Dx12

**Pass Criteria:** Correct backend selected, rendering succeeds

---

#### T3.4: HiDPI Rendering (macOS Retina, Windows HighDPI)
**Test:** HiDPI scaling correctness
**Platforms:** macOS, Windows (Linux: varies)
**Steps:**
1. Set display scaling to 2.0x (Retina, 200%)
2. Render terminal
3. Verify logical size vs physical size
4. Verify text sharpness

**Expected:**
- macOS: NSWindow scaleFactor = 2.0, Metal texture 2x
- Windows: GetDpiForWindow(), D3D12 texture scaled

**Pass Criteria:** Text sharp at all scales, no blurriness

---

#### T3.5-T3.20: Additional Rendering Tests
- **T3.5:** Color accuracy (256 colors, true color)
- **T3.6:** Font rendering (bold, italic, underline)
- **T3.7:** Cursor styles (block, beam, underline, blinking)
- **T3.8:** Selection rendering (mouse selection, keyboard selection)
- **T3.9:** Scrollback rendering (10k lines, smooth scrolling)
- **T3.10:** VT escape sequence rendering (SGR, cursor movement)
- **T3.11:** Performance under load (high-throughput cat, 100k lines/s)
- **T3.12:** Memory usage (stable under long sessions)
- **T3.13:** Window resize handling (smooth, no flicker)
- **T3.14:** Multi-monitor support (move between monitors)
- **T3.15:** Fullscreen mode
- **T3.16:** Transparency/blur effects (platform-specific)
- **T3.17:** IME input (Chinese, Japanese, Korean)
- **T3.18:** Right-to-left text (Arabic, Hebrew)
- **T3.19:** Emoji rendering
- **T3.20:** Ligature support (programming fonts)

---

### 3.4 Distribution Package Integration Tests (8 tests × 5 distros = 40 tests)

#### T4.1: Package Installation (.deb)
**Test:** Install .deb package on Ubuntu/Debian
**Platforms:** Ubuntu 22.04, Debian 11
**Steps:**
1. Download .deb package
2. Install: `sudo dpkg -i monoterminal_0.1.0_amd64.deb`
3. Verify binary installed: `/usr/bin/monoterminal`
4. Verify systemd unit: `/etc/systemd/system/monoterminal.service`
5. Verify dependencies installed

**Expected:**
- Binary executable, correct permissions
- Service file valid
- Dependencies satisfied (libc6, libssl3, etc.)

**Pass Criteria:** Package installs without errors, all files present

---

#### T4.2: Package Installation (.rpm)
**Test:** Install .rpm package on Fedora
**Platforms:** Fedora 38
**Steps:**
1. Download .rpm package
2. Install: `sudo rpm -i monoterminal-0.1.0.x86_64.rpm`
3. Verify binary installed: `/usr/bin/monoterminal`
4. Verify systemd unit: `/etc/systemd/system/monoterminal.service`

**Expected:**
- Same as .deb, RPM format

**Pass Criteria:** Package installs without errors, all files present

---

#### T4.3: Package Installation (Homebrew)
**Test:** Install via Homebrew on macOS
**Platforms:** macOS 13+
**Steps:**
1. Add tap: `brew tap monoterminal/tap`
2. Install: `brew install monoterminal`
3. Verify binary: `/opt/homebrew/bin/monoterminal`
4. Verify launchd plist: `~/Library/LaunchAgents/com.monoterminal.daemon.plist`

**Expected:**
- Binary installed in Homebrew prefix
- launchd plist created

**Pass Criteria:** Installation succeeds, binary runs

---

#### T4.4: Package Upgrade
**Test:** In-place upgrade from 0.1.0 → 0.2.0
**Platforms:** All
**Steps:**
1. Install version 0.1.0
2. Create 3 sessions
3. Upgrade to 0.2.0 (dpkg, rpm, brew upgrade)
4. Verify sessions preserved
5. Verify service restarted

**Expected:**
- SQLite schema migration succeeds
- Sessions recovered after upgrade

**Pass Criteria:** Zero downtime, sessions preserved

---

#### T4.5: Package Removal
**Test:** Clean uninstallation
**Platforms:** All
**Steps:**
1. Install package
2. Create sessions (persist to SQLite)
3. Uninstall package (dpkg -r, rpm -e, brew uninstall)
4. Verify binary removed
5. Verify service unregistered
6. Check for leftover files

**Expected:**
- Binary removed
- Service files removed
- Config files preserved (user data not deleted)

**Pass Criteria:** Clean removal, user data preserved in ~/.monoterminal/

---

#### T4.6-T4.8: Additional Distribution Tests
- **T4.6:** Package dependency resolution (missing libs, auto-install)
- **T4.7:** Package signature verification (GPG signed .deb/.rpm)
- **T4.8:** Tarball installation (manual ./install.sh, system-agnostic)

---

### 3.5 Cross-Platform E2E Integration Tests (25 tests)

#### T5.1: Windows Client → Linux Server
**Test:** Web client on Windows connects to Linux daemon
**Platforms:** Windows (client) + Linux (server)
**Steps:**
1. Start Linux daemon
2. Start Windows web browser
3. Connect to Linux daemon (https://linux-host:5000)
4. Create session, run commands
5. Verify output correct

**Expected:**
- Protocol compatibility (same Protobuf)
- TLS handshake succeeds
- Session lifecycle works

**Pass Criteria:** Connection succeeds, full session functionality

---

#### T5.2: macOS Client → Windows Server
**Test:** Web client on macOS connects to Windows daemon
**Platforms:** macOS (client) + Windows (server)
**Steps:**
1. Start Windows daemon
2. Start macOS Safari
3. Connect to Windows daemon
4. Create session, run commands

**Expected:**
- Cross-platform protocol works
- No Safari-specific issues

**Pass Criteria:** Connection succeeds, full functionality

---

#### T5.3: Linux Client → macOS Server
**Test:** Web client on Linux connects to macOS daemon
**Platforms:** Linux (client) + macOS (server)
**Steps:**
1. Start macOS daemon
2. Start Linux Firefox
3. Connect to macOS daemon
4. Create session

**Expected:**
- Full cross-platform compatibility

**Pass Criteria:** Connection succeeds

---

#### T5.4: Multi-Platform Session Sharing
**Test:** 3 clients (Windows, Linux, macOS) attach to same session
**Platforms:** All
**Steps:**
1. Start daemon on any platform
2. Client 1 (Windows) creates session
3. Client 2 (Linux) attaches to session
4. Client 3 (macOS) attaches to session
5. Verify all clients see same output
6. Type in Client 1, verify clients 2+3 receive

**Expected:**
- Multi-client attach works cross-platform
- RBAC enforced (owner/viewer permissions)

**Pass Criteria:** All clients synchronized, <50ms propagation

---

#### T5.5-T5.25: Additional Cross-Platform E2E Tests
- **T5.5:** Session recovery across platforms (create on Windows, recover on Linux)
- **T5.6:** Cross-platform P2P connection (WebRTC Linux ↔ macOS)
- **T5.7:** Cross-platform file transfer (if implemented)
- **T5.8:** Cross-platform clipboard sync (OSC 52)
- **T5.9:** Mixed-platform load test (10 sessions across 3 platforms)
- **T5.10:** Network failure recovery (disconnect/reconnect cross-platform)
- **T5.11-T5.25:** Additional scenarios (auth, discovery, collaboration)

---

## 4. Performance Benchmarking

### 4.1 Performance Test Matrix

| Benchmark | Metric | Target | Platforms | Automation |
|-----------|--------|--------|-----------|------------|
| PTY I/O Latency | p95 latency | <10ms LAN | All | ✅ |
| Rendering FPS | Mean FPS | ≥60 FPS | All | ✅ |
| Memory Usage | RSS after 1h | <200MB | All | ✅ |
| Service Startup | Time to ready | <2s | All | ✅ |
| Package Install | Time to install | <30s | All | ❌ Manual |

### 4.2 PTY I/O Latency Benchmark

**Test:** Round-trip latency (write → read)
**Platforms:** All
**Method:**
1. Spawn PTY
2. Write 1000 single-character commands
3. Measure time from write() to read() receiving echo
4. Calculate p50, p95, p99

**Target:** <10ms p95 (SRS §1.3: <30ms LAN, PTY should be <10ms)

**Expected Results:**
- Linux: 1-5ms p95 (Unix domain socket, no network)
- macOS: 1-5ms p95 (Unix domain socket)
- Windows: 2-8ms p95 (Named pipe, slightly slower)

---

### 4.3 Rendering FPS Benchmark

**Test:** Sustained FPS under load
**Platforms:** All
**Method:**
1. Render terminal with high-throughput input (cat 1MB file)
2. Measure frame times over 60 seconds
3. Calculate FPS (1000ms / frame_time_ms)

**Target:** ≥60 FPS mean, ≥55 FPS p95

**Expected Results:**
- All platforms: 60-75 FPS (wgpu backend efficient)

---

### 4.4 Memory Usage Benchmark

**Test:** Memory stability over time
**Platforms:** All
**Method:**
1. Start daemon
2. Create 10 sessions
3. Run high-throughput commands (continuous output)
4. Measure RSS every 10 minutes for 1 hour
5. Calculate growth rate

**Target:** <200MB RSS after 1h, <5% growth

**Expected Results:**
- Linux: ~150MB RSS (efficient allocator)
- macOS: ~180MB RSS (system allocator)
- Windows: ~200MB RSS (jemalloc)

---

### 4.5 Service Startup Time Benchmark

**Test:** Cold start to ready state
**Platforms:** All
**Method:**
1. Stop daemon
2. Start daemon (systemctl start, launchctl start, sc start)
3. Measure time to sd_notify("READY=1") / SERVICE_RUNNING

**Target:** <2s startup time

**Expected Results:**
- Linux: 0.5-1.5s (systemd Type=notify)
- macOS: 0.8-2.0s (launchd startup)
- Windows: 1.0-2.5s (Service start)

---

## 5. Phase 3 Gate Acceptance Criteria

### 5.1 SRS §7.3 Acceptance Criteria

| Criterion | Target | Validation Method | Status |
|-----------|--------|-------------------|--------|
| **Ubuntu 22.04+ support** | Full support | T1-T5 test suites | ⏳ Pending |
| **Debian 11+ support** | Full support | T1-T5 test suites | ⏳ Pending |
| **Fedora 38+ support** | Full support | T1-T5 test suites | ⏳ Pending |
| **macOS 12+ support** | Monterey+ | T1-T5 test suites | ⏳ Pending |
| **Feature parity** | Same as Windows | T5 cross-platform tests | ⏳ Pending |
| **80% test coverage** | Coverage ≥80% | cargo-tarpaulin or manual | ⏳ Pending |
| **60 FPS rendering** | All platforms | T3.1 FPS benchmark | ⏳ Pending |
| **<30ms LAN latency** | p95 latency | T4.2 latency benchmark | ⏳ Pending |
| **Distribution packages** | .deb, .rpm, Homebrew | T4 installation tests | ⏳ Pending |
| **Service management** | systemd, launchd | T2 service tests | ⏳ Pending |

### 5.2 Quality Metrics

| Metric | Phase 1 | Phase 2 | Phase 3 Target |
|--------|---------|---------|----------------|
| **Test Coverage** | 41% → 77% | 77.2% | **80%** |
| **Test Count** | 62 tests | 176 tests | **405 tests** |
| **Platforms** | 1 (Windows) | 1 (Windows) | **3 (Win+Lin+Mac)** |
| **Critical Bugs** | <5 per release | <5 per release | **<5 per release** |
| **P0 Blockers** | 2 (Phase 1) | 0 (Phase 2) | **0 (Phase 3)** |

### 5.3 Exit Criteria

**Phase 3 testing is COMPLETE when:**

1. ✅ All 405 tests passing on all platforms (95% pass rate minimum)
2. ✅ 80% code coverage achieved across all platforms
3. ✅ Performance benchmarks meet targets (60 FPS, <30ms latency)
4. ✅ Distribution packages install successfully on all platforms
5. ✅ Service management validated (systemd, launchd, Windows Service)
6. ✅ Cross-platform E2E tests passing (25/25)
7. ✅ Zero P0 blockers, <5 P1 bugs
8. ✅ Documentation complete (installation guides, troubleshooting)

**Escalation:** If any criterion fails, escalate to eng-director for gate decision

---

## 6. Test Automation Plan

### 6.1 CI Matrix (GitHub Actions)

```yaml
strategy:
  matrix:
    os:
      - ubuntu-22.04
      - ubuntu-latest  
      - macos-13        # Intel
      - macos-14        # Apple Silicon
      - windows-2022
      - windows-latest
    test-suite:
      - unit
      - integration-pty
      - integration-service
      - integration-rendering
      - e2e
      - performance
```

**Total:** 6 OSes × 6 test suites = 36 CI jobs per PR

### 6.2 Test Execution Timeline

| Test Suite | Duration | Parallelization | Total Time |
|------------|----------|-----------------|------------|
| Unit tests | 5 min | 1× per OS | 5 min |
| Integration (PTY) | 10 min | 1× per OS | 10 min |
| Integration (Service) | 8 min | 1× per OS | 8 min |
| Integration (Rendering) | 15 min | 1× per OS | 15 min |
| E2E tests | 12 min | 1× per OS | 12 min |
| Performance | 20 min | 1× per OS | 20 min |
| **Total per OS** | **70 min** | **6 parallel** | **20 min** |

**Parallel execution:** All 6 OSes run simultaneously → **20 minutes total CI time**

### 6.3 Test Environments

| Platform | Environment | Runner | Cost |
|----------|-------------|--------|------|
| Ubuntu 22.04 | GitHub Actions | ubuntu-22.04 | Free |
| Debian 11 | Docker container | ubuntu-latest + Docker | Free |
| Fedora 38 | Docker container | ubuntu-latest + Docker | Free |
| macOS 13 (Intel) | GitHub Actions | macos-13 | $0.08/min |
| macOS 14 (M1) | GitHub Actions | macos-14 | $0.16/min |
| Windows 2022 | GitHub Actions | windows-2022 | Free |

**Estimated CI cost:** ~$5-10 per PR (macOS runners)

---

## 7. Manual Test Checklists

### 7.1 Installation Checklist (per platform)

**Linux (.deb - Ubuntu 22.04):**
- [ ] Download .deb package from release
- [ ] `sudo dpkg -i monoterminal_0.1.0_amd64.deb`
- [ ] Verify binary: `which monoterminal` → `/usr/bin/monoterminal`
- [ ] Verify service: `systemctl status monoterminal` → loaded
- [ ] Start service: `sudo systemctl start monoterminal`
- [ ] Connect via web: `https://localhost:5000`
- [ ] Create session, run commands
- [ ] Stop service: `sudo systemctl stop monoterminal`
- [ ] Uninstall: `sudo dpkg -r monoterminal`
- [ ] Verify cleanup: binary removed, service unregistered

**macOS (Homebrew - macOS 13):**
- [ ] `brew tap monoterminal/tap`
- [ ] `brew install monoterminal`
- [ ] Verify binary: `which monoterminal` → `/opt/homebrew/bin/monoterminal`
- [ ] Start service: `brew services start monoterminal`
- [ ] Connect via web: `https://localhost:5000`
- [ ] Create session, run commands
- [ ] Stop service: `brew services stop monoterminal`
- [ ] Uninstall: `brew uninstall monoterminal`
- [ ] Verify cleanup

**Windows (Installer - Windows 11):**
- [ ] Run monoterminal-setup-0.1.0.exe
- [ ] Follow installation wizard
- [ ] Verify binary: `where monoterminal` → `C:\Program Files\Monoterminal\monoterminal.exe`
- [ ] Verify service: `sc query monoterminal` → RUNNING
- [ ] Connect via web: `https://localhost:5000`
- [ ] Create session, run PowerShell commands
- [ ] Uninstall: Control Panel → Uninstall
- [ ] Verify cleanup

### 7.2 Service Lifecycle Checklist (per platform)

**Linux (systemd):**
- [ ] Install package
- [ ] `sudo systemctl enable monoterminal`
- [ ] `sudo systemctl start monoterminal`
- [ ] `systemctl status monoterminal` → active (running)
- [ ] `journalctl -u monoterminal -n 50` → check logs
- [ ] Create 3 sessions
- [ ] `sudo systemctl restart monoterminal` → sessions preserved
- [ ] `sudo systemctl stop monoterminal` → graceful shutdown
- [ ] `sudo systemctl disable monoterminal`

**macOS (launchd):**
- [ ] Install via Homebrew
- [ ] `brew services start monoterminal`
- [ ] `launchctl list | grep monoterminal` → verify loaded
- [ ] Create 3 sessions
- [ ] `brew services restart monoterminal` → sessions preserved
- [ ] `brew services stop monoterminal` → graceful shutdown

**Windows (Service Control Manager):**
- [ ] Install package
- [ ] `sc start monoterminal`
- [ ] `sc query monoterminal` → RUNNING
- [ ] Create 3 sessions
- [ ] `sc stop monoterminal` → graceful shutdown
- [ ] `sc config monoterminal start=auto` → auto-start on boot

---

## 8. Risk Mitigation

### 8.1 Known Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **VM unavailability** | MEDIUM | HIGH | Use Docker containers for Linux distros, cloud VMs for macOS |
| **CI macOS runner cost** | LOW | MEDIUM | Optimize test duration, use self-hosted runner if cost exceeds budget |
| **Platform-specific bugs** | MEDIUM | MEDIUM | Extensive manual testing, community beta testing |
| **Service integration issues** | MEDIUM | HIGH | Early integration testing, systemd/launchd experts review |
| **Rendering backend issues** | LOW | MEDIUM | Fallback to software renderer (Cairo) if wgpu fails |
| **Package signing issues** | LOW | LOW | GPG signing setup early, test on fresh VMs |

### 8.2 Contingency Plans

**If 80% coverage target not met:**
- Accept manual projection (validated in Phase 2)
- Document gaps for Phase 4
- Escalate to eng-director

**If macOS CI cost exceeds budget:**
- Reduce test frequency (only on release branches)
- Use self-hosted runner (M1 Mac Mini)
- Manual testing as fallback

**If critical platform bug found:**
- Create P0 bug, assign to platform owner
- Block Phase 3 gate until resolved
- Consider platform-specific workaround

---

## 9. Timeline and Ownership

### 9.1 Week 11: Integration Testing

| Day | Activity | Owner | Deliverable |
|-----|----------|-------|-------------|
| Mon | PTY integration tests (T1.1-T1.15) | qa-lead | 90 tests passing |
| Tue | Service integration tests (T2.1-T2.12) | qa-lead | 70 tests passing |
| Wed | Rendering integration tests (T3.1-T3.20) | qa-lead + gpu-rendering-engineer | 120 tests passing |
| Thu | Distribution tests (T4.1-T4.8) | qa-lead + devops-lead | 40 tests passing |
| Fri | Cross-platform E2E tests (T5.1-T5.25) | qa-lead + test-engineer-e2e | 25 tests passing |

### 9.2 Week 12: Performance + Documentation

| Day | Activity | Owner | Deliverable |
|-----|----------|-------|-------------|
| Mon | Performance benchmarks (§4.2-4.5) | performance-engineer | Benchmark results report |
| Tue | Coverage measurement (§5.2) | qa-lead | 80% coverage validation |
| Wed | Manual testing (§7.1-7.2) | qa-lead + test-engineer-e2e | Manual test results |
| Thu | Bug triage + fixes | All engineers | P0 bugs fixed, P1 bugs documented |
| Fri | Phase 3 gate decision | eng-director + qa-lead | PASS/DEFER decision |

---

## 10. Success Criteria Summary

**Phase 3 Integration Testing is SUCCESSFUL when:**

✅ **405 tests created and executed** (380+ passing, 95% pass rate)  
✅ **80% code coverage** achieved across all platforms  
✅ **Performance targets met** (60 FPS, <30ms latency, <200MB memory)  
✅ **Distribution packages validated** (.deb, .rpm, Homebrew)  
✅ **Service management validated** (systemd, launchd, Windows Service)  
✅ **Cross-platform parity confirmed** (25/25 E2E tests passing)  
✅ **Zero P0 blockers, <5 P1 bugs**  
✅ **Documentation complete** (installation guides, troubleshooting, migration)

**Phase 3 Gate:** ✅ **RECOMMEND PASS** when all criteria met

---

**Document Status:** DRAFT  
**Next Review:** After VM/CI availability  
**Approval Required:** eng-director

---

**End of Integration Testing Strategy**
