# Phase 3 Test Suite Specifications

**Task:** task-60  
**Date:** 2026-08-20  
**Owner:** qa-lead  
**Phase:** Phase 3 Weeks 11-12  
**Status:** Planning

---

## Executive Summary

This document defines the detailed test suite specifications for Phase 3 cross-platform validation, including test case IDs, acceptance criteria, automation status, and execution procedures for all 405 tests across 6 platforms.

**Scope:** Executable test specifications for PTY, Service, Rendering, Distribution, and E2E test suites

**Timeline:** Weeks 11-12 (integration testing execution)

---

## 1. Test Suite Overview

### 1.1 Test Organization

| Test Suite | Test Count | Automation | Platforms | Execution Time | File Location |
|------------|-----------|------------|-----------|----------------|---------------|
| **PTY Integration** | 90 | ✅ 100% | All 6 | 10 min | `tests/integration/pty/` |
| **Service Management** | 70 | ✅ 100% | All 6 | 8 min | `tests/integration/service/` |
| **Rendering** | 120 | ✅ 95% | All 6 | 15 min | `tests/integration/rendering/` |
| **Distribution** | 40 | ❌ 50% | All 6 | 20 min | `tests/integration/distribution/` |
| **E2E Cross-Platform** | 85 | ✅ 90% | All 6 | 12 min | `tests/e2e/cross_platform/` |
| **Total** | **405** | **90%** | **All 6** | **65 min** | `tests/` |

### 1.2 Platform Matrix

| Platform | Tests | Pass Rate Target | CI Runner |
|----------|-------|------------------|-----------|
| Ubuntu 22.04 | 65 | ≥95% | ubuntu-22.04 |
| Debian 11 | 65 | ≥95% | ubuntu-latest + Docker |
| Fedora 38 | 65 | ≥95% | ubuntu-latest + Docker |
| macOS 13 (Intel) | 63 | ≥95% | macos-13 |
| macOS 14 (Apple Silicon) | 63 | ≥95% | macos-14 |
| Windows 2022 | 59 | ≥95% (baseline) | windows-2022 |
| Cross-Platform | 25 | ≥90% | All |
| **Total** | **405** | **≥95%** | **6 runners** |

---

## 2. PTY Integration Test Specifications

### 2.1 Test Suite: PTY Creation and Lifecycle

**File:** `tests/integration/pty/creation.rs`  
**Tests:** 15 tests × 6 platforms = 90 tests  
**Automation:** ✅ 100% automated

---

#### T1.1: PTY Creation and Shell Spawning

**Test ID:** `PTY-001`  
**Priority:** P0 (Critical)  
**Platforms:** All

**Objective:** Verify PTY can be created and shell process spawned

**Preconditions:**
- Daemon running
- No existing PTY sessions for test user

**Test Steps:**
1. Create PTY with 80x24 size
2. Spawn shell (`/bin/bash` on Unix, `cmd.exe` on Windows)
3. Wait for shell prompt
4. Verify process created with valid PID
5. Cleanup: terminate shell, close PTY

**Expected Results:**
- PTY created successfully (Unix: openpty() returns 0, Windows: ConPTY handle valid)
- Shell process spawned (PID > 0)
- Shell prompt received within 500ms
- No resource leaks (file descriptors closed)

**Acceptance Criteria:**
- ✅ Shell spawns within 100ms
- ✅ Prompt received within 500ms
- ✅ PID valid and process running
- ✅ Cleanup succeeds (exit code 0)

**Test Code:**
```rust
#[test]
fn test_pty_creation_and_spawning() {
    let mut pty = PlatformPty::new(80, 24).unwrap();
    let start = Instant::now();
    
    #[cfg(unix)]
    pty.spawn("/bin/bash", &["-i"]).unwrap();
    
    #[cfg(windows)]
    pty.spawn("cmd.exe", &["/K"]).unwrap();
    
    assert!(start.elapsed() < Duration::from_millis(100));
    
    let mut buf = [0u8; 4096];
    let start_read = Instant::now();
    let n = pty.read_with_timeout(&mut buf, Duration::from_millis(500)).unwrap();
    assert!(n > 0, "No output from shell");
    assert!(start_read.elapsed() < Duration::from_millis(500));
    
    pty.kill().unwrap();
}
```

