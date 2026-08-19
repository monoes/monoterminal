# MONOTERMINAL Phase 1 Test Strategy & Coverage Framework

**Version:** 1.0  
**Date:** August 14, 2026  
**Phase:** Phase 1 (Windows + Web MVP)  
**Owner:** QA Lead  

---

## 1. Executive Summary

This document defines the test strategy, coverage framework, and quality gates for MONOTERMINAL Phase 1 (Windows master + Web client MVP). It implements the testing pyramid specified in SRS §6.1 with a 70% coverage target and establishes the foundation for scaling to 75% (Phase 2) and 80% (Phase 3+).

**Phase 1 Quality Targets:**
- **Coverage**: 70% minimum (cargo-tarpaulin + codecov)
- **Stability**: Zero crashes in 24-hour soak test
- **Platform**: Windows 10 1809+ (ConPTY validation)
- **CI**: All tests pass on windows-2022 × (Rust stable + beta)

---

## 2. Testing Pyramid (SRS §6.1)

### 2.1 Distribution

```
        /\
       /  \        E2E (5%): Full system tests
      /────\       ~50 tests, ~2-5 min runtime
     /      \      
    /────────\     Integration (25%): Multi-component tests
   /          \    ~200 tests, ~30-60s runtime
  /────────────\   
 /              \  Unit (70%): Isolated component tests
/────────────────\ ~550 tests, <10s runtime
```

**Total Test Count Target (Phase 1):** ~800 tests

### 2.2 Test Types & Tooling

| Type | Tool | Count | Runtime | Coverage | Purpose |
|------|------|-------|---------|----------|---------|
| **Unit** | `cargo test` | ~550 | <10s | 70% | PTY logic, protocol parsing, state management |
| **Integration** | `cargo test --test` | ~200 | 30-60s | 25% | Client-server handshake, session lifecycle |
| **E2E** | pytest + Playwright | ~50 | 2-5min | 5% | Full workflow (attach, type, receive, detach) |
| **Property** | proptest | ~30 | 10-30s | Fuzz coverage | Protocol parser, state transitions |
| **Snapshot** | insta | ~20 | <5s | VT rendering | VT sequence golden files |

---

## 3. Phase 1 Test Scope

### 3.1 In-Scope Components

#### 3.1.1 Master Daemon (`crates/master`)

**Unit Tests:**
- ✅ ConPTY session creation (`pty/windows.rs`)
- ✅ Session state management (`session/manager.rs`)
- ✅ Client connection handling (`network/connection.rs`)
- ✅ Authentication flow (Ed25519 + JWT) (`auth/mod.rs`)
- ✅ Configuration parsing (`config/mod.rs`)

**Integration Tests:**
- ✅ Full ConPTY lifecycle (create → attach → resize → terminate)
- ✅ WebSocket + TLS handshake
- ✅ Multi-client attach to same session
- ✅ Session persistence across daemon restart (SQLite)

**Example Test Structure:**
```rust
// crates/master/tests/session_lifecycle.rs
#[tokio::test]
async fn test_conpty_session_lifecycle() {
    let config = TestConfig::default();
    let daemon = MasterDaemon::new(config).await.unwrap();
    
    // Create session
    let session_id = daemon.create_session("cmd.exe", 80, 24).await.unwrap();
    
    // Verify PTY is running
    assert!(daemon.get_session(&session_id).await.is_some());
    
    // Send input
    daemon.send_input(&session_id, b"echo hello\r\n").await.unwrap();
    
    // Receive output
    let output = daemon.read_output(&session_id, Duration::from_secs(1)).await.unwrap();
    assert!(output.contains("hello"));
    
    // Cleanup
    daemon.terminate_session(&session_id).await.unwrap();
}
```

#### 3.1.2 Protocol (`crates/protocol`)

**Unit Tests:**
- ✅ Protobuf message serialization/deserialization
- ✅ Compression (zstd) for >4KB chunks
- ✅ Message envelope validation

**Property Tests (proptest):**
```rust
// crates/protocol/tests/proto_fuzzing.rs
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_envelope_roundtrip(
        seq in any::<u64>(),
        data in prop::collection::vec(any::<u8>(), 0..8192)
    ) {
        let envelope = Envelope {
            sequence_number: seq,
            message: Some(Message::OutputData(OutputData { data, ..Default::default() })),
        };
        
        let encoded = encode_envelope(&envelope).unwrap();
        let decoded = decode_envelope(&encoded).unwrap();
        
        prop_assert_eq!(envelope, decoded);
    }
}
```

#### 3.1.3 Monomind Bridge (`crates/monomind-bridge`)

