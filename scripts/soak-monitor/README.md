# MONOTERMINAL 24-Hour Soak Test - Monitoring Suite

**Owner:** sre-observability-engineer  
**Purpose:** External monitoring infrastructure for Phase 1 Criterion #7 (24h stability validation)

## Overview

This monitoring suite provides **independent, external monitoring** that runs in parallel with the soak test process. It detects crashes, resource anomalies, and network issues that the test itself might miss.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  run-full-soak-test.ps1                                 │
│  (Orchestrator)                                         │
└─────────┬──────────────────────────────┬────────────────┘
          │                              │
          ▼                              ▼
┌──────────────────────┐    ┌──────────────────────────┐
│ Cargo Test Process   │    │ External Monitor         │
│ (stability_24h.rs)   │    │ (external-monitor.ps1)   │
│                      │    │                          │
│ • Built-in metrics   │    │ • Process crash detect   │
│ • Memory monitoring  │    │ • System-level metrics   │
│ • PTY lifecycle test │    │ • Network tracking       │
└──────────────────────┘    │ • Event log correlation  │
                            │ • Real-time alerting     │
                            └──────────────────────────┘
                                       │
                                       ▼
                            ┌──────────────────────────┐
                            │ Evidence Collector       │
                            │ (collect-evidence.ps1)   │
                            │                          │
                            │ • Windows Event Logs     │
                            │ • Performance counters   │
                            │ • Network stats          │
                            │ • Crash dumps (if any)   │
                            └──────────────────────────┘
```

## Scripts

### 1. `run-full-soak-test.ps1` (Orchestrator)

**Purpose:** Single entry point for running the full soak test with monitoring.

**Usage:**
```powershell
# Full 24-hour run
.\run-full-soak-test.ps1 -DurationHours 24

# Quick 1-hour validation
.\run-full-soak-test.ps1 -DurationHours 1

# With crash dump collection
.\run-full-soak-test.ps1 -DurationHours 24 -EnableCrashDumps

