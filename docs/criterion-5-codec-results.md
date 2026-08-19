# Criterion #5 - Protocol Codec Latency Results (Partial)

**Generated:** 2026-08-16 (Run 1 - Codec Only)  
**Status:** ⚠️ **PARTIAL** - Component-level measurements only  
**SRS Reference:** §6.1 (Protocol Performance), §5.1.2 (Latency Targets)

---

## ⚠️ Status: E2E Benchmark Blocked

**Completed:**
- ✅ Protocol encode/decode latency (codec benchmark)
- ✅ Compression overhead measurements
- ✅ WebSocket frame overhead
- ✅ Fan-out broadcast latency

**Blocked:**
- ❌ **End-to-end RTT measurement (Criterion #5 primary requirement)**
- **Blocker:** TLS certificate path issue in `latency_e2e_lan` benchmark
- **Error:** `Failed to open cert file certs/server.crt` (working directory mismatch)

---

## Protocol Encode/Decode Latency

### AttachRequest (Auth Handshake)
| Operation | Latency | Notes |
|-----------|---------|-------|
| **Encode** | **265.87 ns** | Client → Server |
| **Decode** | **184.53 ns** | Server processing |
| **Round-trip** | **~450 ns** | Codec overhead only |

### OutputData (Terminal Stream)
| Payload Size | Encode | Decode | Total RT |
|--------------|--------|--------|----------|
| 256 bytes | 222.63 ns | 153.25 ns | 375.88 ns |
| 1024 bytes | 264.56 ns | 169.59 ns | 434.15 ns |
| 4096 bytes | 329.15 ns | 228.59 ns | 557.74 ns |
| 16384 bytes | 1.2755 µs | 750.56 ns | 2.026 µs |

**Throughput:**
- 256 bytes: 1.07 GiB/s encode, 1.56 GiB/s decode
- 4096 bytes: 11.59 GiB/s encode, 16.69 GiB/s decode
- 16384 bytes: 11.96 GiB/s encode, 20.33 GiB/s decode

---

## Compression Overhead (zstd level 3)

| Operation | Latency | Throughput |
|-----------|---------|------------|
| **Encode** | **25.104 µs** | 721.78 MiB/s |
| **Decode** | **6.5507 µs** | 5.39 MiB/s |

**Test data:** 1000 lines of repeated "echo 'Hello World'\n" (~18KB)

---

## WebSocket Frame Overhead

| Frame Size | Latency | Throughput |
|------------|---------|------------|
| 64 bytes (small) | 52.798 ns | 1.13 GiB/s |
| 4096 bytes (large) | 106.36 ns | 35.87 GiB/s |

---

## Fan-out Broadcast (Session Collaboration)

| Clients | Latency | Throughput |
|---------|---------|------------|
| 1 client | 73.929 ns | 13.53 Melem/s |
| 2 clients | 151.08 ns | 13.24 Melem/s |
| 5 clients | 379.29 ns | 13.18 Melem/s |
| 10 clients | 767.08 ns | 13.04 Melem/s |

**Observation:** Near-linear scaling (cloning Arc overhead)

---

## AttachResponse with Scrollback

**1000 lines scrollback:** 27.672 µs

---

## Latency Budget Breakdown (Estimated)

Based on codec measurements, protocol overhead contribution to total RTT:

| Component | Measured Latency | Budget Target |
|-----------|------------------|---------------|
| Client encode (InputData) | ~265 ns | < 0.5 ms ✅ |
| Server decode (InputData) | ~185 ns | < 0.5 ms ✅ |
| Server encode (OutputData) | ~265 ns | < 0.5 ms ✅ |
| Client decode (OutputData) | ~185 ns | < 0.5 ms ✅ |
| **Total codec overhead** | **~900 ns (0.0009 ms)** | **< 2 ms ✅** |

**Remaining budget for network + PTY:** ~9 ms (out of 10ms p95 target)

---

## Next Steps to Complete Criterion #5

1. **Fix E2E benchmark TLS cert path**
   - Issue: Benchmark working directory ≠ project root
   - Solution: Use absolute path or adjust cargo bench working directory

2. **Execute `latency_e2e_lan` benchmark**
   - Measures ACTUAL end-to-end RTT with real WebSocket server
   - Validates p95 < 10ms acceptance criterion

3. **Deliver complete results**
   - Codec measurements (this document) + E2E RTT measurements
   - Combined report with pass/fail verdict for Criterion #5

---

## Evidence Trail

- **Raw Output:** `target/benchmark_all_output.txt`
- **Criterion JSON:** `target/criterion/*/base/estimates.json`
- **HTML Reports:** `target/criterion/*/report/index.html`

---

## Conclusion

**Protocol codec performance is EXCELLENT** - well under latency budget with nanosecond-level overhead. However, **Criterion #5 cannot be verified until E2E RTT benchmark completes** with actual network round-trip measurements.

**Status:** ⏸️ **BLOCKED** on TLS cert path fix for E2E measurement