**Unit Tests:**
- ✅ `.monomind/` directory detection
- ✅ Session-scoped suggestion logic
- ✅ Health check execution

**Integration Tests:**
- ✅ End-to-end monomind detection workflow
- ✅ Embedded dashboard API responses

#### 3.1.4 Web Client (`web/`)

**Unit Tests (Vitest):**
- ✅ xterm.js integration
- ✅ WebSocket message handling
- ✅ Session state management (React hooks)

**E2E Tests (Playwright):**
```typescript
// web/e2e/session-attach.spec.ts
test('attach to existing session and receive output', async ({ page }) => {
  await page.goto('http://localhost:5173');
  
  // Mock WebSocket connection
  await page.evaluate(() => {
    window.mockWsConnection('session-123');
  });
  
  // Type command
  await page.locator('.xterm').click();
  await page.keyboard.type('echo test\n');
  
  // Verify output appears
  await expect(page.locator('.xterm')).toContainText('test');
});
```

### 3.2 Out-of-Scope (Phase 1)

- ❌ P2P/WebRTC tests (Phase 2)
- ❌ Multi-session management tests (Phase 2)
- ❌ SQLite persistence tests (Phase 2)
- ❌ Collaboration features (Phase 2)
- ❌ Linux/macOS tests (Phase 3)

---

## 4. Coverage Framework

### 4.1 Cargo Tarpaulin Setup

**Installation:**
```powershell
cargo install cargo-tarpaulin
```

**Configuration (`.tarpaulin.toml`):**
```toml
[report]
out = ["Html", "Xml", "Lcov"]

[run]
timeout = "300s"
follow-exec = true
post-test-delay = "1s"

[html]
output-dir = "coverage/html"

[xml]
output-dir = "coverage"

[coverage]
exclude = [
    "crates/protocol/src/generated/*",  # Generated protobuf code
    "*/tests/*",                         # Test code itself
    "*/benches/*",                       # Benchmark code
]

[windows]
# Windows-specific exclusions
exclude = []
```

**Run Coverage:**
```powershell
# Full coverage report
cargo tarpaulin --workspace --all-features --out Html --out Xml

# Per-crate coverage
cargo tarpaulin -p monoterminal-master --out Html

# CI mode (codecov upload)
cargo tarpaulin --workspace --all-features --out Xml
```

### 4.2 Codecov Integration

**`.codecov.yml`:**
```yaml
coverage:
  status:
    project:
      default:
        target: 70%           # Phase 1 minimum
        threshold: 1%         # Allow 1% deviation
        informational: false  # Block PR if below target
    patch:
      default:
        target: 80%           # New code should be well-tested
        threshold: 5%

comment:
  layout: "header, diff, files"
  require_changes: true

ignore:
  - "crates/protocol/src/generated/**"
  - "**/tests/**"
  - "**/benches/**"
```

### 4.3 Coverage Enforcement in CI

**GitHub Actions workflow (see §5.2) enforces:**
1. ✅ Minimum 70% total coverage (fail PR if below)
2. ✅ New code ≥80% coverage (patch coverage)
3. ✅ Coverage reports on every PR (Codecov comment)
4. ✅ Coverage badge in README

---

## 5. CI/CD Test Matrix

### 5.1 Phase 1 Matrix (Windows Only)

```yaml
strategy:
  matrix:
    os: [windows-2022]
    arch: [x86_64]
    rust: [stable, beta]
    features: [default, all-features]
  fail-fast: false
```

**Justification:** Phase 1 scope is Windows-first (SRS §7.1). Linux/macOS added in Phase 3.

### 5.2 GitHub Actions Workflow

**`.github/workflows/test.yml`:**
```yaml
name: Test Suite

on:
  pull_request:
  push:
    branches: [main]

env:
  RUST_BACKTRACE: 1
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test (${{ matrix.rust }} on ${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [windows-2022]
        rust: [stable, beta]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      
      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2
      
      - name: Run unit tests
        run: cargo test --workspace --lib --bins
      
      - name: Run integration tests
        run: cargo test --workspace --test '*'
      
      - name: Run doctests
        run: cargo test --workspace --doc
      
      - name: Verify ConPTY availability
        run: |
          $version = [System.Environment]::OSVersion.Version
          if ($version.Build -lt 17763) {
            Write-Error "ConPTY requires Windows 10 1809+ (build 17763+), got $version"
            exit 1
          }
          Write-Host "ConPTY available: Windows build $($version.Build)"

  coverage:
    name: Coverage
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      
      - name: Generate coverage
        run: cargo tarpaulin --workspace --all-features --out Xml --timeout 300
      
      - name: Upload to codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
          fail_ci_if_error: true
          flags: phase1-windows

  lint:
    name: Lint
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings

  e2e:
    name: E2E Tests
    runs-on: windows-2022
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      
      - name: Install dependencies
        working-directory: ./web
        run: npm ci
      
      - name: Install Playwright
        working-directory: ./web
        run: npx playwright install --with-deps chromium
      
      - name: Build web client
        working-directory: ./web
        run: npm run build
      
      - name: Run E2E tests
        working-directory: ./web
        run: npm run test:e2e
      
      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: playwright-report
          path: web/playwright-report/
```

