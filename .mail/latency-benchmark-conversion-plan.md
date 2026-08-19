# Latency Benchmark Conversion Plan - Saturday Draft

**Target File:** `crates/master/benches/latency_e2e_lan.rs` (lines 56-107)  
**Conversion Effort:** 2-4 hours (Monday 9 AM - 1 PM)  
**Author:** performance-engineer  
**Date:** 2026-08-15 Saturday evening

## Current State Analysis

### Mock Implementation (Lines 56-107)
**Problems identified:**
1. `MockWebSocketClient` returns hardcoded 500μs latency (line 350)
2. `mock_websocket_echo_server` is raw TCP echo (lines 315-334) - NO auth, NO protobuf
3. NOT measuring real WebSocket + TLS + Auth + Protocol stack
4. NOT suitable for Phase 1 acceptance verification

### Working Reference Benchmark
**File:** `crates/master/benches/websocket_latency.rs`

**Key patterns to copy:**
- Criterion config (lines 304-309): `sample_size(10_000)`, `warm_up_time(5s)`, `measurement_time(20s)`
- RTT measurement pattern (lines 172-220): `b.iter_custom(|iters| { let start = Instant::now(); /* work */ start.elapsed() })`
- Uses `black_box()` for optimizer prevention

## Conversion Plan (Pseudocode)

### Step 1: Add Imports (Top of File)
```rust
// Add after existing use statements:
use monoterminal_master::{
    auth::{Ed25519AuthService, RateLimiter},
    server::{Server, ServerConfig},
    session::{Session, SessionManager},
};
use std::sync::Arc;
use tokio::sync::broadcast;
use std::path::PathBuf;
use uuid::Uuid;

// Import TestWsClient from tests/common
// NOTE: May need to adjust path based on crate structure
mod common {
    pub mod ws_client;
}
use common::ws_client::TestWsClient;
```

### Step 2: Implement MockPtyBackend with Echo Buffer
```rust
/// Mock PTY backend for reproducible latency baseline
/// Fixed 1ms echo delay for consistent measurements
struct MockPtyBackend {
    pid: u32,
    echo_buffer: Option<Vec<u8>>,
}

impl MockPtyBackend {
    fn new() -> Self {
        Self {
            pid: 12345,
            echo_buffer: None,
        }
    }
}

#[async_trait::async_trait]
impl monoterminal_master::pty::PtyBackend for MockPtyBackend {
    async fn create(_config: PtyConfig) -> PtyResult<Self> {
        Ok(Self::new())
    }

    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Fixed 1ms delay for reproducible baseline
        tokio::time::sleep(Duration::from_millis(1)).await;

        // Echo back what was written
        if let Some(data) = self.echo_buffer.take() {
            let len = std::cmp::min(data.len(), buf.len());
            buf[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    async fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        // Store for echo in read()
        self.echo_buffer = Some(data.to_vec());
        Ok(())
    }

    fn resize(&mut self, _rows: u16, _cols: u16) -> PtyResult<()> {
        Ok(())
    }

    fn shell_pid(&self) -> u32 {
        self.pid
    }

    async fn terminate(self) -> PtyResult<()> {
        Ok(())
    }
}
```

