# Test Templates

**Quick-start templates for writing tests in MONOTERMINAL**

Copy and adapt these templates when writing new tests.

---

## Unit Test Template (Inline)

```rust
// crates/master/src/session/manager.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation_success() {
        // Arrange
        let manager = SessionManager::new();
        let config = SessionConfig {
            shell: "cmd.exe".to_string(),
            rows: 24,
            cols: 80,
        };

        // Act
        let result = manager.create_session(config);

        // Assert
        assert!(result.is_ok());
        let session_id = result.unwrap();
        assert!(manager.get_session(&session_id).is_some());
    }

    #[tokio::test]
    async fn test_async_function() {
        // Arrange
        let manager = SessionManager::new();

        // Act
        let result = manager.async_operation().await;

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    #[should_panic(expected = "invalid configuration")]
    fn test_invalid_config_panics() {
        let manager = SessionManager::new();
        let bad_config = SessionConfig::default();
        manager.create_session(bad_config);  // Should panic
    }
}
```

---

## Integration Test Template

```rust
// crates/master/tests/session_lifecycle.rs

use monoterminal_master::{MasterDaemon, Config};
use std::time::Duration;
use tokio::time::timeout;

mod common;
use common::fixtures::TestContext;

#[tokio::test]
async fn test_full_session_lifecycle() -> anyhow::Result<()> {
    // Arrange
    let mut ctx = TestContext::new().await?;
    ctx.start_daemon().await?;
    let daemon = ctx.daemon();

    // Act - Create session
    let session_id = daemon.create_session("cmd.exe", 80, 24).await?;
    
    // Assert - Session exists
    assert!(daemon.get_session(&session_id).await.is_some());

    // Act - Send input
    daemon.send_input(&session_id, b"echo hello\r\n").await?;

    // Assert - Receive output
    let output = timeout(
        Duration::from_secs(2),
        daemon.read_output(&session_id)
    ).await??;
    
    assert!(String::from_utf8_lossy(&output).contains("hello"));

    // Act - Terminate
    daemon.terminate_session(&session_id).await?;

    // Assert - Session gone
    assert!(daemon.get_session(&session_id).await.is_none());

    Ok(())
}

#[tokio::test]
async fn test_multiple_clients_same_session() -> anyhow::Result<()> {
    let mut ctx = TestContext::new().await?;
    ctx.start_daemon().await?;
    
    let session_id = ctx.daemon().create_session("cmd.exe", 80, 24).await?;
    
    // Attach client 1
    let mut client1 = common::ws_client::TestWsClient::connect(
        &format!("ws://127.0.0.1:{}", ctx.daemon_port())
    ).await?;
    client1.send_attach_request(&session_id, &common::sample_jwt()).await?;
    
    // Attach client 2
    let mut client2 = common::ws_client::TestWsClient::connect(
        &format!("ws://127.0.0.1:{}", ctx.daemon_port())
    ).await?;
    client2.send_attach_request(&session_id, &common::sample_jwt()).await?;
    
    // Both clients should receive same output
    client1.send_input(b"echo test\r\n").await?;
    
    let output1 = client1.recv_output().await?;
    let output2 = client2.recv_output().await?;
    
    assert_eq!(output1, output2);
    assert!(String::from_utf8_lossy(&output1).contains("test"));
    
    Ok(())
}
```

---

## Property Test Template (Fuzzing)

```rust
// crates/protocol/tests/fuzz.rs

use proptest::prelude::*;
use monoterminal_protocol::{Envelope, encode_envelope, decode_envelope};

proptest! {
    #[test]
    fn fuzz_envelope_decode_never_panics(
        bytes in prop::collection::vec(any::<u8>(), 0..16384)
    ) {
        // Should never panic, even on malformed input
        let _ = decode_envelope(&bytes);
    }

    #[test]
    fn fuzz_envelope_roundtrip(
        sequence_number in any::<u64>(),
        data in prop::collection::vec(any::<u8>(), 0..8192)
    ) {
        let original = Envelope {
            sequence_number,
            message: Some(Message::OutputData(OutputData {
                data: data.clone(),
                sequence: 0,
                compression: CompressionType::None,
            })),
        };

        let encoded = encode_envelope(&original).unwrap();
        let decoded = decode_envelope(&encoded).unwrap();

        prop_assert_eq!(original, decoded);
    }

    #[test]
    fn fuzz_compression_preserves_data(
        data in prop::collection::vec(any::<u8>(), 0..65536)
    ) {
        let compressed = compress_zstd(&data).unwrap();
        let decompressed = decompress_zstd(&compressed).unwrap();
        prop_assert_eq!(data, decompressed);
    }
}

// Custom strategy for generating valid session IDs
fn session_id_strategy() -> impl Strategy<Value = String> {
    "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
        .prop_map(|s| s.to_string())
}

proptest! {
    #[test]
    fn fuzz_session_id_validation(
        session_id in session_id_strategy()
    ) {
        prop_assert!(validate_session_id(&session_id).is_ok());
    }
}
```

