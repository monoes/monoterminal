# Cross-Platform Memory Profiling Methodology

**Phase:** 3 Week 7 Day 2  
**Task:** task-64  
**Engineer:** performance-engineer  
**Date:** 2026-08-20

---

## Overview

This document defines the methodology for measuring and comparing memory usage across Windows, Linux, and macOS platforms during Phase 3 expansion.

**SRS Memory Targets (§1.3, §5.1.1):**
- 1000 concurrent sessions: ≤7GB total memory
- Linear memory growth (validated Phase 2: 0% overhead)
- No memory leaks (validated Phase 2: 5.5% growth over 30 min acceptable)

---

## Memory Profiling Categories

### 1. Idle Daemon Footprint

**Measurement:** Base memory usage with zero sessions

**Methodology:**
1. Start master daemon (no sessions)
2. Wait for stabilization (2-3 minutes)
3. Measure resident memory (RSS)
4. Measure virtual memory (VSZ/VIRT)
5. Sample every 30 seconds for 5 minutes
6. Report: min, max, mean, std dev

**Tools:**
- **Windows:** Task Manager, PowerShell `Get-Process`, Windows Performance Analyzer
- **Linux:** `ps`, `/proc/[pid]/status`, `pmap`, `valgrind --tool=massif`
- **macOS:** Activity Monitor, `ps`, Instruments (Memory Profiler)

**Expected Range:**
- Idle daemon: 50-150 MB RSS (rough estimate, to be validated)
- Breakdown: Rust runtime, SQLite, wgpu initialization, network stack

### 2. Per-Session Memory Overhead

**Measurement:** Incremental memory cost per session

**Methodology:**
1. Measure baseline (idle daemon)
2. Create 100 sessions sequentially
3. Measure memory after each batch of 10 sessions
4. Calculate: `overhead_per_session = (memory_100 - memory_0) / 100`
5. Verify linear growth (no exponential/quadratic patterns)

**Data Points:**
- 0 sessions (baseline)
- 10 sessions
- 50 sessions
- 100 sessions

**Expected:**
- Per-session overhead: 1-5 MB (PTY + scrollback buffer + metadata)
- Linear growth: R² > 0.95 (validated Phase 2: 1.0)

### 3. 1000-Session Validation

**Measurement:** Total memory at SRS ultimate capacity target

**Reference:** Phase 2 task-48 validated 1000 sessions on Windows with:
- Sessions created: 1000/1000 (100% success)
- Memory growth: 1000 sessions (0% overhead)
- Linear scaling confirmed

**Methodology:**
1. Use existing Phase 2 evidence as Windows baseline
2. Replicate on Linux/macOS (when available)
3. Compare absolute memory usage across platforms
4. Verify <7GB SRS target

**Acceptance Criteria:**
- Total memory ≤7GB (SRS §5.1.1)
- Linear growth maintained (no quadratic scaling)
- Platform parity: <20% variance acceptable

### 4. Memory Leak Detection

**Measurement:** Long-running stability test

**Reference:** Phase 2 task-31 validated:
- 30-minute soak test: 5.5% memory growth (acceptable)
- Conclusion: Minor growth attributed to scrollback buffering, not leaks

**Methodology:**
1. Start daemon with 100 active sessions
2. Simulate continuous PTY I/O (100 lines/sec per session)
3. Measure memory every 5 minutes for 30 minutes
4. Calculate growth rate: `(final - initial) / initial * 100%`
5. Acceptable: <10% growth over 30 minutes

**Tools:**
- **Windows:** Windows Performance Recorder (WPR) + Windows Performance Analyzer (WPA)
- **Linux:** `valgrind --tool=massif`, `heaptrack`, `perf mem`
- **macOS:** Instruments (Allocations, Leaks)

---

## Measurement Tools

### Windows

