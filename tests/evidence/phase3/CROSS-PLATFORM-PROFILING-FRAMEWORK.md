# Cross-Platform Performance Profiling Framework

**Phase:** 3 Week 7-8  
**Task:** task-64  
**Engineer:** performance-engineer  
**Date:** 2026-08-20

---

## Overview

This document defines the profiling framework for validating SRS performance targets across Windows, Linux, and macOS platforms during Phase 3 expansion.

**SRS Performance Targets (§1.3):**
- 60 FPS rendering (all platforms)
- <30ms p95 latency LAN (cross-platform)
- 1000 concurrent sessions (validated Phase 2 Windows)
- Platform parity (no significant degradation)

---

## Profiling Categories

### 1. PTY Performance

**Metrics:**
- Creation time (target: <100ms, SRS §6.1)
- Read throughput (4KB buffer, SRS §3.1.4)
- Write throughput (various sizes: 64B, 256B, 1KB, 4KB)
- Resize latency (target: <10ms, SRS §6.1)
- Concurrent operations (write + resize)

**Platform-Specific Implementations:**
- **Windows:** ConPTY (Windows Console API)
- **Linux:** portable-pty (openpty/forkpty)
- **macOS:** portable-pty (openpty/forkpty)

**Benchmark:** `crates/master/benches/unix_pty_performance.rs` (Unix) + Windows equivalent

### 2. GPU Rendering Performance

**Metrics:**
- Frame budget compliance (<16.67ms for 60 FPS)
- Dirty cell tracking (<0.5ms for 80x24 terminal)
- Glyph cache lookup (<1ms)
- GPU command submission (<8ms for full-screen)
- Incremental rendering (1%, 5%, 10%, 25%, 50%, 100% dirty)

**Platform-Specific Backends:**
- **Windows:** wgpu + DirectX 12
- **Linux:** wgpu + Vulkan (or OpenGL fallback)
- **macOS:** wgpu + Metal

**Benchmark:** `crates/master/benches/fps_rendering.rs`

### 3. Memory Profiling

**Metrics:**
- Baseline memory footprint (idle daemon)
- Per-session memory overhead
- 1000-session memory usage (validated Phase 2: linear growth, 0% overhead)
- Memory leak detection (30-min soak test)

**Tools:**
- **Windows:** Windows Task Manager, Windows Performance Analyzer
- **Linux:** `valgrind --tool=massif`, `/proc/[pid]/status`
- **macOS:** Instruments (Memory Profiler), Activity Monitor

### 4. Network Latency

**Metrics:**
- LAN p95 latency (target: <30ms, SRS §1.3)
- WebSocket frame overhead
- Protocol encode/decode (validated Phase 2: <1µs)

**Benchmark:** `crates/master/benches/latency_e2e_lan.rs`

---

## Execution Plan

### Week 7: Baseline Profiling

**Day 1-2: Windows Baseline (Local)**
- ✅ PTY throughput benchmark
- ✅ FPS rendering benchmark
- ⏳ Memory profiling (idle + 100 sessions + 1000 sessions)
- ⏳ Network latency benchmark

**Day 3-4: Linux Profiling (CI or Cloud VM)**
- 🔄 Unix PTY benchmark (`cargo bench --bench unix_pty_performance`)
- 🔄 FPS rendering (Vulkan backend)
- 🔄 Memory profiling
- 🔄 Network latency

**Day 5-6: macOS Profiling (CI or Cloud VM)**
- 🔄 Unix PTY benchmark
- 🔄 FPS rendering (Metal backend)
- 🔄 Memory profiling
- 🔄 Network latency

**Day 7: Comparison Analysis**
- 🔄 Platform comparison matrix
- 🔄 Bottleneck identification
- 🔄 Optimization recommendations

### Week 8: Optimization & Documentation

**Day 8-10: Platform-Specific Optimizations**
- 🔄 PTY buffer tuning
- 🔄 GPU backend optimizations
- 🔄 Memory allocation strategies

**Day 11-12: Benchmark Suite Expansion**
- 🔄 CI integration (GitHub Actions)
- 🔄 Automated regression detection
- 🔄 Platform comparison reports