---

## 6. Test Utilities & Fixtures

### 6.1 Mock PTY (Unit Tests)

**`crates/master/tests/common/mock_pty.rs`:**
```rust
/// Mock PTY for unit testing without real ConPTY
pub struct MockPty {
    input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    output_tx: mpsc::UnboundedSender<Vec<u8>>,
    dimensions: (u16, u16),
}

impl MockPty {
    pub fn new() -> (Self, MockPtyHandle) {
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        
        let pty = MockPty {
            input_rx,
            output_tx,
            dimensions: (80, 24),
        };
        
        let handle = MockPtyHandle {
            input_tx,
            output_rx,
        };
        
        (pty, handle)
    }
    
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Simulate PTY read
        if let Some(data) = self.input_rx.recv().await {
            let len = buf.len().min(data.len());
            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }
    
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Simulate PTY write
        self.output_tx.send(buf.to_vec()).unwrap();
        Ok(buf.len())
    }
    
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.dimensions = (rows, cols);
    }
}

pub struct MockPtyHandle {
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl MockPtyHandle {
    pub fn send_input(&self, data: &[u8]) {
        self.input_tx.send(data.to_vec()).unwrap();
    }
    
    pub async fn recv_output(&mut self) -> Option<Vec<u8>> {
        self.output_rx.recv().await
    }
}
```

### 6.2 WebSocket Client Simulator

**`crates/master/tests/common/ws_client.rs`:**
```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct TestWsClient {
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TestWsClient {
    pub async fn connect(url: &str) -> Result<Self> {
        let (stream, _) = connect_async(url).await?;
        Ok(Self { stream })
    }
    
    pub async fn send_attach_request(&mut self, session_id: &str, jwt: &str) -> Result<()> {
        let envelope = create_attach_request(session_id, jwt);
        let bytes = encode_envelope(&envelope)?;
        self.stream.send(Message::Binary(bytes)).await?;
        Ok(())
    }
    
    pub async fn recv_attach_response(&mut self) -> Result<AttachResponse> {
        let msg = self.stream.next().await.unwrap()?;
        let envelope = decode_envelope(msg.into_data())?;
        // Extract AttachResponse from envelope
        Ok(extract_attach_response(envelope)?)
    }
}
```

### 6.3 Common Test Fixtures

**`crates/master/tests/common/fixtures.rs`:**
```rust
pub struct TestContext {
    pub temp_dir: TempDir,
    pub config: Config,
    pub daemon: Option<MasterDaemon>,
}

impl TestContext {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let config = Config {
            database_path: temp_dir.path().join("test.db"),
            listen_address: "127.0.0.1:0".parse().unwrap(),  // Random port
            ..Default::default()
        };
        
        Ok(Self {
            temp_dir,
            config,
            daemon: None,
        })
    }
    
    pub async fn start_daemon(&mut self) -> Result<()> {
        self.daemon = Some(MasterDaemon::new(self.config.clone()).await?);
        Ok(())
    }
    
    pub fn daemon(&self) -> &MasterDaemon {
        self.daemon.as_ref().expect("Daemon not started")
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Cleanup
        if let Some(daemon) = self.daemon.take() {
            let _ = daemon.shutdown();
        }
    }
}

// Shared test data generators - implementation deferred to test modules
pub fn sample_jwt() -> String {
    "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9...".to_string()
}
```

---

## 7. Acceptance Criteria (SRS §7.1)

### 7.1 Coverage Gates

| Metric | Target | Enforcement |
|--------|--------|-------------|
| **Total Coverage** | ≥70% | CI fails PR if below |
| **Patch Coverage** | ≥80% | Codecov comment warns if below |
| **Core Modules** | ≥85% | Manual review for `pty/`, `session/`, `auth/` |

### 7.2 Soak Test (24-hour stability)