# With email alerts (optional)
.\run-full-soak-test.ps1 -DurationHours 24 -AlertEmail "oncall@example.com"
```

**What it does:**
1. Starts external monitor in background (PowerShell job)
2. Runs `cargo test --release --test stability_24h`
3. Waits for both to complete
4. Collects evidence
5. Generates summary report

**Output:**
- `soak-results/run-YYYYMMDD-HHMMSS/` (all results for this run)
  - `SUMMARY.json` - Final verdict and key metrics
  - `soak-test-output.log` - Cargo test console output
  - `external-monitor-output.log` - Monitor console output
  - `external-metrics-*.csv` - 1-minute sample data (1440 rows for 24h)
  - `alerts-*.log` - All alerts triggered during run

### 2. `external-monitor.ps1` (Independent Monitor)

**Purpose:** External process monitoring that detects crashes and anomalies.

**Metrics collected (1-minute intervals):**
- **Memory:** Working Set, Private Bytes, Handle Count
- **CPU:** Per-core utilization percentage
- **Network:** TCP connections, WebSocket connections (port 7777)
- **Process health:** Thread count, page faults delta
- **Crash detection:** Immediate alert if process dies

**Alerting thresholds:**
- Memory growth > 8% → **WARNING** (before test fails at 10%)
- Handle growth > 50% → **WARNING** (potential handle leak)
- CPU > 80% sustained → **WARNING** (potential spin loop)
- Process crash → **CRITICAL** (test failure)

**Can run standalone:**
```powershell
.\external-monitor.ps1 -ProcessName "monoterminal" -DurationHours 24
```

### 3. `collect-evidence.ps1` (Post-Test Forensics)

**Purpose:** Automated evidence collection for analysis and postmortems.

**Collects:**
1. **Windows Event Logs** (last 25 hours)
   - Application log (errors/warnings)
   - System log (errors/warnings)
   - Process crashes/hangs
2. **Performance Counters**
   - Process snapshot at collection time
3. **Network Statistics**
   - TCP connection states
   - WebSocket connection count
   - Network interface stats
4. **Disk I/O**
   - Physical disk health
   - Logical volume stats
5. **System Information**
   - OS version, uptime, memory
6. **Crash Dumps** (if any)
   - Auto-detected from WER directories
   - Copied to evidence folder

**Usage:**
```powershell
.\collect-evidence.ps1 -ProcessName "monoterminal" -OutputDir ".\soak-results"
```

## Monitoring Philosophy

### Built-in vs. External

**Built-in (stability_24h.rs):**
- Runs INSIDE the test process
- Tests actual SessionManager APIs (create/send/kill)
- Validates behavior correctness
- **Limitation:** Can't detect process crashes (process death kills the monitor)

**External (external-monitor.ps1):**
- Runs OUTSIDE the test process (separate PowerShell job)
- Survives process crashes
- Correlates with system-level events
- **Limitation:** Can't see internal daemon state

**Both are required** for comprehensive monitoring.

### Alert Strategy

**Alert Levels:**
- **CRITICAL:** Process crash, test failure (immediate escalation)
- **WARNING:** Threshold violations (investigate but not paged)
- **INFO:** Informational events (logged only)

**Philosophy:**
- Alert **before** the test fails (8% memory growth vs. 10% failure threshold)
- Minimize false positives (50% handle growth is abnormal, 10% is noise)
- Capture context around alerts (timestamp + current metrics)

## Integration with Existing Infrastructure

### Tracing Infrastructure

The daemon already uses `tracing` crate (§9.5 in SRS). The external monitor **complements** this by:

1. **Collecting structured logs** → Evidence collection grabs Windows Event Log entries written by `tracing_subscriber::fmt()`
2. **Correlating crashes** → If daemon crashes, monitor captures the moment + system context
3. **Performance baselines** → External metrics provide OS-level view to validate daemon's internal metrics

### Recommended: Enhanced Tracing for Soak Tests

Add to `crates/master/src/main.rs` before soak test runs:

```rust
// For soak tests: structured JSON logging to file
if std::env::var("SOAK_TEST_MODE").is_ok() {
    let file_appender = tracing_appender::rolling::hourly("./soak-logs", "monoterminal.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .json()  // Structured JSON for parsing
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(non_blocking)
        .init();
} else {
    // Normal mode: compact console output
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();
}
```

Then set `$env:SOAK_TEST_MODE=1` before running the test.

## Outputs and Evidence

### What devops-lead will receive

After test completes (or fails), you'll get a `soak-results/run-YYYYMMDD-HHMMSS/` directory with:

1. **SUMMARY.json** - Machine-readable verdict
   ```json
   {
     "TestResult": "PASSED",
     "DurationActual": "24.05h",
     "CriticalAlerts": 0,
     "WarningAlerts": 3,
     "ExternalMetricsCSV": "path/to/metrics.csv"
   }
   ```

2. **external-metrics-*.csv** - 1440 rows (1 per minute for 24h)
   - Timestamp, Memory, CPU, Handles, Network Connections
   - Import into Excel/Grafana for visualization

3. **alerts-*.log** - All alerts with timestamps
   ```
   [2026-08-16 14:32:15] [WARNING] Memory growth 8.2% exceeds threshold 8.0%
   [2026-08-16 18:45:03] [WARNING] Handle count increased by 52% (current: 1520, baseline: 1000)
   ```

4. **evidence-YYYYMMDD-HHMMSS/** - Full forensics package
   - Event logs (CSV format)
   - Performance snapshots (JSON)
   - Network stats (CSV)
   - Crash dumps (if any)

### Analysis Workflow

**For PASSED tests:**
1. Review `SUMMARY.json` → Check WarningAlerts count
2. If warnings > 0 → Review `alerts-*.log` for context
3. Plot `external-metrics-*.csv` → Visualize memory/CPU trends
4. Archive results for baseline comparison

**For FAILED tests:**
1. Check `SUMMARY.json` → See failure reason
2. **Critical step:** Review `alerts-*.log` → Find first CRITICAL alert
3. Correlate timestamp with:
   - `evidence-*/event-log-crashes.csv` (Windows Error Reporting)
   - `evidence-*/event-log-application.csv` (daemon logs)
4. If crash dumps exist → Use WinDbg for stack trace analysis
5. Write postmortem (template in docs/postmortem-template.md if available)

## Acceptance Criterion #7 Validation

**SRS §7.1 Phase 1 Criterion #7:**
- Zero crashes in 24-hour test ✓ (monitor alerts on crash)
- Memory growth ≤ 10% from baseline ✓ (CSV has growth% column)
- No handle leaks (Windows) ✓ (HandleCount tracked)
- No zombie PTY processes ✓ (built-in test checks this)

**Evidence required for sign-off:**
1. `SUMMARY.json` with `"TestResult": "PASSED"`
2. `external-metrics-*.csv` with final `MemoryGrowth%` ≤ 10.0
3. `alerts-*.log` with **zero CRITICAL alerts**
4. `evidence-*/event-log-crashes.csv` is **empty** (no crashes)

## Troubleshooting

### Monitor doesn't detect the process

**Symptom:** "Process 'monoterminal' not found within 5 minutes"

**Fix:**
- Ensure daemon is running before starting monitor
- Or: Adjust timeout in `external-monitor.ps1` (line 118)
- Or: Use orchestrator (`run-full-soak-test.ps1`) which handles timing

### High false-positive alert rate

**Symptom:** Too many WARNING alerts for normal behavior

**Fix:** Adjust thresholds in `external-monitor.ps1`:
```powershell
-MemoryGrowthAlertPercent 8.0   # Default: 8%
```

Or edit line 235-255 in `external-monitor.ps1` to change alert logic.

### Crash dumps not collected

**Symptom:** Process crashed but no `.dmp` files

**Fix:**
1. Use `-EnableCrashDumps` flag (configures Windows Error Reporting)
2. Manually configure WER:
   ```powershell
   $werKey = "HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\monoterminal.exe"
   New-Item -Path $werKey -Force
   Set-ItemProperty -Path $werKey -Name "DumpFolder" -Value "C:\dumps"
   Set-ItemProperty -Path $werKey -Name "DumpType" -Value 2  # Full dump
   ```

## Next Steps (Post-Monitoring Setup)

1. **After task-7 completes** (memory leak validation):
   - Run `.\run-full-soak-test.ps1 -DurationHours 24`
   - Monitor should be fully automated now

2. **For Criterion #7 sign-off:**
   - Provide `SUMMARY.json` + `external-metrics-*.csv` to qa-lead
   - Upload full results to shared drive (or commit to `test-results/` if in .gitignore)

3. **Future enhancements** (Phase 2+):
   - Grafana dashboard for live metrics (push to Prometheus)
   - Slack/email alerting integration (webhook in alert logic)
   - Automated report generation (HTML report with charts)
   - Distributed tracing for P2P sessions (OpenTelemetry)

## Questions?

Contact: **sre-observability-engineer** (this agent's owner)

Related tasks:
- task-7: Memory leak validation (blocks soak test execution)
- task-8: 24h soak test execution (uses this monitoring suite)

---

**Last updated:** 2026-08-16  
**Status:** Ready for use (monitoring scripts complete)