**Day 13-14: Performance Documentation**
- 🔄 Platform characteristics guide
- 🔄 Tuning guidelines per platform
- 🔄 Known performance differences

---

## Benchmark Execution

### Automated (Recommended)

**GitHub Actions CI:**
```yaml
# .github/workflows/performance-benchmarks.yml
name: Cross-Platform Performance Benchmarks

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: '0 0 * * 0' # Weekly

jobs:
  benchmark-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo bench --bench pty_throughput
      - run: cargo bench --bench fps_rendering
      - uses: actions/upload-artifact@v3
        with:
          name: windows-benchmarks
          path: target/criterion/

  benchmark-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo bench --bench unix_pty_performance
      - run: cargo bench --bench fps_rendering
      - uses: actions/upload-artifact@v3
        with:
          name: linux-benchmarks
          path: target/criterion/

  benchmark-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - run: cargo bench --bench unix_pty_performance
      - run: cargo bench --bench fps_rendering
      - uses: actions/upload-artifact@v3
        with:
          name: macos-benchmarks
          path: target/criterion/
```

### Manual Execution

**Local or SSH to remote platform:**
```bash
# Clone repository
git clone https://github.com/monoes/monoterminal
cd monoterminal

# Run benchmarks
cargo bench --bench pty_throughput
cargo bench --bench unix_pty_performance # Unix only
cargo bench --bench fps_rendering
cargo bench --bench latency_e2e_lan

# Results in target/criterion/
```

---

## Results Collection

### Criterion Output Format

**Location:** `target/criterion/[bench_name]/[test_name]/`

**Files:**
- `report/index.html` - Visual report with charts
- `base/estimates.json` - Statistical estimates (mean, std dev, p-values)
- `new/estimates.json` - Latest run estimates

**Extract metrics:**
```bash
# Mean execution time
cat target/criterion/unix_pty_creation/create_pty/new/estimates.json | jq '.mean.point_estimate'

# Throughput
cat target/criterion/unix_pty_write/write_4kb/new/estimates.json | jq '.throughput'
```

### Evidence Archival

**Directory structure:**
```
tests/evidence/phase3/
├── windows-pty-throughput-YYYYMMDD-HHMMSS.log
├── windows-fps-rendering-YYYYMMDD-HHMMSS.log
├── linux-pty-throughput-YYYYMMDD-HHMMSS.log
├── linux-fps-rendering-YYYYMMDD-HHMMSS.log
├── macos-pty-throughput-YYYYMMDD-HHMMSS.log
├── macos-fps-rendering-YYYYMMDD-HHMMSS.log
├── PLATFORM-COMPARISON-MATRIX.md
└── OPTIMIZATION-RECOMMENDATIONS.md
```

---

## Platform Comparison Matrix Template

**See:** `PLATFORM-COMPARISON-MATRIX.md` (generated after all platforms profiled)

**Format:**
| Metric | Target | Windows | Linux | macOS | Status |
|--------|--------|---------|-------|-------|--------|
| PTY creation | <100ms | [TBD] | [TBD] | [TBD] | [PASS/FAIL] |
| PTY read (4KB) | [baseline] | [TBD] | [TBD] | [TBD] | [comparison] |
| FPS (60 Hz) | 16.67ms | [TBD] | [TBD] | [TBD] | [PASS/FAIL] |
| Memory (1000 sessions) | Linear growth | [TBD] | [TBD] | [TBD] | [PASS/FAIL] |

---

## Success Criteria

**Phase 3 Week 7-8 Completion:**
- ✅ All platforms profiled (Windows, Linux, macOS)
- ✅ SRS targets validated on each platform
- ✅ Platform parity confirmed (<20% variance acceptable)
- ✅ Bottlenecks identified and documented
- ✅ Optimization recommendations delivered
- ✅ Performance documentation complete

---

**Status:** Week 7 Day 1 - Windows baseline profiling in progress

**Next steps:**
1. Complete Windows baseline (PTY + FPS benchmarks running)
2. Extract results and populate comparison matrix
3. Execute Linux profiling (CI or cloud VM)
4. Execute macOS profiling (CI or cloud VM)
5. Generate platform comparison report

---

**Updated:** 2026-08-20  
**Engineer:** performance-engineer