---

## Snapshot Test Template

```rust
// crates/master/tests/vt_rendering.rs

use insta::{assert_snapshot, assert_debug_snapshot};
use monoterminal_master::vt::render;

#[test]
fn test_ansi_color_sequence() {
    let input = "\x1b[31mRed text\x1b[0m";
    let rendered = render(input);
    assert_snapshot!(rendered);
}

#[test]
fn test_complex_vt_sequence() {
    let input = "\x1b[2J\x1b[H\x1b[5;10HHello World\x1b[0m";
    let rendered = render(input);
    assert_snapshot!(rendered);
}

#[test]
fn test_unicode_rendering() {
    let input = "ASCII 世界 🚀 emoji";
    let rendered = render(input);
    assert_snapshot!(rendered);
}

#[test]
fn test_cursor_movement_debug() {
    let sequence = parse_vt_sequence("\x1b[5;10H");
    assert_debug_snapshot!(sequence);
}
```

**Review snapshots:**
```powershell
cargo insta review
cargo insta accept  # If correct
cargo insta reject  # If wrong
```

---

## Benchmark Template

```rust
// crates/master/benches/pty_throughput.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use monoterminal_master::pty::ConPty;

fn benchmark_pty_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("pty-read");
    group.throughput(Throughput::Bytes(4096));
    
    group.bench_function("4KB chunks", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let pty = ConPty::new(80, 24).await.unwrap();
            let mut buf = vec![0u8; 4096];
            pty.read(&mut buf).await.unwrap();
            black_box(buf);
        });
    });
    
    group.finish();
}

fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");
    
    let data = vec![b'a'; 8192];
    
    group.throughput(Throughput::Bytes(8192));
    group.bench_function("zstd compress 8KB", |b| {
        b.iter(|| {
            let compressed = compress_zstd(black_box(&data)).unwrap();
            black_box(compressed);
        });
    });
    
    let compressed = compress_zstd(&data).unwrap();
    group.throughput(Throughput::Bytes(compressed.len() as u64));
    group.bench_function("zstd decompress 8KB", |b| {
        b.iter(|| {
            let decompressed = decompress_zstd(black_box(&compressed)).unwrap();
            black_box(decompressed);
        });
    });
    
    group.finish();
}

criterion_group!(benches, benchmark_pty_read, benchmark_compression);
criterion_main!(benches);
```

---

## Mock Template

```rust
// crates/master/tests/common/mock_pty.rs

use tokio::sync::mpsc;
use std::io;

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
        if let Some(data) = self.input_rx.recv().await {
            let len = buf.len().min(data.len());
            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }
    
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
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

// Usage in tests:
#[tokio::test]
async fn test_with_mock_pty() {
    let (mut pty, mut handle) = MockPty::new();
    
    // Simulate input from shell
    handle.send_input(b"hello");
    
    // Read from PTY
    let mut buf = vec![0u8; 5];
    let n = pty.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"hello");
    
    // Write to PTY (goes to output)
    pty.write(b"world").await.unwrap();
    
    // Receive output
    let output = handle.recv_output().await.unwrap();
    assert_eq!(output, b"world");
}
```

---

## E2E Test Template (Playwright)