---

#### T1.2: PTY Bidirectional I/O

**Test ID:** `PTY-002`  
**Priority:** P0 (Critical)  
**Platforms:** All

**Objective:** Verify bidirectional I/O through PTY

**Test Steps:**
1. Spawn shell via PTY
2. Write "echo hello\\n" to PTY master
3. Read output from PTY master
4. Verify "hello" in output
5. Measure round-trip latency

**Expected Results:**
- Write succeeds (returns number of bytes written)
- Read returns output within 100ms
- Output contains "hello"
- Round-trip latency <10ms (p95)

**Acceptance Criteria:**
- ✅ Write returns correct byte count
- ✅ Read succeeds within 100ms
- ✅ Output contains expected text
- ✅ Latency p95 <10ms

**Test Code:**
```rust
#[test]
fn test_pty_bidirectional_io() {
    let mut pty = PlatformPty::new(80, 24).unwrap();
    pty.spawn_shell().unwrap();
    
    let cmd = b"echo hello\\n";
    let start = Instant::now();
    let written = pty.write(cmd).unwrap();
    assert_eq!(written, cmd.len());
    
    let mut buf = [0u8; 4096];
    let n = pty.read_with_timeout(&mut buf, Duration::from_millis(100)).unwrap();
    let output = String::from_utf8_lossy(&buf[..n]);
    
    assert!(output.contains("hello"), "Output: {}", output);
    assert!(start.elapsed() < Duration::from_millis(10));
}
```

---

#### T1.3: PTY Window Resize (SIGWINCH)

**Test ID:** `PTY-003`  
**Priority:** P1 (High)  
**Platforms:** All

**Objective:** Verify terminal window resize propagates to child process

**Test Steps:**
1. Spawn shell with 80x24 size
2. Resize to 120x30
3. Verify `$COLUMNS` and `$LINES` updated in shell
4. Resize to 100x40
5. Verify again

**Expected Results:**
- Unix: `ioctl(TIOCSWINSZ)` succeeds, SIGWINCH delivered
- Windows: `SetConsoleScreenBufferSize()` succeeds
- Shell environment variables update within 50ms

**Acceptance Criteria:**
- ✅ Resize call succeeds
- ✅ SIGWINCH delivered (Unix)
- ✅ $COLUMNS and $LINES correct
- ✅ Propagation <50ms

**Test Code:**
```rust
#[test]
#[cfg(unix)]
fn test_pty_resize_unix() {
    let mut pty = PlatformPty::new(80, 24).unwrap();
    pty.spawn("/bin/bash", &["-i"]).unwrap();
    
    pty.resize(120, 30).unwrap();
    thread::sleep(Duration::from_millis(50));
    
    pty.write(b"echo $COLUMNS $LINES\\n").unwrap();
    let mut buf = [0u8; 4096];
    let n = pty.read_with_timeout(&mut buf, Duration::from_millis(100)).unwrap();
    let output = String::from_utf8_lossy(&buf[..n]);
    
    assert!(output.contains("120"), "COLUMNS not updated");
    assert!(output.contains("30"), "LINES not updated");
}
```

---

### 2.2 PTY Test Summary

| Test ID | Test Name | Priority | Automation | Platforms |
|---------|-----------|----------|------------|-----------|
| PTY-001 | Creation and Spawning | P0 | ✅ | All |
| PTY-002 | Bidirectional I/O | P0 | ✅ | All |
| PTY-003 | Window Resize | P1 | ✅ | All |
| PTY-004 | Environment Variables | P1 | ✅ | All |
| PTY-005 | Signal Handling (Ctrl+C) | P1 | ✅ | Unix |
| PTY-006 | Error Handling (Broken Pipe) | P2 | ✅ | All |
| PTY-007 | UTF-8 Encoding | P1 | ✅ | All |
| PTY-008 | Flow Control | P2 | ✅ | All |
| PTY-009 | Termios Settings | P2 | ✅ | Unix |
| PTY-010 | Process Groups | P2 | ✅ | Unix |
| PTY-011 | Concurrent I/O | P2 | ✅ | All |
| PTY-012 | Large Buffer Handling | P2 | ✅ | All |
| PTY-013 | Non-Blocking I/O | P2 | ✅ | All |
| PTY-014 | Cleanup on Shutdown | P1 | ✅ | All |
| PTY-015 | Recovery After Failure | P2 | ✅ | All |