**PowerShell Script:**
```powershell
# Get-ProcessMemory.ps1
param(
    [string]$ProcessName = "monoterminal",
    [int]$IntervalSeconds = 30,
    [int]$DurationMinutes = 5
)

$samples = @()
$iterations = ($DurationMinutes * 60) / $IntervalSeconds

for ($i = 0; $i -lt $iterations; $i++) {
    $process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
    
    if ($process) {
        $sample = [PSCustomObject]@{
            Timestamp = Get-Date
            WorkingSetMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
            PrivateBytesMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
            VirtualMemoryMB = [math]::Round($process.VirtualMemorySize64 / 1MB, 2)
        }
        
        $samples += $sample
        Write-Host "$($sample.Timestamp): WS=$($sample.WorkingSetMB)MB Private=$($sample.PrivateBytesMB)MB"
    }
    
    Start-Sleep -Seconds $IntervalSeconds
}

# Calculate statistics
$avgWS = ($samples | Measure-Object -Property WorkingSetMB -Average).Average
$maxWS = ($samples | Measure-Object -Property WorkingSetMB -Maximum).Maximum
$minWS = ($samples | Measure-Object -Property WorkingSetMB -Minimum).Minimum

Write-Host "`n=== Statistics ==="
Write-Host "Working Set: Min=$minWS MB, Max=$maxWS MB, Avg=$avgWS MB"

# Export to CSV
$samples | Export-Csv -Path "memory-profile-$(Get-Date -Format 'yyyyMMdd-HHmmss').csv" -NoTypeInformation
```

**Usage:**
```powershell
# Start daemon first
.\target\release\monoterminal.exe --daemon &

# Run profiling
.\tests\scripts\Get-ProcessMemory.ps1 -ProcessName monoterminal -IntervalSeconds 30 -DurationMinutes 5
```

### Linux

**Bash Script:**
```bash
#!/bin/bash
# monitor_memory.sh

PROCESS_NAME="monoterminal"
INTERVAL=30  # seconds
DURATION=300 # 5 minutes

OUTPUT="memory-profile-$(date +%Y%m%d-%H%M%S).csv"
echo "Timestamp,RSS_MB,VSZ_MB,Shared_MB" > "$OUTPUT"

end=$((SECONDS + DURATION))

while [ $SECONDS -lt $end ]; do
    PID=$(pgrep -x "$PROCESS_NAME" | head -1)
    
    if [ -n "$PID" ]; then
        # Read from /proc/[pid]/status
        RSS=$(grep VmRSS /proc/$PID/status | awk '{print $2}')
        VSZ=$(grep VmSize /proc/$PID/status | awk '{print $2}')
        SHARED=$(grep RssFile /proc/$PID/status | awk '{print $2}')
        
        # Convert KB to MB
        RSS_MB=$(echo "scale=2; $RSS / 1024" | bc)
        VSZ_MB=$(echo "scale=2; $VSZ / 1024" | bc)
        SHARED_MB=$(echo "scale=2; $SHARED / 1024" | bc)
        
        TIMESTAMP=$(date +%Y-%m-%d\ %H:%M:%S)
        echo "$TIMESTAMP,$RSS_MB,$VSZ_MB,$SHARED_MB" >> "$OUTPUT"
        echo "$TIMESTAMP: RSS=${RSS_MB}MB VSZ=${VSZ_MB}MB"
    fi
    
    sleep $INTERVAL
done

# Calculate statistics
tail -n +2 "$OUTPUT" | awk -F',' '{sum+=$2; if(NR==1){min=max=$2} if($2<min){min=$2} if($2>max){max=$2}} END {print "RSS: Min="min"MB, Max="max"MB, Avg="sum/NR"MB"}'
```

**Usage:**
```bash
# Start daemon
./target/release/monoterminal --daemon &