```typescript
// web/e2e/session-attach.spec.ts

import { test, expect } from '@playwright/test';

test.describe('Session Attachment', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to app
    await page.goto('http://localhost:5173');
    
    // Wait for app to load
    await page.waitForSelector('.terminal-container');
  });

  test('should attach to existing session', async ({ page }) => {
    // Arrange - Mock WebSocket connection
    await page.evaluate(() => {
      window.mockWebSocket = true;
      window.sessionId = 'test-session-123';
    });

    // Act - Click attach button
    await page.click('[data-testid="attach-session"]');
    
    // Assert - Terminal shows connected state
    await expect(page.locator('[data-testid="connection-status"]'))
      .toHaveText('Connected');
  });

  test('should send input and receive output', async ({ page }) => {
    // Arrange
    await page.evaluate(() => {
      window.mockWebSocket = true;
    });

    // Act - Type in terminal
    const terminal = page.locator('.xterm');
    await terminal.click();
    await page.keyboard.type('echo hello\n');

    // Assert - Output appears
    await expect(terminal).toContainText('hello');
  });

  test('should handle disconnection gracefully', async ({ page }) => {
    // Arrange - Connect
    await page.evaluate(() => {
      window.mockWebSocket = true;
    });
    await page.click('[data-testid="attach-session"]');

    // Act - Simulate disconnect
    await page.evaluate(() => {
      window.mockWsDisconnect();
    });

    // Assert - Shows reconnecting state
    await expect(page.locator('[data-testid="connection-status"]'))
      .toHaveText('Reconnecting...');
  });
});
```

---

## Test Fixture Template

```rust
// crates/master/tests/common/fixtures.rs

use tempfile::TempDir;
use monoterminal_master::{MasterDaemon, Config};

pub struct TestContext {
    pub temp_dir: TempDir,
    pub config: Config,
    pub daemon: Option<MasterDaemon>,
}

impl TestContext {
    pub async fn new() -> anyhow::Result<Self> {
        let temp_dir = TempDir::new()?;
        
        let config = Config {
            database_path: temp_dir.path().join("test.db"),
            listen_address: "127.0.0.1:0".parse()?,  // Random port
            tls_cert: None,  // Self-signed for tests
            ..Default::default()
        };
        
        Ok(Self {
            temp_dir,
            config,
            daemon: None,
        })
    }
    
    pub async fn start_daemon(&mut self) -> anyhow::Result<()> {
        self.daemon = Some(MasterDaemon::new(self.config.clone()).await?);
        Ok(())
    }
    
    pub fn daemon(&self) -> &MasterDaemon {
        self.daemon.as_ref().expect("Daemon not started. Call start_daemon() first")
    }
    
    pub fn daemon_port(&self) -> u16 {
        self.daemon().actual_listen_address().port()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Automatic cleanup
        if let Some(daemon) = self.daemon.take() {
            let _ = daemon.shutdown_blocking();
        }
    }
}

// Shared test data
pub fn sample_jwt() -> String {
    "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9.test".to_string()
}

pub fn sample_session_config() -> SessionConfig {
    SessionConfig {
        shell: "cmd.exe".to_string(),
        rows: 24,
        cols: 80,
    }
}
```

---

## Common Test Patterns

### Testing Error Cases
```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = parse_invalid_data();
    
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::InvalidFormat));
    assert_eq!(err.to_string(), "invalid format");
}
```

### Testing Timeouts
```rust
#[tokio::test]
async fn test_operation_times_out() {
    let result = tokio::time::timeout(
        Duration::from_millis(100),
        slow_operation()
    ).await;
    
    assert!(result.is_err());  // Timeout
}
```

### Testing Panics
```rust
#[test]
#[should_panic(expected = "division by zero")]
fn test_divide_by_zero_panics() {
    divide(10, 0);
}
```

### Testing Async Streams
```rust
#[tokio::test]
async fn test_stream_produces_values() {
    use tokio_stream::StreamExt;
    
    let mut stream = create_stream();
    let values: Vec<_> = stream.take(3).collect().await;
    
    assert_eq!(values, vec![1, 2, 3]);
}
```

---

## Tips

- **Keep tests small and focused** - One assertion per test when possible
- **Use descriptive names** - `test_session_creation_with_invalid_shell_fails()`
- **Arrange-Act-Assert** - Clear structure makes tests readable
- **Test behavior, not implementation** - Don't test private details
- **Clean up resources** - Use Drop trait or defer macros
- **Avoid flaky tests** - No hardcoded sleeps, use timeouts instead

---

For more details, see:
- `docs/test-strategy-phase1.md` - Full strategy
- `docs/testing-quick-reference.md` - Commands and troubleshooting
