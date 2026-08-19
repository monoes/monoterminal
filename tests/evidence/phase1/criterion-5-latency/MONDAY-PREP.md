# Monday Execution Prep - Criterion #5 Latency Benchmark Conversion

**Date:** 2026-08-15 (Saturday prep for Monday Aug 18)  
**Owner:** performance-engineer  
**Status:** ✅ Ready for execution (pending toolchain fix)

## Critical Dependency
⚠️ **BLOCKER:** Wait for devops-lead's "environment ready" confirmation (Monday 8-9 AM)  
Do NOT start benchmark work until Rust toolchain is confirmed working.

## File Conversion Plan

### Target File
`crates/master/benches/latency_e2e_lan.rs` (lines 56-107)

### Changes Required

#### 1. Replace Mock Client with Real Client (lines 337-352)
**Delete:** `MockWebSocketClient` struct  
**Replace with:** Use `TestWsClient` from `crates/master/tests/common/ws_client.rs`

```rust
// FROM tests/common/ws_client.rs (already exists, ready to use):
pub struct TestWsClient {
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    url: String,
}

impl TestWsClient {
    pub async fn connect(&mut self) -> Result<()>
    pub async fn send_binary(&mut self, data: Vec<u8>) -> Result<()>
    pub async fn recv(&mut self) -> Result<Message>
    pub async fn attach(&mut self, session_id: &str, jwt_bearer: &str, rows: u32, cols: u32) -> Result<AttachResponse>
    pub async fn send_input(&mut self, data: &[u8], jwt_bearer: &str) -> Result<()>
}
```

#### 2. Add MockPtyBackend for Reproducible Echo
**Pattern from:** `crates/master/tests/session_state_machine.rs` (lines 8-47)

```rust
// Add to benchmark file (after imports):
struct MockPtyBackend {
    pid: u32,
}

#[async_trait::async_trait]
impl monoterminal_master::pty::PtyBackend for MockPtyBackend {
    async fn create(_config: PtyConfig) -> PtyResult<Self> {
        Ok(Self { pid: 12345 })
    }

    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Fixed 1ms delay for reproducible baseline
        tokio::time::sleep(Duration::from_millis(1)).await;
        // Echo pattern: copy last written data
        Ok(0) // TODO: Implement echo buffer
    }

    async fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        // Store for echo in read()
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

#### 3. Replace Mock Echo Server with Real Server (lines 63-95)
**Pattern from:** `crates/master/src/main.rs`

**Key steps:**
1. Generate test JWT key (32 bytes)
2. Create auth service, rate limiter, session manager
3. Start real server on 127.0.0.1:18080
4. Create session with MockPtyBackend
5. Connect TestWsClient and authenticate
6. Measure attach → send_input → recv RTT
7. Cleanup

**Reference files:**
- Server init: `crates/master/src/main.rs` (lines 42-86)
- Test client: `crates/master/tests/common/ws_client.rs`
- Session creation: `crates/master/tests/session_state_machine.rs` (lines 56-66)

#### 4. Remove Old Mock Echo Server (lines 314-334)
Delete `mock_websocket_echo_server()` function (no longer needed).

## Evidence Collection Plan

### 1. Criterion HTML Report (Automatic)
**Source:** `target/criterion/e2e_lan_latency/report/index.html`  
**Destination:** `tests/evidence/phase1/criterion-5-latency/benchmark-report.html`

```powershell
# After benchmark completes
Copy-Item `
    "target\criterion\e2e_lan_latency\report\index.html" `
    "tests\evidence\phase1\criterion-5-latency\benchmark-report.html"
```

### 2. Wireshark PCAP Capture (Manual)
**Prerequisites:** Verify npcap installed (`wireshark --version`)

**Steps:**
1. Open Wireshark
2. Select adapter: "Adapter for loopback traffic capture"
3. Start capture
4. Apply filter: `tcp.port == 18080`
5. Run benchmark: `cargo bench --bench latency_e2e_lan`
6. Stop capture
7. Export: `tests/evidence/phase1/criterion-5-latency/lan_traffic.pcapng`
8. Statistics → Conversations → TCP
9. Screenshot showing RTT column (verify p95 < 10ms)

### 3. Latency Histogram (Generate from Criterion JSON)
**Source:** `target/criterion/e2e_lan_latency/base/estimates.json`

```python
# Quick Python script (or use Excel)
import json
import matplotlib.pyplot as plt

with open('target/criterion/e2e_lan_latency/base/estimates.json') as f:
    data = json.load(f)

# Extract percentiles
p50 = data['percentiles']['50']
p95 = data['percentiles']['95']
p99 = data['percentiles']['99']