# Run profiling
chmod +x tests/scripts/monitor_memory.sh
./tests/scripts/monitor_memory.sh
```

### macOS

**Same as Linux script** (Unix-based, `/proc` alternative via `ps`)

**Instruments (GUI):**
1. Open Xcode Instruments
2. Select "Allocations" template
3. Target: monoterminal process
4. Record for 5 minutes
5. Export report

---

## Comparison Methodology

### Platform Parity Calculation

**Formula:**
```
variance = ((max_memory - min_memory) / min_memory) * 100%
```

**Example:**
- Windows: 120 MB RSS (idle)
- Linux: 110 MB RSS (idle)
- macOS: 125 MB RSS (idle)
- Variance: ((125 - 110) / 110) * 100% = **13.6%** ✅ PASS (<20%)

**Acceptance:** <20% variance between platforms

### Per-Session Overhead Comparison

| Platform | Idle (MB) | 100 Sessions (MB) | Per-Session (MB) | Variance |
|----------|-----------|-------------------|------------------|----------|
| Windows  | [TBD]     | [TBD]             | [TBD]            | [TBD]    |
| Linux    | [TBD]     | [TBD]             | [TBD]            | [TBD]    |
| macOS    | [TBD]     | [TBD]             | [TBD]            | [TBD]    |

---

## Expected Results (Predictions)

### Platform-Specific Characteristics

**Windows (ConPTY):**
- Idle daemon: ~120 MB RSS (Rust runtime + wgpu + SQLite)
- Per-session: ~2-3 MB (ConPTY handle + scrollback buffer)
- 100 sessions: ~320 MB RSS
- 1000 sessions: ~2.2 GB RSS (well under 7GB target)

**Linux (portable-pty):**
- Idle daemon: ~100 MB RSS (slightly lower than Windows, no DirectX overhead)
- Per-session: ~1.5-2 MB (openpty + scrollback buffer)
- 100 sessions: ~280 MB RSS
- 1000 sessions: ~1.8 GB RSS

**macOS (portable-pty + Metal):**
- Idle daemon: ~110 MB RSS (Metal framework overhead)
- Per-session: ~2 MB (openpty + scrollback buffer)
- 100 sessions: ~300 MB RSS
- 1000 sessions: ~2.0 GB RSS

**Variance Prediction:** 10-15% between platforms (within acceptable <20%)

---

## Integration with Existing Evidence

### Phase 2 Baseline (Windows)

**task-48 (1000-session stress test):**
- Sessions: 1000/1000 (100% success)
- Memory growth: 0% overhead (perfect linear)
- Conclusion: Architecture scales to 1000 sessions

**task-31 (30-min soak test):**
- Memory growth: 5.5% over 30 minutes
- Conclusion: Acceptable, attributed to scrollback buffering

**Phase 3 Goal:**
- Replicate these tests on Linux/macOS
- Validate platform parity
- Confirm <7GB at 1000 sessions

---

## Deliverables

### Memory Profiling Report

**Format:** `MEMORY-PROFILING-REPORT.md`

**Contents:**
1. Idle daemon footprint (all platforms)
2. Per-session overhead (all platforms)
3. 1000-session validation (reference Phase 2 Windows)
4. Platform comparison matrix
5. Variance analysis
6. Bottleneck identification (if any)
7. Optimization recommendations

**Timeline:** Day 2-3 (after profiling execution)

---

## Execution Schedule

### Day 2 (Today)

**Windows Profiling:**
1. Idle daemon measurement (5 min)
2. 100-session overhead (10 min)
3. Stability check (30 min soak test)

**Expected:** 1-2 hours total

### Day 3-4 (Linux/macOS)

**CI Execution or Manual:**
- Same methodology
- Compare vs Windows baseline
- Populate comparison matrix

**Expected:** 2-3 hours (parallel with other profiling)

---

## Next Steps

**Immediate:**
1. Create PowerShell profiling script (`Get-ProcessMemory.ps1`)
2. Start daemon and measure idle footprint
3. Create 100 sessions and measure overhead
4. Document results in comparison matrix

**Validation:**
- Compare vs Phase 2 baseline (1000 sessions)
- Verify <7GB SRS target
- Confirm linear growth

---

**Status:** Methodology defined, ready for execution

**Updated:** 2026-08-20  
**Engineer:** performance-engineer
