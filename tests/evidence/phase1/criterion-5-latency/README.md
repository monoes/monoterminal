# Phase 1 Criterion #5: LAN Latency Evidence

## Acceptance Criterion
**Phase 1 Gate:** p95 < 10ms local/LAN latency (SRS §7.1)  
**General SRS Target:** LAN p95 < 30ms (SRS §5.1.2)

## Evidence Files

### 1. Criterion.rs Benchmark Reports
- `websocket_latency_report/` - Component-level latency benchmarks
- `latency_e2e_lan_report/` - End-to-end network RTT benchmarks
- `estimates.json` - Quantitative p50/p95/p99 measurements

### 2. Network Packet Captures
- `lan_traffic.pcapng` - Wireshark capture of benchmark traffic
- `wireshark_statistics.png` - TCP conversation statistics (p95 verification)

### 3. Latency Histograms
- `latency_histogram_p95.png` - Visual distribution with p50/p95/p99 markers
- `latency_histogram_p95.svg` - SVG version for documentation

### 4. Test Procedure
- `test_procedure.md` - Step-by-step reproduction instructions
- `environment.md` - Hardware/network configuration details

## Benchmark Execution

### Component Benchmarks (Existing)
```powershell
cargo bench --bench websocket_latency
```

### End-to-End LAN Benchmarks (New)
```powershell
# Run with real network measurement
cargo bench --bench latency_e2e_lan

# With Wireshark capture (manual steps):
# 1. Start Wireshark, filter: tcp.port == 18080
# 2. Run: cargo bench --bench latency_e2e_lan
# 3. Stop capture, save as lan_traffic.pcapng
# 4. Statistics → Conversations → TCP
```

## Success Criteria (Verification Plan §3.5)
- ✅ p50 < 5ms
- ✅ p95 < 10ms (GATE REQUIREMENT)
- ✅ p99 < 15ms
- ✅ No packet loss (0% dropped frames)
- ✅ Consistent latency under concurrent load (10 clients)

## Measurement Points
```
[Web Client] ---(1)---> [WebSocket] ---(2)---> [Master PTY Input]
                                                     |
                                                     v
[Web Client] <---(4)--- [WebSocket] <---(3)--- [Master PTY Output]

Total RTT = (4) - (1)
```

## Notes
- Benchmarks run on **same LAN** (1 Gbps switch recommended)
- Internet disabled during measurement to isolate LAN-only latency
- Sample size: 10,000 measurements per verification plan
- Measurement time: 30 seconds per benchmark group
