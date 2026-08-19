# Enhanced Tracing for Soak Tests

**Status:** ✅ Implemented (2026-08-16)  
**Owner:** sre-observability-engineer

## Overview

The MONOTERMINAL daemon now supports **structured JSON logging** specifically for soak test runs. This provides parseable, timestamped logs for post-test analysis and correlation with external metrics.

## How It Works

### Normal Mode (Default)
```rust
// Compact console output (existing behavior)
tracing_subscriber::fmt()
    .with_target(false)
    .compact()
    .init();
```

**Output:** Human-readable console logs
```
2026-08-16T14:30:00Z INFO  MONOTERMINAL master daemon starting...
2026-08-16T14:30:00Z INFO  Phase 1: Windows + Web client
2026-08-16T14:30:01Z INFO  Ed25519 keypair loaded
```

### Soak Test Mode (SOAK_TEST_MODE=1)
```rust
// JSON structured logs to hourly-rotated files
let file_appender = tracing_appender::rolling::hourly("./soak-logs", "monoterminal.log");
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

tracing_subscriber::fmt()
    .json()
    .with_current_span(false)
    .with_writer(non_blocking)
    .init();
```

**Output:** Machine-parseable JSON in `./soak-logs/monoterminal.log.YYYY-MM-DD-HH`
```json
{"timestamp":"2026-08-16T14:30:00.123456Z","level":"INFO","fields":{"message":"MONOTERMINAL master daemon starting (SOAK TEST MODE - JSON logging enabled)"},"target":"monoterminal"}
{"timestamp":"2026-08-16T14:30:00.234567Z","level":"INFO","fields":{"message":"Phase 1: Windows + Web client"},"target":"monoterminal"}
{"timestamp":"2026-08-16T14:30:01.345678Z","level":"INFO","fields":{"message":"Ed25519 keypair loaded"},"target":"monoterminal"}
```

### Features

**Hourly Log Rotation**
- Files rotate every hour: `monoterminal.log.2026-08-16-14`, `monoterminal.log.2026-08-16-15`, etc.
- Prevents single huge log file (24h run = 24 files max)
- Makes timestamp-based analysis easier

**Non-Blocking I/O**
- Writes happen in background thread
- Zero impact on daemon performance
- Uses `tracing_appender::non_blocking` wrapper

**Structured Fields**
- Timestamp (ISO 8601 with microsecond precision)
- Log level (INFO, WARN, ERROR, DEBUG, TRACE)
- Message text
- Target module (e.g., `monoterminal::server`, `monoterminal::session`)

## Usage

### Automatic (via run-full-soak-test.ps1)

The orchestrator script automatically enables JSON logging:

```powershell
.\run-full-soak-test.ps1 -DurationHours 24
```

Sets `$env:SOAK_TEST_MODE = "1"` before running `cargo test`.

### Manual Activation

If running the test manually:

```powershell
# Set environment variable
$env:SOAK_TEST_MODE = "1"

# Run soak test
cd crates\master
cargo test --release --test stability_24h -- --ignored --nocapture
```

Or one-liner:
```powershell
$env:SOAK_TEST_MODE = "1"; cargo test --release --test stability_24h -- --ignored --nocapture
```

### Verification

Check if JSON logging is active:
```powershell
# Logs should appear in this directory
ls .\soak-logs\

# Example output:
# monoterminal.log.2026-08-16-14
# monoterminal.log.2026-08-16-15
# ...
```

## Log Analysis

### Parse JSON Logs

**PowerShell:**
```powershell
# Count log entries by level
Get-Content .\soak-logs\*.log.* |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Group-Object level |
    Select-Object Name, Count

# Find errors
Get-Content .\soak-logs\*.log.* |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { $_.level -eq "ERROR" } |
    Format-Table timestamp, message -AutoSize
```

**Python (for complex analysis):**
```python
import json
import glob
from collections import Counter

# Load all log files
logs = []
for file in glob.glob("soak-logs/monoterminal.log.*"):
    with open(file) as f:
        for line in f:
            logs.append(json.loads(line))

# Count by level
levels = Counter(log['level'] for log in logs)
print(f"Levels: {levels}")

# Extract errors
errors = [log for log in logs if log['level'] == 'ERROR']
print(f"Errors: {len(errors)}")
for err in errors:
    print(f"  {err['timestamp']}: {err['fields']['message']}")
```

### Correlate with External Metrics

**Scenario:** External monitor shows memory spike at 18:45. Find what happened in logs.

```powershell
# Filter logs by time window
Get-Content .\soak-logs\monoterminal.log.2026-08-16-18 |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { 
        [datetime]::Parse($_.timestamp) -ge "2026-08-16T18:40:00" -and
        [datetime]::Parse($_.timestamp) -le "2026-08-16T18:50:00"
    } |
    Format-Table timestamp, level, message -AutoSize
```

### Timeline Visualization

Convert JSON logs to CSV for Excel/Grafana:

```powershell
Get-Content .\soak-logs\*.log.* |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Select-Object timestamp, level, @{Name='message';Expression={$_.fields.message}}, target |
    Export-Csv -Path "soak-logs-timeline.csv" -NoTypeInformation
```

Import `soak-logs-timeline.csv` into Excel:
- Create pivot table: Rows=Level, Values=Count
- Create timeline chart: X=timestamp, Y=level (color-coded)

## Evidence Collection

The `collect-evidence.ps1` script automatically:

1. **Detects structured logs** in `./soak-logs/`
2. **Copies to evidence package**:
   ```
   evidence-YYYYMMDD-HHMMSS/
   └── structured-logs/
       ├── monoterminal.log.2026-08-16-14
       ├── monoterminal.log.2026-08-16-15
       └── ...
   ```