# Create histogram with threshold lines
plt.figure(figsize=(10, 6))
plt.hist(data['samples'], bins=50, alpha=0.7, label='Latency Distribution')
plt.axvline(p50, color='green', linestyle='--', label=f'p50: {p50:.2f}ms')
plt.axvline(p95, color='orange', linestyle='--', label=f'p95: {p95:.2f}ms (target: <10ms)')
plt.axvline(p99, color='red', linestyle='--', label=f'p99: {p99:.2f}ms')
plt.xlabel('Latency (ms)')
plt.ylabel('Frequency')
plt.title('End-to-End LAN Latency Distribution - Criterion #5')
plt.legend()
plt.savefig('tests/evidence/phase1/criterion-5-latency/latency-histogram.png', dpi=150)
```

## Expected Results (from Saturday Analysis)

### Loopback Test (127.0.0.1)
- **p50:** 3-7ms ✅ SHOULD PASS
- **p95:** 6-10ms ✅ BORDERLINE (likely pass, but close)
- **p99:** 8-12ms ✅ SHOULD PASS

### Component Overhead Budget
- Protobuf encode/decode: <1ms total
- Network RTT loopback: 1-3ms
- MockPtyBackend echo: 1ms (fixed)
- Thread scheduling jitter: 0.5-2ms

**Total budget:** ~5-8ms p50, ~8-10ms p95 (meets Phase 1 target)

## Monday Timeline

### 08:00-09:00: WAIT for Toolchain Fix
⏸️ **DO NOT START** until devops-lead confirms "environment ready"

### 09:00-13:00: Benchmark Conversion (2-4 hours)
1. Add imports for server components
2. Implement `MockPtyBackend` with echo buffer
3. Replace mock client/server with real server + `TestWsClient`
4. Build test: `cargo build --release -p monoterminal-master`
5. Verify compilation: `cargo check --benches`

### 13:00-14:00: Benchmark Execution
```powershell
# Build with instrumentation
cargo build --release --features latency-tracing -p monoterminal-master

# Run benchmark (10,000 samples, ~5 minutes)
cargo bench --bench latency_e2e_lan

# Verify HTML report generated
ls target\criterion\e2e_lan_latency\report\index.html
```

### 14:00-15:00: Evidence Collection
1. Copy Criterion HTML report
2. Run Wireshark capture (repeat benchmark)
3. Generate latency histogram from JSON
4. Screenshot Wireshark statistics

### 15:00-17:00: Report to qa-lead
**Report contents:**
- ✅/❌ Pass/fail verdict (p95 < 10ms threshold)
- Criterion HTML report (linked)
- Wireshark PCAP + screenshot
- Latency histogram PNG
- Component breakdown analysis
- Any issues encountered

## Risk Mitigation

### If p95 > 10ms on Loopback
1. **Check ConPTY overhead:** Switch to MockPtyBackend if needed (already planned)
2. **Verify environment:** No background processes causing jitter
3. **Run multiple iterations:** Confirm consistency (not one-off spike)
4. **Escalate:** Report to eng-director if consistently fails

### If Wireshark/npcap Not Installed
1. Download: https://npcap.com/
2. Install with "WinPcap compatibility mode" enabled
3. Restart Wireshark
4. Verify loopback adapter visible

### If Benchmark Conversion Issues
1. Check imports (may need to add `use` statements)
2. Verify `TestWsClient` path: `use crate::tests::common::ws_client::TestWsClient;`
3. Check server initialization (auth service, session manager)
4. Escalate to rust-backend-lead if stuck >1 hour

## Coordination Notes

### Soak Test Alignment (with devops-lead)
**Opportunity:** Run 5-min latency validation BEFORE devops-lead's 24h soak test  
**Benefit:** Reuse same instrumented build for both tests  
**Action:** Coordinate timing Monday morning

### Success Criteria Recap (Phase 1 Verification Plan §3.5)
**Automated benchmark:**
- ✅ p50 < 5ms
- ✅ **p95 < 10ms** ← PHASE 1 GATE TARGET (authoritative)
- ✅ p99 < 15ms
- ✅ 0% packet loss

**Manual Wireshark:**
- ✅ 95% of TCP RTT < 10ms
- ✅ 0% retransmissions

## Quick Reference: File Paths

### Source Files
- Benchmark: `crates/master/benches/latency_e2e_lan.rs`
- Test client: `crates/master/tests/common/ws_client.rs`
- Mock PTY pattern: `crates/master/tests/session_state_machine.rs`
- Server pattern: `crates/master/src/main.rs`

### Evidence Output
- Base dir: `tests/evidence/phase1/criterion-5-latency/`
- HTML report: `benchmark-report.html`
- PCAP capture: `lan_traffic.pcapng`
- Histogram: `latency-histogram.png`
- Final report: `verification-report.md`

## Immediate Actions (Monday 9 AM)

```powershell
# 1. Verify toolchain working
cargo --version
rustc --version

# 2. Verify benchmark file exists
cat crates\master\benches\latency_e2e_lan.rs | head -20

# 3. Start conversion work (use this prep doc as reference)
code crates\master\benches\latency_e2e_lan.rs
```

---

**Prepared by:** performance-engineer  
**Date:** 2026-08-15 22:30 (Saturday)  
**Status:** ✅ Ready for Monday execution