**Total:** 15 tests × 6 platforms = 90 tests

---

## 3. Service Management Test Specifications

### 3.1 Test Suite: Service Lifecycle

**File:** `tests/integration/service/lifecycle.rs`  
**Tests:** 12 tests × 6 platforms = 70 tests  
**Automation:** ✅ 100% automated

---

#### T2.1: Service Installation

**Test ID:** `SVC-001`  
**Priority:** P0 (Critical)  
**Platforms:** All

**Objective:** Verify daemon installs as system service

**Test Steps:**
1. Install package (dpkg, rpm, brew install, sc create)
2. Verify service file created
3. Verify service registered with system
4. Check service status (should be inactive/loaded)

**Expected Results:**
- Service file created:
  - Linux: `/etc/systemd/system/monoterminal.service`
  - macOS: `~/Library/LaunchAgents/com.monoterminal.daemon.plist`
  - Windows: Service registered in SCM
- Service status: inactive/loaded (not running yet)

**Acceptance Criteria:**
- ✅ Service file exists at correct path
- ✅ Service registered with system
- ✅ Service status correct (inactive/loaded)

**Test Code:**
```bash
#!/bin/bash
# tests/integration/service/test_installation.sh

# Ubuntu
sudo dpkg -i monoterminal_0.1.0_amd64.deb
systemctl list-unit-files | grep monoterminal
test -f /etc/systemd/system/monoterminal.service
systemctl status monoterminal | grep "loaded"
```

---

#### T2.2: Service Start

**Test ID:** `SVC-002`  
**Priority:** P0 (Critical)  
**Platforms:** All

**Objective:** Verify daemon starts as service

**Test Steps:**
1. Start service (systemctl start, launchctl start, sc start)
2. Wait 2 seconds
3. Check service status (should be active/running)
4. Verify PID file created
5. Verify process running

**Expected Results:**
- Linux: systemd reports "active (running)", sd_notify("READY=1") sent
- macOS: launchd reports service running
- Windows: SERVICE_RUNNING state
- PID file exists: `/var/run/monoterminal.pid` (Linux), `~/.monoterminal/daemon.pid` (macOS)

**Acceptance Criteria:**
- ✅ Service starts within 2s
- ✅ Service status "active/running"
- ✅ PID file created with valid PID
- ✅ Process actually running (kill -0 $PID succeeds)

**Test Code:**
```bash
#!/bin/bash
# tests/integration/service/test_start.sh

sudo systemctl start monoterminal
sleep 2
systemctl status monoterminal | grep "active (running)"
test -f /var/run/monoterminal.pid
PID=$(cat /var/run/monoterminal.pid)
kill -0 $PID  # Check if process exists
```

---

#### T2.3: Service Socket Activation (Linux/macOS)

**Test ID:** `SVC-003`  
**Priority:** P1 (High)  
**Platforms:** Linux, macOS only

**Objective:** Verify daemon auto-starts on first connection

**Test Steps:**
1. Stop daemon if running
2. Ensure socket unit enabled (systemd) or socket loaded (launchd)
3. Connect to Unix domain socket
4. Verify daemon auto-starts
5. Verify connection succeeds

**Expected Results:**
- Daemon not running before connection
- Daemon starts within 1s of connection attempt
- Connection succeeds
- No manual start needed

**Acceptance Criteria:**
- ✅ Daemon auto-starts within 1s
- ✅ Connection succeeds
- ✅ Socket activation works without manual systemctl start

