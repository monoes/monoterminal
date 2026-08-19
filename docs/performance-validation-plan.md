# MONOTERMINAL Performance Validation Plan

**Version:** 1.0  
**Date:** 2026-08-15  
**Phase:** Phase 1 — Windows + Web MVP  
**Task:** task-17 (Performance Validation & Soak Test)

---

## Executive Summary

This document defines the performance validation procedure for MONOTERMINAL Phase 1, per SRS v1.2 §7.1 acceptance criteria. Validation occurs after integration tests (task-16) complete and before final QA acceptance (task-19).

**Phase 1 Scope:** 1-5 concurrent sessions (NOT 1000 — that's Phase 2)

---

## 1. Performance Targets (SRS §7.1)

### 1.1 Master Rendering: 60 FPS

**Target:** >= 58 FPS sustained (allowing 2 FPS margin)  
**Frame Budget:** 16.67ms total

**Phase Budgets (SRS §2.1.1):**
- PTY read: 2ms
- Dirty tracking: 0.5ms
- Glyph lookup: 1ms
- GPU render: 8ms
- VSync: 5ms

**Validation Method:**
1. Launch master daemon with local terminal UI (egui + wgpu)
2. Create 1 session, attach via local UI
3. Generate continuous output: `cat large_file.txt` (10k lines)
4. Measure FPS using built-in `PerformanceMonitor` (already exists in `crates/master/src/ui/performance.rs`)
5. Record average FPS over 60 seconds of scrolling

**Pass Criteria:** Average FPS >= 58 FPS over 60-second test

---

### 1.2 Network Latency: <10ms LAN

**Target (SRS §7.1):**
- p50 < 5ms (unstated but implied from p95 < 10ms)
- p95 < 10ms (localhost/LAN)

**Note:** Phase 1 tests localhost only (127.0.0.1). Internet latency testing deferred to Phase 2.

**Validation Method:**
1. Launch master daemon: `monoterminal.exe --listen 127.0.0.1:5000`
2. Open web client in Chrome/Edge: `http://localhost:5000`
3. Open latency measurement tool: `tools/latency-measurement.html`
4. Connect to WebSocket: `ws://127.0.0.1:5000/ws`
5. Run 100-sample test:
   - Send message with sequence number + timestamp (`performance.now()`)
   - Master echoes back with same sequence
   - Measure round-trip: send → receive → render
6. Calculate percentiles: p50, p95, p99

**Tool:** `tools/latency-measurement.html` (browser-based, uses `performance.now()`)

**Pass Criteria:**
- p50 < 5ms
- p95 < 10ms

---

### 1.3 24-Hour Soak Test: Zero Crashes

**Target (SRS §7.1):**
- Zero crashes (process exit code != 0)
- Memory stable (RSS growth < 10% over 24h)
- No handle leaks (Windows handle count stable)
- CPU no sustained spikes (< 50% sustained)

**Validation Method:**
1. Launch master daemon with 1 session attached
2. Simulate periodic I/O:
   - Every 5 minutes: send 100 lines of output (`echo "line" >> /dev/tty`)
   - Web client remains connected entire 24h
3. Monitor every 5 minutes using PowerShell script:
   - Working Set (MB)
   - Private Bytes (MB)
   - Handle Count
   - Thread Count
   - CPU usage
4. Log all metrics to CSV: `soak-test-results.csv`
5. Check for process crashes (PID no longer exists)

**Tool:** `tools/soak-test-monitor.ps1` (PowerShell, Windows-specific)

**Pass Criteria:**
- Zero crashes (process runs entire 24h)
- Memory growth < 10% from baseline
- Handle count stable (+/- 5%)
- No CPU spikes > 50% sustained

---

## 2. Benchmark Suite (SRS §6.1)

Uses **criterion.rs** for micro-benchmarks. Validates low-level performance assumptions.

### 2.1 Protocol Encode/Decode

**Location:** `crates/protocol/benches/codec.rs`

**Benchmarks:**
- `encode_attach_request` — encode AttachRequest to Protobuf
- `decode_attach_request` — decode AttachRequest from Protobuf
- `encode_output_data` — encode OutputData (various sizes: 256B, 1KB, 4KB, 16KB)
- `decode_output_data` — decode OutputData
- `compression_zstd` — zstd level 3 compression/decompression
- `attach_response_with_scrollback` — encode 1000 lines of scrollback
- `websocket_frame_overhead` — WebSocket framing overhead
- `client_fanout` — broadcast to N clients (N=1,2,5,10)

**Run:**
```bash
cd crates/protocol
cargo bench --bench codec
```

**Deliverable:** `target/criterion/` HTML reports

---

### 2.2 PTY Throughput

**Location:** `crates/master/benches/pty_throughput.rs`

**Benchmarks:**
- `ring_buffer_append` — scrollback ring buffer append with eviction
- `utf8_validation` — validate PTY output (various sizes)
- `ansi_parsing` — simplified ANSI escape sequence parsing
- `scrollback_retrieval` — retrieve chunks (100, 500, 1000, 5000 lines)
- `scrollback_compression` — compress 1000 lines with zstd level 3

**Run:**
```bash
cd crates/master
cargo bench --bench pty_throughput
```

**Deliverable:** `target/criterion/` HTML reports

---

### 2.3 Baseline Establishment

**First Run:** Establish baseline metrics (no comparison yet)

**Future Runs:** Compare against baseline to detect regressions:
- Protocol encode/decode: throughput (messages/sec)
- Compression ratio: ~50-60% (SRS §4.1.3)
- Scrollback retrieval: >10k lines/sec

---

## 3. Validation Workflow

### Phase 1: Benchmark Prep (NOW — before task-16 completes)
- ✅ Create criterion benchmarks (`crates/protocol/benches/`, `crates/master/benches/`)
- ✅ Create latency measurement tool (`tools/latency-measurement.html`)
- ✅ Create soak test monitor (`tools/soak-test-monitor.ps1`)
- ✅ Document validation plan (this file)

### Phase 2: Integration Tests Complete (task-16 done)
- Run criterion benchmarks, establish baseline
- Validate build artifacts exist (`target/release/monoterminal.exe`)

### Phase 3: Performance Validation (task-17 execution)

**Day 1: FPS & Latency Testing**
1. Launch master daemon
2. Run FPS test (60-second scrolling test)
3. Run latency test (100 samples, localhost)
4. Generate preliminary report

**Day 2-3: Start Soak Test**
1. Launch master daemon in stable configuration
2. Start soak test monitor (24 hours)
3. Schedule check-ins every 4-6 hours

**Day 4: Soak Test Analysis**
1. Analyze soak test results
2. Check for crashes, memory leaks, handle leaks
3. Generate final performance report

**Day 5: Report Delivery**
1. Compile all results into `docs/performance-report-phase1.md`
2. Include:
   - Executive summary (Pass/Fail)
   - FPS histogram with graph
   - Latency percentiles table
   - Soak test timeline (CSV + summary)
   - Criterion benchmark results (HTML links)
3. Handoff to qa-lead (task-19)

---

## 4. Tools Inventory

| Tool | Purpose | Platform | Output |
|------|---------|----------|--------|
| `tools/latency-measurement.html` | Browser-based latency testing | Web (Chrome/Edge) | Console + percentiles |
| `tools/soak-test-monitor.ps1` | 24-hour stability monitoring | Windows (PowerShell) | CSV + exit code |
| `crates/protocol/benches/codec.rs` | Protocol benchmarks | Rust (criterion) | HTML reports |
| `crates/master/benches/pty_throughput.rs` | PTY benchmarks | Rust (criterion) | HTML reports |
| `crates/master/src/ui/performance.rs` | FPS monitor (built-in) | Rust (egui) | Real-time FPS counter |

---

## 5. Acceptance Gates

**Phase 1 MUST meet all of:**
- ✅ 60 FPS master rendering (>= 58 FPS sustained)
- ✅ <10ms local latency (p95 < 10ms, localhost)
- ✅ Zero crashes in 24-hour soak test
- ✅ Memory stable (< 10% growth over 24h)
- ✅ Criterion benchmarks run successfully (baseline established)

**If any gate fails:** Block progression to task-19 (QA acceptance), file bug, fix, re-run validation.

---

## 6. Known Limitations (Phase 1)

**Out of Scope for Phase 1:**
- ❌ 1000 concurrent sessions (Phase 2, SRS §7.2)
- ❌ Internet latency testing (Phase 2)
- ❌ Mobile browser battery testing (Phase 2, SRS §5.1.3)
- ❌ Load testing beyond 1-5 sessions

**These are validated in Phase 2** after P2P/persistence/collaboration features land.

---

## 7. Deliverables

**To qa-lead (task-19):**
1. Performance report: `docs/performance-report-phase1.md`
2. Soak test results: `soak-test-results.csv`
3. Criterion HTML reports: `target/criterion/**/*.html`
4. Pass/Fail verdict per SRS §7.1 acceptance criteria

**Timeline:** 5 days after task-16 completes (1 day FPS/latency, 2 days soak test, 1 day analysis, 1 day reporting)

---

## References

- SRS v1.2 §7.1: Phase 1 acceptance criteria
- SRS v1.2 §5.1: Performance & Scalability targets
- SRS v1.2 §6.1: Testing strategy (criterion.rs)
- Task-17: Performance Validation & Soak Test assignment

---

**Prepared by:** performance-engineer  
**Status:** Ready for execution (awaiting task-16 completion)