**Soak Test Script (`tests/soak/24h-stability.rs`):**
```rust
#[tokio::test]
#[ignore]  // Run manually via `cargo test --ignored soak`
async fn soak_test_24h_no_crashes() {
    let ctx = TestContext::new().await.unwrap();
    ctx.start_daemon().await.unwrap();
    
    let start = Instant::now();
    let duration = Duration::from_secs(24 * 60 * 60);  // 24 hours
    
    let mut sessions = vec![];
    
    while start.elapsed() < duration {
        // Create 10 sessions
        for _ in 0..10 {
            let session = ctx.daemon().create_session("cmd.exe", 80, 24).await.unwrap();
            sessions.push(session);
        }
        
        // Simulate activity
        for session_id in &sessions {
            ctx.daemon().send_input(session_id, b"echo test\r\n").await.ok();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Terminate half
        for session_id in sessions.drain(..5) {
            ctx.daemon().terminate_session(&session_id).await.ok();
        }
        
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    
    // No panics = success
    assert!(ctx.daemon().is_running());
}
```

**Run Command:**
```powershell
# Run 24-hour soak test
cargo test --ignored soak_test_24h_no_crashes -- --nocapture
```

### 7.3 ConPTY Validation

**ConPTY Feature Tests (`crates/master/tests/conpty_validation.rs`):**
```rust
#[cfg(target_os = "windows")]
mod windows_tests {
    #[test]
    fn test_conpty_available() {
        use windows::Win32::System::Console::CreatePseudoConsole;
        // Verify ConPTY API is available (Windows 10 1809+)
        assert!(CreatePseudoConsole::is_available());
    }
    
    #[tokio::test]
    async fn test_conpty_resize() {
        let pty = ConPty::new(80, 24).await.unwrap();
        pty.resize(100, 30).await.unwrap();
        let dimensions = pty.get_dimensions();
        assert_eq!(dimensions, (100, 30));
    }
    
    #[tokio::test]
    async fn test_conpty_utf8_output() {
        let pty = ConPty::spawn("cmd.exe", 80, 24).await.unwrap();
        pty.write(b"echo Hello World\r\n").await.unwrap();
        let output = pty.read_timeout(Duration::from_secs(1)).await.unwrap();
        assert!(String::from_utf8_lossy(&output).contains("Hello World"));
    }
}
```

---

## 8. Property Testing (Fuzzing)

### 8.1 Protocol Fuzzing

**`crates/protocol/tests/fuzz.rs`:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn fuzz_envelope_decode(bytes in prop::collection::vec(any::<u8>(), 0..16384)) {
        // Should never panic, even on malformed input
        let _ = decode_envelope(&bytes);
    }
    
    #[test]
    fn fuzz_compression_roundtrip(
        data in prop::collection::vec(any::<u8>(), 0..65536)
    ) {
        let compressed = compress_zstd(&data).unwrap();
        let decompressed = decompress_zstd(&compressed).unwrap();
        prop_assert_eq!(data, decompressed);
    }
}
```

### 8.2 State Machine Fuzzing

**`crates/master/tests/fuzz_session_state.rs`:**
```rust
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum SessionAction {
    Create,
    Attach,
    Detach,
    SendInput(Vec<u8>),
    Resize(u16, u16),
    Terminate,
}

fn session_action_strategy() -> impl Strategy<Value = SessionAction> {
    prop_oneof![
        Just(SessionAction::Create),
        Just(SessionAction::Attach),
        Just(SessionAction::Detach),
        any::<Vec<u8>>().prop_map(SessionAction::SendInput),
        (1u16..1000, 1u16..1000).prop_map(|(r, c)| SessionAction::Resize(r, c)),
        Just(SessionAction::Terminate),
    ]
}

proptest! {
    #[test]
    fn fuzz_session_state_transitions(
        actions in prop::collection::vec(session_action_strategy(), 0..100)
    ) {
        // State machine should never panic regardless of action sequence
        let mut session = SessionStateMachine::new();
        for action in actions {
            let _ = session.apply(action);  // Should not panic
        }
    }
}
```

---

## 9. Snapshot Testing (VT Sequences)

### 9.1 Insta Setup

**`crates/master/tests/vt_rendering.rs`:**
```rust
use insta::assert_snapshot;

#[test]
fn test_ansi_color_rendering() {
    let input = "\x1b[31mRed text\x1b[0m";
    let rendered = render_vt_sequence(input);
    assert_snapshot!(rendered);
}

#[test]
fn test_cursor_movement() {
    let input = "\x1b[2J\x1b[H\x1b[5;10HHello";
    let rendered = render_vt_sequence(input);
    assert_snapshot!(rendered);
}

