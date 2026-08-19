# MONOTERMINAL Performance Testing Tools

This directory contains performance validation tools for MONOTERMINAL Phase 1 acceptance testing (SRS §7.1).

---

## Tools Overview

### 1. Latency Measurement Tool

**File:** `latency-measurement.html`  
**Purpose:** Measure round-trip WebSocket latency (localhost/LAN)  
**Platform:** Web browser (Chrome/Edge recommended)

**Usage:**
1. Launch master daemon:
   ```powershell
   .\target\release\monoterminal.exe --listen 127.0.0.1:5000
   ```

2. Open `tools/latency-measurement.html` in Chrome/Edge

3. Configure WebSocket URL (default: `ws://127.0.0.1:5000/ws`)

4. Click **Connect**, then **Run Latency Test (100 samples)**

5. Results show p50/p95/p99 latency with Pass/Fail vs SRS §7.1 targets

**Targets (SRS §7.1):**
- p50 < 5ms
- p95 < 10ms (Phase 1 acceptance gate)

**Technical Details:**
- Uses `performance.now()` for microsecond precision
- Measures: keypress timestamp → server echo → client receive
- 100 samples per test run
- Results color-coded: green (pass), red (fail)

---

### 2. 24-Hour Soak Test Monitor

**File:** `soak-test-monitor.ps1`  
**Purpose:** Monitor process stability over 24 hours  
**Platform:** Windows PowerShell 5.1+

**Usage:**
1. Launch master daemon (ensure it's running):
   ```powershell
   .\target\release\monoterminal.exe
   ```

2. In a separate PowerShell window, start the monitor:
   ```powershell
   .\tools\soak-test-monitor.ps1 -ProcessName "monoterminal" -IntervalSeconds 300 -DurationHours 24
   ```

3. Monitor runs for 24 hours, sampling every 5 minutes (300 seconds)

4. Press Ctrl+C to stop early if needed

5. Results saved to `soak-test-results.csv` in current directory

**Targets (SRS §7.1):**
- Zero crashes (process runs entire 24h)
- Memory growth < 10% from baseline
- Handle count stable (+/- 5%)
- CPU < 50% sustained

**Output:**
- Console: real-time status with color-coded warnings
- CSV: full metrics log (timestamp, memory, handles, CPU, status)
- Exit code: 0 (pass), 1 (fail)

**Metrics Tracked:**
- Working Set (MB)
- Private Bytes (MB)
- Handle Count (Windows handles)
- Thread Count
- CPU usage (total seconds)
- Status: OK, MEM_LEAK, HANDLE_LEAK, CRASHED

**Parameters:**
```powershell
-ProcessName "monoterminal"    # Process to monitor
-IntervalSeconds 300           # Sample interval (default: 5 minutes)
-DurationHours 24              # Total duration (default: 24 hours)
-OutputCsv "results.csv"       # Output CSV filename
```

**Example Output:**
```
[14:35:00] Elapsed: 12.5h | WS: 128.45MB (+2.3%) | Handles: 245 (+1.2%) | Status: OK
[14:40:00] Elapsed: 12.6h | WS: 129.12MB (+2.8%) | Handles: 246 (+1.6%) | Status: OK
```

---

## Running Benchmarks

### Protocol Benchmarks

**Location:** `crates/protocol/benches/codec.rs`

**Run:**
```powershell
cd crates\protocol
cargo bench --bench codec
```

**Benchmarks:**
- Protocol encode/decode (AttachRequest, OutputData)
- Compression (zstd level 3)
- Scrollback serialization (1000 lines)
- WebSocket framing overhead
- Client fan-out broadcast (1→N clients)

**Output:** `target/criterion/codec/` (HTML reports)

---

### PTY Throughput Benchmarks

**Location:** `crates/master/benches/pty_throughput.rs`

**Run:**
```powershell
cd crates\master
cargo bench --bench pty_throughput
```

**Benchmarks:**
- Ring buffer append (scrollback eviction)
- UTF-8 validation (various sizes)
- ANSI escape sequence parsing
- Scrollback retrieval (chunk sizes: 100-5000 lines)
- Scrollback compression (zstd)

**Output:** `target/criterion/pty_throughput/` (HTML reports)

---

### Run All Benchmarks

**Quick Script:**
```powershell
# From repository root
cd crates\protocol
cargo bench --bench codec
cd ..\master
cargo bench --bench pty_throughput
cd ..\..
```

**View Results:**
Open `target/criterion/report/index.html` in browser for combined report.

---

## Performance Validation Workflow

**Full Phase 1 validation sequence:**

1. **Prep (before task-16 completes):**
   - Benchmarks already written ✅
   - Tools ready to use ✅

2. **Benchmarks (Day 1):**
   ```powershell
   # Run all benchmarks, establish baseline
   cargo bench
   ```

3. **FPS Testing (Day 1):**
   - Launch master with local UI
   - Generate continuous output (cat large_file.txt)
   - Monitor FPS counter in egui status bar
   - Target: >= 58 FPS sustained

4. **Latency Testing (Day 1):**
   ```powershell
   # Terminal 1: Launch master
   .\target\release\monoterminal.exe --listen 127.0.0.1:5000

   # Browser: Open tools/latency-measurement.html
   # Run 100-sample test
   ```

5. **Soak Test (Days 2-4):**
   ```powershell
   # Terminal 1: Launch master
   .\target\release\monoterminal.exe

   # Terminal 2: Start monitor
   .\tools\soak-test-monitor.ps1 -DurationHours 24
   ```

6. **Report (Day 5):**
   - Compile all results into `docs/performance-report-phase1.md`
   - Handoff to qa-lead (task-19)

---

## Troubleshooting

### Latency Tool Not Connecting

**Symptom:** WebSocket connection fails  
**Fix:**
1. Verify master is running: `Get-Process monoterminal`
2. Check port: `netstat -an | findstr 5000`
3. Verify WebSocket URL matches master listen address
4. Check browser console for errors (F12)

### Soak Test Monitor Errors

**Symptom:** "Process not found"  
**Fix:**
1. Ensure process name matches: `Get-Process monoterminal`
2. Launch master before starting monitor
3. If process name differs, use `-ProcessName "actual-name"`

**Symptom:** CSV not created  
**Fix:**
1. Check write permissions in current directory
2. Specify full path: `-OutputCsv "C:\path\to\results.csv"`

### Benchmark Compilation Errors

**Symptom:** `criterion` not found  
**Fix:**
```powershell
# Verify criterion in workspace dependencies
cargo update
cargo bench --bench codec
```

**Symptom:** Protobuf errors  
**Fix:**
```powershell
# Rebuild protocol crate
cd crates\protocol
cargo clean
cargo build
cargo bench
```

---

## References

- Performance Validation Plan: `docs/performance-validation-plan.md`
- SRS §7.1: Phase 1 acceptance criteria
- SRS §5.1: Performance targets
- SRS §6.1: Benchmark requirements

---

**Maintained by:** performance-engineer  
**Last Updated:** 2026-08-15