**Test Code:**
```bash
#!/bin/bash
# tests/integration/service/test_socket_activation.sh

sudo systemctl stop monoterminal
sudo systemctl start monoterminal.socket
! pgrep -x monoterminal  # Verify not running

# Connect to socket (triggers activation)
nc -U /var/run/monoterminal.sock &
sleep 1

pgrep -x monoterminal  # Should be running now
```

---

### 3.2 Service Test Summary

| Test ID | Test Name | Priority | Automation | Platforms |
|---------|-----------|----------|------------|-----------|
| SVC-001 | Service Installation | P0 | ✅ | All |
| SVC-002 | Service Start | P0 | ✅ | All |
| SVC-003 | Socket Activation | P1 | ✅ | Linux, macOS |
| SVC-004 | Service Stop (Graceful) | P0 | ✅ | All |
| SVC-005 | Service Restart (Preserve Sessions) | P0 | ✅ | All |
| SVC-006 | Auto-Restart on Crash | P1 | ✅ | All |
| SVC-007 | Logging to System Journal | P1 | ✅ | All |
| SVC-008 | Run as Correct User | P1 | ✅ | All |
| SVC-009 | Resource Limits | P2 | ✅ | Linux |
| SVC-010 | Service Uninstallation | P1 | ✅ | All |
| SVC-011 | Service Upgrade | P1 | ✅ | All |
| SVC-012 | Service Status Reporting | P2 | ✅ | All |

**Total:** 12 tests × 6 platforms (minus 2 tests × 1 platform for Windows-only socket) = 70 tests

---

## 4. Rendering Test Specifications

### 4.1 Test Suite: Performance and Visual Parity

**File:** `tests/integration/rendering/performance.rs`  
**Tests:** 20 tests × 6 platforms = 120 tests  
**Automation:** ✅ 95% automated (5% manual visual inspection)

---

#### T3.1: 60 FPS Rendering (SRS §1.3 Target)

**Test ID:** `RND-001`  
**Priority:** P0 (Critical)  
**Platforms:** All

**Objective:** Verify sustained 60 FPS rendering under load

**Test Steps:**
1. Start daemon with local UI
2. Create terminal session
3. Run high-throughput command (`cat large_file.txt`)
4. Measure FPS over 60 seconds
5. Calculate mean, p50, p95, p99

**Expected Results:**
- Mean FPS ≥60
- p95 FPS ≥55
- p99 FPS ≥50
- No frame drops >100ms

**Acceptance Criteria:**
- ✅ Mean ≥60 FPS
- ✅ p95 ≥55 FPS
- ✅ p99 ≥50 FPS
- ✅ Sustained for 60 seconds

**Test Code:**
```rust
#[test]
fn test_rendering_60fps() {
    let mut renderer = Renderer::new().unwrap();
    let mut pty = PlatformPty::new(80, 24).unwrap();
    pty.spawn_shell().unwrap();
    
    // High-throughput command
    pty.write(b"cat large_file.txt\\n").unwrap();
    
    let mut frame_times = Vec::new();
    let start = Instant::now();
    
    while start.elapsed() < Duration::from_secs(60) {
        let frame_start = Instant::now();
        renderer.render_frame(&pty).unwrap();
        frame_times.push(frame_start.elapsed());
    }
    
    let fps: Vec<f64> = frame_times.iter()
        .map(|t| 1000.0 / t.as_millis() as f64)
        .collect();
    
    let mean = fps.iter().sum::<f64>() / fps.len() as f64;
    assert!(mean >= 60.0, "Mean FPS: {}", mean);
    
    let p95 = percentile(&fps, 0.95);
    assert!(p95 >= 55.0, "p95 FPS: {}", p95);
}
```

---

#### T3.2: Visual Parity (Cross-Platform)

**Test ID:** `RND-002`  
**Priority:** P1 (High)  
**Platforms:** All

**Objective:** Verify identical rendering across platforms

**Test Steps:**
1. Render identical terminal content on all platforms
2. Capture screenshots (1920x1080, 24-bit PNG)
3. Compare pixel-by-pixel
4. Calculate difference percentage