### Step 3: Replace Mock Echo Server (Lines 63-95)
```rust
group.bench_function("real_master_rtt_loopback", |b| {
    b.iter_custom(|iters| {
        rt.block_on(async {
            // === SERVER SETUP ===
            
            // 1. Generate test JWT key (32 bytes, all zeros for determinism)
            let jwt_key = [0u8; 32];
            
            // 2. Create auth service
            let auth_service = Arc::new(
                Ed25519AuthService::new(&jwt_key)
                    .expect("Failed to create auth service")
            );
            
            // 3. Create rate limiter
            let rate_limiter = Arc::new(RateLimiter::new());
            
            // 4. Create session manager
            let session_manager = Arc::new(SessionManager::new(None));
            
            // 5. Create health channel
            let (health_tx, _health_rx) = broadcast::channel(16);
            
            // 6. Configure server for loopback test
            let server_config = ServerConfig {
                bind_addr: "127.0.0.1:18080".parse().unwrap(),
                ..Default::default()
            };
            
            // 7. Create server instance
            let server = Server::new(
                server_config,
                session_manager.clone(),
                rate_limiter,
                auth_service.clone(),
                health_tx,
            ).expect("Failed to create server");
            
            // 8. Spawn server in background
            let server_handle = tokio::spawn(async move {
                server.run().await
            });
            
            // Give server time to bind (100ms)
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // === SESSION CREATION ===
            
            // 9. Create session with MockPtyBackend
            let session_id = Uuid::new_v4();
            let pty = Box::new(MockPtyBackend::new());
            let session = Session::new(
                session_id,
                pty,
                "mock.exe".to_string(),
                PathBuf::from("C:\\"),
                24,
                80,
            );
            
            // 10. Add session to manager
            session_manager.add_session(session).await
                .expect("Failed to add session");
            
            // === CLIENT SETUP ===
            
            // 11. Create and connect WebSocket client
            let mut client = TestWsClient::new("ws://127.0.0.1:18080");
            client.connect().await
                .expect("Failed to connect client");
            
            // 12. Generate JWT bearer string
            let user_id = UserId::from("test-user");
            let bearer_pair = auth_service.issue_tokens(&user_id)
                .expect("Failed to issue JWT");
            let bearer_string = bearer_pair.access;  // Use access field
            
            // 13. Attach to session
            client.attach(
                &session_id.to_string(),
                &bearer_string,
                24,
                80
            ).await.expect("Failed to attach to session");
            
            // === LATENCY MEASUREMENT ===
            
            let start = Instant::now();
            
            for _i in 0..iters {
                let send_time = Instant::now();
                
                // 14. Send input (single keypress)
                client.send_input(b"x", &bearer_string).await
                    .expect("Failed to send input");
                
                // 15. Receive output (echo from MockPtyBackend)
                let _response = client.recv().await
                    .expect("Failed to receive output");
                
                let rtt = send_time.elapsed();
                black_box(rtt);
            }
            
            // === CLEANUP ===
            
            // 16. Close client connection
            client.close().await.ok();
            
            // 17. Abort server task
            server_handle.abort();
            
            start.elapsed()
        })
    });
});
```

### Step 4: Delete Old Mock Infrastructure (Lines 314-352)
**Delete:**
- `mock_websocket_echo_server()` function (lines 315-334)
- `MockWebSocketClient` struct (lines 337-352)

**Reason:** No longer needed - using real server + TestWsClient

## Implementation Questions & Answers

### Q1: How do I access TestWsClient from tests/common/?
**A:** Benchmark crates typically can't directly access test utilities. Two options:
1. **Copy TestWsClient into benchmark file** (quick, might have duplicate code)
2. **Create shared test utilities crate** (cleaner, but more setup)

**Decision for Monday:** Start with option 1 (copy) to unblock. If time permits, refactor to option 2.

### Q2: How do I know if the JWT bearer format is correct?
**A:** Check existing integration tests for JWT format. Look for:
- `crates/master/tests/integration_*.rs`
- Pattern: `auth_service.issue_tokens(&user_id)?.access`

### Q3: What if server bind fails (port already in use)?
**A:** Use dynamic port allocation:
```rust
let server_config = ServerConfig {
    bind_addr: "127.0.0.1:0".parse().unwrap(),  // 0 = OS picks free port
    ..Default::default()
};
```
Then extract actual port from server after start.

**Decision for Monday:** Start with fixed 18080. If flaky, switch to dynamic.

### Q4: How do I verify MockPtyBackend echo is working?
**A:** Add debug logging before benchmark run:
```rust
// Before measurement loop:
tracing::info!("Testing MockPtyBackend echo...");
client.send_input(b"test", &bearer_string).await?;
let echo = client.recv().await?;
tracing::info!("Echo received: {:?}", echo);
```

## Risk Analysis

### Risk 1: TestWsClient Not Accessible from Benchmark
**Likelihood:** High (module visibility issue)  
**Impact:** 1-2 hours to resolve  
**Mitigation:** Copy TestWsClient code into benchmark file (lines 337-352 area)

### Risk 2: Server Initialization Failures
**Likelihood:** Medium (missing dependencies)  
**Impact:** 30-60 min debugging  
**Mitigation:** Study `crates/master/src/main.rs` initialization sequence carefully

### Risk 3: Auth Service JWT Format Mismatch
**Likelihood:** Low (well-tested API)  
**Impact:** 15-30 min  
**Mitigation:** Copy JWT generation pattern from existing integration tests

### Risk 4: p95 > 10ms on Loopback
**Likelihood:** Medium (borderline expected 6-10ms)  
**Impact:** Escalation to eng-director  
**Mitigation:** 
1. Verify no background CPU load (close VSCode, browsers)
2. Run multiple benchmark iterations to confirm consistency
3. Check Windows power plan (High Performance mode)
4. If consistent failure, report to eng-director with data

## Expected Benchmark Output

### Criterion Report Structure
```
target/criterion/
└── e2e_lan_latency/
    ├── report/
    │   └── index.html          ← Copy to evidence folder
    └── base/
        └── estimates.json      ← Contains p50/p95/p99 values
```

