# Testing Quick Reference

**Phase 1 Testing Infrastructure for MONOTERMINAL**

---

## Quick Commands

### Running Tests

```powershell
# Run all tests
cargo test --workspace

# Run unit tests only
cargo test --workspace --lib

# Run integration tests only
cargo test --workspace --test '*'

# Run tests for specific crate
cargo test -p monoterminal-master
cargo test -p monoterminal-protocol
cargo test -p monomind-bridge

# Run tests with logging
cargo test -- --nocapture

# Run specific test
cargo test test_conpty_creation

# Run property tests (fuzzing)
cargo test proptest

# Run ignored tests (e.g., soak tests)
cargo test --ignored
```

### Coverage

```powershell
# Generate HTML coverage report
cargo tarpaulin --workspace --all-features --out Html

# View coverage report
start coverage/html/index.html

# Generate XML for CI (codecov)
cargo tarpaulin --workspace --all-features --out Xml

# Coverage for specific crate
cargo tarpaulin -p monoterminal-master --out Html
```

### Benchmarks

```powershell
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench pty_throughput

# Compare benchmark results
cargo bench -- --save-baseline before
# ... make changes ...
cargo bench -- --baseline before
```

### Snapshot Tests

```powershell
# Run snapshot tests
cargo test

# Review new/changed snapshots
cargo insta review

# Accept all snapshots
cargo insta accept

# Reject all snapshots
cargo insta reject
```

### Web Client Tests

```powershell
cd web

# Unit tests (Vitest)
npm run test:unit

# E2E tests (Playwright)
npm run test:e2e

# Watch mode
npm run test:unit -- --watch

# Coverage
npm run test:unit -- --coverage
```

---

## Test Organization

### Unit Tests
**Location:** `crates/*/src/**/*.rs` (inline `#[cfg(test)]` modules)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test code
    }
}
```

### Integration Tests
**Location:** `crates/*/tests/*.rs`

```rust
// crates/master/tests/session_lifecycle.rs
#[tokio::test]
async fn test_full_session_lifecycle() {
    // Integration test code
}
```

### Property Tests
**Location:** `crates/*/tests/fuzz_*.rs`

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip(data in any::<Vec<u8>>()) {
        // Property test
    }
}
```

### Snapshot Tests
**Location:** `crates/*/tests/snapshots/`

```rust
use insta::assert_snapshot;

#[test]
fn test_rendering() {
    let output = render_something();
    assert_snapshot!(output);
}
```

---

## Coverage Targets

| Phase | Target | Current | Status |
|-------|--------|---------|--------|
| Phase 1 (Windows + Web) | 70% | TBD | 🟡 In Progress |
| Phase 2 (P2P + Storage) | 75% | - | ⏳ Planned |
| Phase 3 (Linux + macOS) | 80% | - | ⏳ Planned |

---

## CI/CD Workflows

### Pull Request Checks
- ✅ Tests pass (all platforms)
- ✅ Coverage ≥70% (enforced)
- ✅ Clippy warnings = 0
- ✅ Formatting check
- ✅ E2E tests pass

### Main Branch
- 🔄 Full test suite
- 📊 Coverage report to Codecov
- 🏃 24-hour soak test (nightly)
- 📦 Release builds

---

## Test Utilities

### Mock PTY
```rust
use common::mock_pty::MockPty;

let (pty, handle) = MockPty::new();
handle.send_input(b"echo test\n");
let output = handle.recv_output().await;
```

### Test Context
```rust
use common::fixtures::TestContext;

let mut ctx = TestContext::new().await?;
ctx.start_daemon().await?;
let daemon = ctx.daemon();
```

### WebSocket Client Simulator
```rust
use common::ws_client::TestWsClient;

let mut client = TestWsClient::connect("ws://localhost:5000").await?;
client.send_attach_request("session-id", "jwt-token").await?;
```

---

## Troubleshooting

### Coverage Too Low
1. Run coverage to see uncovered lines:
   ```powershell
   cargo tarpaulin --out Html
   start coverage/html/index.html
   ```
2. Add tests for uncovered code
3. Focus on core modules first: `pty/`, `session/`, `auth/`

### Tests Hanging
- Check for missing `.await` on async calls
- Use `tokio::time::timeout()` for flaky tests
- Ensure proper cleanup in test fixtures

### Flaky Tests
- Add retry logic for timing-sensitive tests
- Use fixed test data instead of random
- Isolate test state (no shared globals)

### ConPTY Tests Failing
- Verify Windows version ≥1809 (build 17763)
- Check ConPTY is available:
  ```rust
  assert!(windows::Win32::System::Console::CreatePseudoConsole::is_available());
  ```

---

## Best Practices

### ✅ DO
- Test one thing per test
- Use descriptive test names: `test_conpty_creation_succeeds()`
- Clean up resources in tests (use RAII/Drop)
- Mock external dependencies (PTY, network, filesystem)
- Test error paths, not just happy paths
- Write property tests for parsers and state machines

### ❌ DON'T
- Skip tests with `#[ignore]` without good reason
- Use `unwrap()` in tests (use `?` or `assert!`)
- Write tests that depend on execution order
- Hard-code timing (use `timeout()` instead of `sleep()`)
- Test implementation details (test behavior)
- Commit commented-out tests

---

## Phase 1 Acceptance Checklist

**Before declaring Phase 1 complete:**
- [ ] Total coverage ≥70%
- [ ] All tests pass on Windows 10 1809+
- [ ] ConPTY integration tests pass
- [ ] 24-hour soak test: zero crashes
- [ ] E2E web client tests pass
- [ ] CI green on main branch
- [ ] Coverage badge in README
- [ ] Test documentation complete

---

## Getting Help

- **Test Strategy:** See `docs/test-strategy-phase1.md`
- **SRS Testing Requirements:** See `docs/monoterminal-srs.md` §6.1
- **CI Configuration:** See `.github/workflows/test.yml`
- **Coverage Config:** See `.tarpaulin.toml` and `.codecov.yml`

**Questions?** Contact qa-lead