**Expected Results:**
- <1% pixel difference (accounting for font hinting)
- Same font: JetBrains Mono
- Same colors: ANSI color scheme
- Same cursor position

**Acceptance Criteria:**
- ✅ Pixel difference <1%
- ✅ Font matches across platforms
- ✅ Colors match (within 2% tolerance)

**Test Code:**
```rust
#[test]
fn test_visual_parity() {
    let content = "Hello World\\n$ ls\\nfile1.txt\\nfile2.txt\\n";
    
    let screenshot_linux = render_and_capture(content, Platform::Linux);
    let screenshot_macos = render_and_capture(content, Platform::MacOS);
    let screenshot_windows = render_and_capture(content, Platform::Windows);
    
    let diff_linux_mac = pixel_difference(&screenshot_linux, &screenshot_macos);
    let diff_linux_win = pixel_difference(&screenshot_linux, &screenshot_windows);
    
    assert!(diff_linux_mac < 0.01, "Linux vs macOS: {}% diff", diff_linux_mac * 100.0);
    assert!(diff_linux_win < 0.01, "Linux vs Windows: {}% diff", diff_linux_win * 100.0);
}
```

---

### 4.2 Rendering Test Summary

| Test ID | Test Name | Priority | Automation | Platforms |
|---------|-----------|----------|------------|-----------|
| RND-001 | 60 FPS Rendering | P0 | ✅ | All |
| RND-002 | Visual Parity | P1 | ❌ Manual | All |
| RND-003 | GPU Backend Validation | P0 | ✅ | All |
| RND-004 | HiDPI Rendering | P1 | ✅ | macOS, Windows |
| RND-005 | Color Accuracy | P1 | ✅ | All |
| RND-006 | Font Rendering | P1 | ✅ | All |
| RND-007 | Cursor Styles | P2 | ✅ | All |
| RND-008 | Selection Rendering | P2 | ✅ | All |
| RND-009 | Scrollback Performance | P1 | ✅ | All |
| RND-010 | VT Escape Sequences | P1 | ✅ | All |
| RND-011-020 | Additional 10 tests | P2 | ✅ | All |

**Total:** 20 tests × 6 platforms = 120 tests

---

## 5. Distribution Package Test Specifications

### 5.1 Test Suite: Package Installation and Upgrade

**File:** `tests/integration/distribution/packaging.sh`  
**Tests:** 8 tests × 5 distros = 40 tests  
**Automation:** ❌ 50% automated (manual VM testing required)

---

#### T4.1: .deb Package Installation

**Test ID:** `PKG-001`  
**Priority:** P0 (Critical)  
**Platforms:** Ubuntu, Debian

**Objective:** Verify .deb package installs correctly

**Test Steps:**
1. Download .deb package
2. Install: `sudo dpkg -i monoterminal_0.1.0_amd64.deb`
3. Verify binary installed at `/usr/bin/monoterminal`
4. Verify service file at `/etc/systemd/system/monoterminal.service`
5. Verify dependencies satisfied

**Expected Results:**
- Package installs without errors
- Binary executable with correct permissions (0755)
- Service file valid
- Dependencies auto-installed

**Acceptance Criteria:**
- ✅ Installation succeeds (exit code 0)
- ✅ Binary exists and is executable
- ✅ Service file valid
- ✅ No dependency errors

---

### 5.2 Distribution Test Summary

| Test ID | Test Name | Priority | Automation | Platforms |
|---------|-----------|----------|------------|-----------|
| PKG-001 | .deb Installation | P0 | ❌ Manual | Ubuntu, Debian |
| PKG-002 | .rpm Installation | P0 | ❌ Manual | Fedora |
| PKG-003 | Homebrew Installation | P0 | ❌ Manual | macOS |
| PKG-004 | Package Upgrade | P1 | ✅ | All |
| PKG-005 | Package Removal | P1 | ✅ | All |
| PKG-006 | Dependency Resolution | P1 | ❌ Manual | All |
| PKG-007 | GPG Signature Verification | P2 | ✅ | Linux |
| PKG-008 | Tarball Installation | P2 | ✅ | All |