### Success Criteria (from Phase 1 Verification Plan §3.5)
```json
{
  "p50": "< 5ms",     // Expected: 3-7ms
  "p95": "< 10ms",    // GATE CRITERION (expected: 6-10ms borderline)
  "p99": "< 15ms"     // Expected: 8-12ms
}
```

### Component Budget Validation
From Saturday analysis:
- Protobuf encode/decode: <1ms total ✅
- Network RTT loopback: 1-3ms ✅
- MockPtyBackend echo: 1ms (fixed) ✅
- Thread scheduling jitter: 0.5-2ms ⚠️ (variable)

**Total expected:** 5-8ms p50, 8-10ms p95

## Monday Morning Checklist

### Pre-Execution (8:00-9:00 AM - WAIT FOR TOOLCHAIN)
- [ ] Confirm devops-lead "environment ready" message
- [ ] Verify `cargo --version` works
- [ ] Verify `rustc --version` works
- [ ] Open `latency_e2e_lan.rs` in editor

### Execution (9:00-13:00 PM - CONVERSION)
- [ ] Add imports (Step 1)
- [ ] Implement MockPtyBackend (Step 2)
- [ ] Replace benchmark function (Step 3)
- [ ] Delete old mock code (Step 4)
- [ ] Build: `cargo build --release -p monoterminal-master`
- [ ] Check: `cargo check --benches`
- [ ] Verify no compilation errors

### Benchmark Run (13:00-14:00 PM)
- [ ] Build with instrumentation: `cargo build --release --features latency-tracing -p monoterminal-master`
- [ ] Run benchmark: `cargo bench --bench latency_e2e_lan`
- [ ] Wait ~5 minutes for completion
- [ ] Verify HTML report: `target/criterion/e2e_lan_latency/report/index.html`

### Evidence Collection (14:00-15:00 PM)
- [ ] Copy Criterion HTML report to evidence folder
- [ ] Install Wireshark + npcap (if not already done)
- [ ] Run Wireshark capture (repeat benchmark)
- [ ] Export PCAP: `tests/evidence/phase1/criterion-5-latency/lan_traffic.pcapng`
- [ ] Screenshot Wireshark statistics (TCP conversations, RTT column)
- [ ] Generate latency histogram from `estimates.json`

### Report to qa-lead (15:00-17:00 PM)
- [ ] Write verification report markdown
- [ ] Include: Pass/Fail verdict, HTML report link, PCAP, histogram
- [ ] Document any issues encountered
- [ ] Send to qa-lead via org_send

## File References

### Source Files (Read-Only Study)
- Working benchmark template: `crates/master/benches/websocket_latency.rs` ✅ STUDIED
- Test client utility: `crates/master/tests/common/ws_client.rs` ✅ STUDIED
- MockPtyBackend pattern: `crates/master/tests/session_state_machine.rs` ✅ STUDIED
- Server initialization: `crates/master/src/main.rs` ✅ STUDIED
- Auth module: `crates/master/src/auth/mod.rs` ✅ STUDIED

### Target Files (Will Modify Monday)
- Benchmark to convert: `crates/master/benches/latency_e2e_lan.rs`
- Evidence folder: `tests/evidence/phase1/criterion-5-latency/`

### Evidence Outputs (Monday Delivery)
- `benchmark-report.html` (from Criterion)
- `lan_traffic.pcapng` (from Wireshark)
- `latency-histogram.png` (generated from JSON)
- `verification-report.md` (final report to qa-lead)

## Open Questions for Monday

1. **TestWsClient access:** Can benchmarks import from `tests/common/`? If not, copy code.
2. **Server port binding:** Is 18080 always free? Consider dynamic port if flaky.
3. **JWT format:** Verify `auth_service.issue_tokens(&user_id)?.access` returns bearer string.
4. **Wireshark npcap:** NOT installed Saturday - need to install Monday morning (30 min task).

## Confidence Assessment

**Conversion complexity:** Medium (2-4 hours is realistic)  
**Risk of p95 > 10ms:** Medium (borderline 6-10ms expected)  
**Overall readiness:** 85% (pending toolchain fix + Wireshark install)

**Blockers identified:**
1. ❌ Rust toolchain (devops-lead fixing Monday 8-9 AM)
2. ❌ Wireshark + npcap (need to install Monday morning)

**Ready to execute:** Monday 9 AM (after toolchain confirmation)

---

**Prepared by:** performance-engineer  
**Reviewed:** eng-director (Saturday coordination)  
**Status:** ✅ DRAFT COMPLETE - Ready for Monday execution