#[test]
fn test_unicode_width() {
    let input = "Hello 世界 🚀";
    let rendered = render_vt_sequence(input);
    assert_snapshot!(rendered);
}
```

**Review Snapshots:**
```powershell
# Review new snapshots
cargo insta review

# Accept all snapshots
cargo insta accept
```

---

## 10. Performance Benchmarks

### 10.1 Criterion.rs Setup

**`crates/master/benches/pty_throughput.rs`:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn benchmark_pty_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("pty-read");
    group.throughput(Throughput::Bytes(4096));
    
    group.bench_function("4KB chunks", |b| {
        b.iter(|| {
            let data = black_box(vec![0u8; 4096]);
            // Benchmark PTY read
        });
    });
    
    group.finish();
}

criterion_group!(benches, benchmark_pty_read);
criterion_main!(benches);
```

**Run Benchmarks:**
```powershell
cargo bench --bench pty_throughput
```

---

## 11. Test Organization & Naming

### 11.1 Directory Structure

```
monoterminal/
├── crates/
│   ├── master/
│   │   ├── src/
│   │   │   ├── pty/
│   │   │   │   ├── mod.rs
│   │   │   │   └── windows.rs
│   │   │   ├── session/
│   │   │   └── auth/
│   │   ├── tests/
│   │   │   ├── common/          # Shared test utilities
│   │   │   │   ├── mod.rs
│   │   │   │   ├── mock_pty.rs
│   │   │   │   └── fixtures.rs
│   │   │   ├── integration/     # Integration tests
│   │   │   │   ├── session_lifecycle.rs
│   │   │   │   └── auth_flow.rs
│   │   │   └── soak/            # Long-running tests
│   │   │       └── 24h_stability.rs
│   │   └── benches/             # Performance benchmarks
│   │       └── pty_throughput.rs
│   ├── protocol/
│   │   ├── tests/
│   │   │   ├── proto_roundtrip.rs
│   │   │   └── fuzz.rs
│   └── monomind-bridge/
│       └── tests/
│           └── detection.rs
└── web/
    ├── src/
    │   └── __tests__/           # Vitest unit tests
    └── e2e/                     # Playwright E2E tests
        └── session-attach.spec.ts
```

### 11.2 Naming Conventions

| Pattern | Example | Type |
|---------|---------|------|
| `test_<feature>` | `test_conpty_creation()` | Unit |
| `test_<component>_<scenario>` | `test_session_attach_success()` | Integration |
| `fuzz_<target>` | `fuzz_envelope_decode()` | Property |
| `bench_<operation>` | `bench_pty_read_4kb()` | Benchmark |

---

## 12. Continuous Improvement

### 12.1 Coverage Ratcheting

**Prevent Coverage Regression:**
```yaml
# .github/workflows/coverage-ratchet.yml
- name: Check coverage doesn't decrease
  run: |
    $current = (Get-Content coverage.json | ConvertFrom-Json).coverage
    $baseline = (Get-Content baseline-coverage.json | ConvertFrom-Json).coverage
    if ($current -lt $baseline - 1) {
      Write-Error "Coverage decreased from $baseline% to $current%"
      exit 1
    }
```

### 12.2 Test Metrics Dashboard

**Tracked Metrics:**
- Total test count (trend over time)
- Coverage percentage per crate
- Test execution time (watch for slowdowns)
- Flaky test rate (<1% target)
- Soak test success rate (100% required)

---

## 13. Dependency on Task-1 (Architecture Review)

**Current Status:** Task-1 is running (principal-architect)

**Unblocking Path:**
1. ✅ **Immediate (can start now):** 
   - Write test strategy document (this document)
   - Set up coverage framework (tarpaulin, codecov)
   - Configure CI workflows
   - Create test utility scaffolding

2. ⏳ **Blocked (waiting on architecture):**
   - Implement actual unit tests (need finalized APIs)
   - ConPTY integration tests (need PTY abstraction design)
   - Protocol tests (need final .proto schema)

**Coordination:** Once task-1 completes, qa-lead will receive architecture artifacts to implement concrete tests against.

---

## 14. Sign-off & Review

**QA Lead Approval:** _Pending_  
**Engineering Director Approval:** _Pending_  
**Principal Architect Review:** _Required after task-1 completion_

**Next Steps:**
1. Review this strategy document with eng-director
2. Set up CI infrastructure (GitHub Actions)
3. Install coverage tools locally
4. Await architecture completion (task-1)
5. Implement first unit tests for ConPTY manager

---

**Document Version:** 1.0 (2026-08-14)  
**Last Updated:** August 14, 2026  
**Next Review:** Phase 2 kickoff (Month 4)