**Total:** 8 tests × 5 distros (Ubuntu, Debian, Fedora, macOS Intel, macOS M1) = 40 tests

---

## 6. Cross-Platform E2E Test Specifications

### 6.1 Test Suite: Cross-Platform Integration

**File:** `tests/e2e/cross_platform/integration.rs`  
**Tests:** 25 E2E tests  
**Automation:** ✅ 90% automated

---

#### T5.1: Windows Client → Linux Server

**Test ID:** `E2E-001`  
**Priority:** P0 (Critical)  
**Platforms:** Windows (client) + Linux (server)

**Objective:** Verify cross-platform client-server communication

**Test Steps:**
1. Start Linux daemon
2. Start Windows web browser (Chrome)
3. Connect to https://linux-host:5000
4. Create session, run commands
5. Verify output correct

**Expected Results:**
- TLS handshake succeeds (Ed25519 cert)
- WebSocket connection established
- Protobuf messages decoded correctly
- Session lifecycle works

**Acceptance Criteria:**
- ✅ Connection succeeds
- ✅ Session created
- ✅ Commands execute correctly
- ✅ Output received within 100ms

---

### 6.2 E2E Test Summary

| Test ID | Test Name | Priority | Automation | Platforms |
|---------|-----------|----------|------------|-----------|
| E2E-001 | Windows → Linux | P0 | ✅ | Win + Lin |
| E2E-002 | macOS → Windows | P0 | ✅ | Mac + Win |
| E2E-003 | Linux → macOS | P0 | ✅ | Lin + Mac |
| E2E-004 | Multi-Platform Session Sharing | P1 | ✅ | All |
| E2E-005 | Session Recovery Cross-Platform | P1 | ✅ | All |
| E2E-006-025 | Additional 20 E2E tests | P1-P2 | ✅ | Various |

**Total:** 25 E2E tests

---

## 7. Test Execution Procedures

### 7.1 Automated Test Execution

**Run all tests:**
```bash
cargo test --all --release
```

**Run specific suite:**
```bash
cargo test --test pty_integration
cargo test --test service_lifecycle
cargo test --test rendering_performance
```

**Platform-specific:**
```bash
cargo test --target x86_64-unknown-linux-gnu
cargo test --target x86_64-apple-darwin
cargo test --target x86_64-pc-windows-msvc
```

### 7.2 Manual Test Execution

**Distribution testing:**
```bash
# Ubuntu
cd tests/integration/distribution/
./test_deb_installation.sh

# macOS
./test_homebrew_installation.sh
```

**Visual parity testing:**
1. Render identical content on all platforms
2. Capture screenshots to `tests/screenshots/`
3. Run pixel comparison tool: `cargo run --bin compare_screenshots`

---

## 8. Test Result Reporting

### 8.1 CI Test Results

**GitHub Actions Matrix:**
- 6 platforms × 6 test suites = 36 CI jobs
- Each job uploads test results as artifacts
- JUnit XML format for test result parsing

**Example:**
```yaml
- name: Run tests
  run: cargo test --all --release -- --format junit > test-results.xml
- name: Upload results
  uses: actions/upload-artifact@v3
  with:
    name: test-results-${{ matrix.os }}
    path: test-results.xml
```

### 8.2 Coverage Reporting

**tarpaulin (Linux/macOS):**
```bash
cargo tarpaulin --out Html --output-dir coverage/
```

**Manual projection (fallback):**
- See task-47 methodology
- Count passing tests, project coverage

---

## 9. Success Criteria

**Phase 3 test suite is COMPLETE when:**

1. ✅ All 405 tests implemented
2. ✅ ≥95% pass rate on all platforms
3. ✅ 90% automation achieved
4. ✅ CI pipeline green (all platforms)
5. ✅ Coverage ≥80% (Phase 3 gate target)
6. ✅ Zero P0 blockers
7. ✅ <5 P1 bugs total

**Escalation:** If <95% pass rate, escalate to eng-director

---

**Document Status:** DRAFT  
**Next Review:** After test implementation  
**Approval Required:** eng-director + qa-lead

---

**End of Test Suite Specifications**