3. **Includes in summary report** (count of log files)

## Benefits for Postmortems

### 1. Precise Crash Timing
**Without JSON logs:**
- External monitor: "Process crashed at ~18:45"
- No internal daemon state

**With JSON logs:**
```json
{"timestamp":"2026-08-16T18:45:03.123456Z","level":"ERROR","fields":{"message":"PTY backend panic: handle 0x1234 invalid"},"target":"monoterminal::pty"}
```
- Exact crash time to microsecond
- Last logged error before crash
- Module that failed (pty backend)

### 2. Pattern Detection
**Count session operations over time:**
```python
session_creates = [log for log in logs if 'create_session' in log['fields']['message']]
print(f"Sessions created: {len(session_creates)}")

# Group by hour
from datetime import datetime
by_hour = {}
for log in session_creates:
    hour = datetime.fromisoformat(log['timestamp'].replace('Z', '+00:00')).hour
    by_hour[hour] = by_hour.get(hour, 0) + 1

print(f"Sessions per hour: {by_hour}")
```

**Detect anomalies:**
- If hour 18 has 2x more session creates → correlate with memory spike
- If hour 22 has zero → daemon hung?

### 3. Error Burst Detection
**Find error clusters:**
```python
errors = [log for log in logs if log['level'] == 'ERROR']
error_times = [datetime.fromisoformat(e['timestamp'].replace('Z', '+00:00')) for e in errors]

# Find errors within 1-second windows
from itertools import combinations
bursts = []
for t1, t2 in combinations(error_times, 2):
    if abs((t1 - t2).total_seconds()) < 1:
        bursts.append((t1, t2))

print(f"Error bursts: {len(bursts)}")
```

Error burst → Likely cascading failure, not isolated incident.

## Integration with External Monitor

### Correlation Workflow

**Step 1: External monitor detects anomaly**
```csv
# external-metrics-*.csv
Timestamp,MemoryGrowth%,HandleCount
2026-08-16T18:45:00,8.5,1520  # ← WARNING threshold crossed
```

**Step 2: Query JSON logs for same time window**
```powershell
Get-Content .\soak-logs\monoterminal.log.2026-08-16-18 |
    ForEach-Object { $_ | ConvertFrom-Json } |
    Where-Object { 
        [datetime]::Parse($_.timestamp) -ge "2026-08-16T18:44:00" -and
        [datetime]::Parse($_.timestamp) -le "2026-08-16T18:46:00"
    }
```

**Step 3: Find root cause**
```json
{"timestamp":"2026-08-16T18:44:58.123Z","level":"WARN","fields":{"message":"Session cleanup skipped: zombie PTY process detected"},"target":"monoterminal::session"}
{"timestamp":"2026-08-16T18:45:02.456Z","level":"ERROR","fields":{"message":"PTY handle leak: 50 unclosed handles"},"target":"monoterminal::pty"}
```

→ **Root cause:** PTY handle leak caused memory/handle growth.

## Log Retention

**During soak test:**
- Logs accumulate in `./soak-logs/`
- 24-hour test → ~24 files (1 per hour)
- Each file ~10-50 MB (depends on log volume)

**After soak test:**
- `collect-evidence.ps1` copies to `evidence-*/structured-logs/`
- Original `./soak-logs/` can be deleted
- Evidence package is the permanent archive

**Cleanup:**
```powershell
# After evidence collection
Remove-Item -Recurse -Force .\soak-logs
```

(This is safe - evidence package has the copy.)

## Git Ignore

Structured logs are excluded from git:

```gitignore
# .gitignore
soak-logs/
soak-results/
```

Never commit soak test outputs - they're ephemeral test artifacts.

## Troubleshooting

### Logs not appearing in ./soak-logs/

**Check 1:** Is `SOAK_TEST_MODE` set?
```powershell
echo $env:SOAK_TEST_MODE  # Should be "1"
```

**Check 2:** Is daemon actually starting?
```powershell
# Console should show:
# "MONOTERMINAL master daemon starting (SOAK TEST MODE - JSON logging enabled)"
```

**Check 3:** Permission issue?
```powershell
# Ensure current directory is writable
Test-Path .\soak-logs -IsValid
```

### Logs are empty or incomplete

**Cause:** Non-blocking writer not flushed before process exit.

**Fix:** The guard is leaked (`Box::leak(Box::new(_guard))`) to prevent early drop. If logs still incomplete:
- Check daemon shutdown logic (should flush logs)
- Add explicit flush in signal handlers

### JSON parsing errors

**Cause:** Malformed JSON (truncated line, encoding issue)

**Fix:**
```powershell
# Skip invalid JSON lines
Get-Content .\soak-logs\*.log.* |
    ForEach-Object { 
        try { 
            $_ | ConvertFrom-Json 
        } catch { 
            Write-Warning "Skipped invalid JSON: $_" 
        }
    }
```

## Future Enhancements (Phase 2+)

1. **OpenTelemetry Integration**
   - Export traces to Jaeger/Tempo
   - Distributed tracing for P2P sessions

2. **Log Aggregation**
   - Ship logs to Elasticsearch/Loki
   - Real-time dashboard in Grafana

3. **Structured Fields Beyond Message**
   - `session_id`, `connection_id`, `user_id`
   - Enables log filtering by session

4. **Custom Formatters**
   - Add context: PID, thread ID, hostname
   - Useful for distributed deployments

## References

- **tracing-subscriber docs:** https://docs.rs/tracing-subscriber/latest/tracing_subscriber/
- **tracing-appender docs:** https://docs.rs/tracing-appender/latest/tracing_appender/
- **SRS §9.5:** Observability requirements (structured logging via `tracing` crate)

---

**Last updated:** 2026-08-16  
**Status:** Ready for use in 24h soak test
