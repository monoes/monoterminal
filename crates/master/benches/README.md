# Performance Benchmarks

This directory contains criterion.rs benchmarks validating Phase 1 acceptance criteria (SRS §7.1).

---

## Quick Start

Run all benchmarks:
```powershell
cd crates\master
cargo bench
```

View results:
```powershell
# HTML reports generated at:
..\..\ target\criterion\report\index.html

# Open in browser:
start ..\..\ target\criterion\report\index.html
```

---

## Benchmark Files

### 1. FPS Rendering (`fps_rendering.rs`)

**Validates:** SRS §7.1 Criterion #1 - 60 FPS master rendering

**What it measures:**
- Dirty cell tracking: < 0.5ms (SRS §2.1.1)
- Glyph cache lookup: < 1ms
- GPU command submission: < 8ms
- Full frame cycle: < 16.67ms (60 Hz budget)
- Incremental rendering (hot path)

**Run:**
```powershell
cargo bench --bench fps_rendering
```

**Success criteria:**
- Full frame cycle < 16.67ms (60 FPS)
- p50 ≥ 60 FPS, p95 ≥ 58 FPS
- Incremental rendering < 5ms for typical workload

**View results:**
```powershell
start ..\..\target\criterion\fps_rendering\report\index.html
```

---

### 2. WebSocket Latency (`websocket_latency.rs`)

**Validates:** SRS §7.1 Criterion #5 - <10ms LAN latency (p95)

**What it measures:**
- Message serialization (encode/decode)
- PTY echo latency
- Session fan-out broadcast (1→N clients)
- Full RTT simulation (input → output)
- Queue backpressure handling
- Concurrent session latency

**Run:**
```powershell
cargo bench --bench websocket_latency
```

**Success criteria:**
- Full RTT p50 < 5ms
- Full RTT p95 < 10ms ✅ (Phase 1 gate)
- Full RTT p99 < 15ms
- Fan-out overhead < 1ms for N ≤ 10 clients

**View results:**
```powershell
start ..\..\target\criterion\websocket_latency\report\index.html
```

---

### 3. PTY Throughput (`pty_throughput.rs`)

**Validates:** SRS §6.1 PTY performance

**What it measures:**
- Ring buffer append/eviction
- UTF-8 validation throughput
- ANSI escape sequence parsing
- Scrollback retrieval
- Scrollback compression (zstd)

**Run:**
```powershell
cargo bench --bench pty_throughput
```

**Success criteria:**
- Ring buffer append: < 1µs per line
- UTF-8 validation: > 100 MB/s
- Scrollback compression: > 300 MB/s

---

## 24-Hour Soak Test

**Validates:** SRS §7.1 Criterion #7 - Zero crashes

**Location:** `tests/soak/stability_24h.rs`

**What it tests:**
- Zero crashes over 24 hours
- Memory growth ≤ 10%
- No handle leaks (Windows)
- No zombie PTY processes

**Run (full 24h):**
```powershell
cargo test --release --test stability_24h -- --ignored --nocapture
```

**Run (1h validation):**
```powershell
$env:SOAK_DURATION_HOURS=1
cargo test --release --test stability_24h -- --ignored --nocapture
```

**Run (8h dry run):**
```powershell
$env:SOAK_DURATION_HOURS=8
cargo test --release --test stability_24h -- --ignored --nocapture
```

**Success criteria:**
- Test completes without panic
- Memory growth ≤ 10%
- Zero zombie processes

**Monitor memory during test:**

In a separate terminal:
```powershell
.\..\..\tools\soak-test-monitor.ps1 -ProcessName "stability_24h" -DurationHours 24
```

---

## Benchmark Configuration

All benchmarks use criterion.rs with:

- **Sample size:**
  - FPS: 100 samples
  - Latency: 10,000 samples (for accurate p95/p99)
  - PTY: 100 samples

- **Warm-up:**
  - FPS: 3 seconds
  - Latency: 5 seconds
  - PTY: 3 seconds

- **Measurement time:**
  - FPS: 10 seconds
  - Latency: 20 seconds (longer for p95 accuracy)
  - PTY: 10 seconds

To customize:
```rust
Criterion::default()
    .sample_size(200)
    .warm_up_time(Duration::from_secs(5))
    .measurement_time(Duration::from_secs(15))
```

---

## CI Integration

All benchmarks are registered in `Cargo.toml`:

```toml
[[bench]]
name = "fps_rendering"
harness = false

[[bench]]
name = "websocket_latency"
harness = false

[[bench]]
name = "pty_throughput"
harness = false
```

CI workflow (`.github/workflows/benchmarks.yml`):
```yaml
- name: Run benchmarks
  run: cargo bench --no-fail-fast

- name: Upload criterion reports
  uses: actions/upload-artifact@v3
  with:
    name: criterion-reports
    path: target/criterion/
```

---

## Interpreting Results

### Criterion Output Format

```
render-scrolling-1000-lines
                        time:   [14.235 ms 14.456 ms 14.701 ms]
                        thrpt:  [68.02 Kelem/s 69.18 Kelem/s 70.28 Kelem/s]
```

- **time**: [p5, p50, p95] latency
- **thrpt**: throughput (elements/second)
- **p50**: median (typical performance)
- **p95**: 95th percentile (worst-case for most requests)

### Pass/Fail Criteria

| Benchmark | Metric | Target | Status |
|-----------|--------|--------|--------|
| **FPS** | Full frame | < 16.67ms (60 Hz) | ✅ Pass if met |
| **Latency** | RTT p95 | < 10ms | ✅ **Phase 1 gate** |
| **PTY** | Ring buffer | < 1µs/line | ✅ Pass if met |
| **Soak** | Zero crashes | 24 hours | ✅ **Phase 1 gate** |

---

## Performance Budgets (SRS §2.1.1)

Frame budget (60 FPS = 16.67ms):

| Component | Budget | Benchmark |
|-----------|--------|-----------|
| PTY read | 2ms | `pty_throughput::bench_pty_echo_latency` |
| Dirty tracking | 0.5ms | `fps_rendering::bench_dirty_cell_tracking` |
| Glyph lookup | 1ms | `fps_rendering::bench_glyph_cache_lookup` |
| GPU render | 8ms | `fps_rendering::bench_gpu_command_submission` |
| VSync | 5ms | *(hardware-dependent)* |
| **Total** | **16.5ms** | `fps_rendering::bench_full_frame_cycle` |

---

## Troubleshooting

### Criterion not found

```powershell
# Ensure criterion is in workspace dependencies
cargo update
cargo bench --bench fps_rendering
```

### Benchmarks timeout

```powershell
# Reduce measurement time
cargo bench --bench fps_rendering -- --measurement-time 5
```

### Cannot run soak test

```powershell
# Soak test is marked #[ignore] - must use --ignored flag
cargo test --release --test stability_24h -- --ignored --nocapture
```

### Memory monitor fails

Ensure PowerShell can read process stats:
```powershell
Get-Process -Id $PID | Select-Object WorkingSet64, HandleCount
```

---

## Phase 1 Gate Checklist

Before advancing to Phase 2:

- [ ] FPS benchmark: p50 ≥ 60 FPS ✅
- [ ] Latency benchmark: p95 < 10ms ✅
- [ ] Soak test: 24h zero crashes ✅
- [ ] Criterion HTML reports generated
- [ ] Evidence uploaded to `tests/evidence/phase1/`
- [ ] QA Lead sign-off

---

**Maintained by:** performance-engineer  
**Last Updated:** 2026-08-15
