# Memory Smoke Test Script
# Task-7: Validates AbortOnDrop memory leak fix
# Target: <1% working set growth over 60 minutes (vs previous 52.1% leak)

param(
    [int]$DurationMinutes = 60,
    [int]$SampleIntervalMinutes = 5
)

Write-Host "MONOTERMINAL Memory Smoke Test" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Duration: $DurationMinutes minutes"
Write-Host "Sample interval: $SampleIntervalMinutes minutes"
Write-Host "Samples: $($DurationMinutes / $SampleIntervalMinutes + 1)" # +1 for baseline
Write-Host ""

# Start server in background
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Starting monoterminal server..." -ForegroundColor Yellow
$env:SOAK_TEST_MODE = "1"
$serverProcess = Start-Process -FilePath ".\target\debug\monoterminal.exe" `
    -ArgumentList "--dev-mode --bind-addr 127.0.0.1:18080" `
    -NoNewWindow `
    -PassThru `
    -RedirectStandardOutput ".\soak-logs\stdout.log" `
    -RedirectStandardError ".\soak-logs\stderr.log"

if (-not $serverProcess) {
    Write-Host "ERROR: Failed to start server" -ForegroundColor Red
    exit 1
}

Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Server started (PID: $($serverProcess.Id))" -ForegroundColor Green

# Wait for server to initialize
Start-Sleep -Seconds 5

# Check if process is still running
if ($serverProcess.HasExited) {
    Write-Host "ERROR: Server exited immediately (exit code: $($serverProcess.ExitCode))" -ForegroundColor Red
    exit 1
}

# Baseline measurement
Write-Host ""
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Collecting baseline metrics (t=0)..." -ForegroundColor Yellow
$process = Get-Process -Id $serverProcess.Id
$baseline = @{
    Timestamp = Get-Date
    WorkingSet = $process.WorkingSet64
    PrivateMemory = $process.PrivateMemorySize64
    HandleCount = $process.HandleCount
    ThreadCount = $process.Threads.Count
}

Write-Host ""
Write-Host "Baseline Metrics (t=0):" -ForegroundColor Cyan
Write-Host "  Working Set:     $([math]::Round($baseline.WorkingSet / 1MB, 2)) MB"
Write-Host "  Private Memory:  $([math]::Round($baseline.PrivateMemory / 1MB, 2)) MB"
Write-Host "  Handle Count:    $($baseline.HandleCount)"
Write-Host "  Thread Count:    $($baseline.ThreadCount)"
Write-Host ""

# Sample collection loop
$samples = @($baseline)
$sampleCount = [math]::Floor($DurationMinutes / $SampleIntervalMinutes)

Write-Host "Monitoring for $DurationMinutes minutes..." -ForegroundColor Yellow
Write-Host "(Press Ctrl+C to abort early - server will be terminated gracefully)" -ForegroundColor Gray
Write-Host ""

for ($i = 1; $i -le $sampleCount; $i++) {
    $elapsed = $i * $SampleIntervalMinutes
    $sleepSeconds = $SampleIntervalMinutes * 60

    # Wait for next sample interval
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Waiting $SampleIntervalMinutes minutes until t=$elapsed..." -ForegroundColor Gray
    Start-Sleep -Seconds $sleepSeconds

    # Check if process still running
    if ($serverProcess.HasExited) {
        Write-Host ""
        Write-Host "FAILURE: Server crashed at t=$elapsed minutes (exit code: $($serverProcess.ExitCode))" -ForegroundColor Red
        exit 1
    }

    # Collect sample
    $process = Get-Process -Id $serverProcess.Id
    $sample = @{
        Timestamp = Get-Date
        ElapsedMinutes = $elapsed
        WorkingSet = $process.WorkingSet64
        PrivateMemory = $process.PrivateMemorySize64
        HandleCount = $process.HandleCount
        ThreadCount = $process.Threads.Count
    }
    $samples += $sample

    # Calculate deltas
    $wsGrowth = (($sample.WorkingSet - $baseline.WorkingSet) / $baseline.WorkingSet) * 100
    $pmGrowth = (($sample.PrivateMemory - $baseline.PrivateMemory) / $baseline.PrivateMemory) * 100
    $handleDelta = $sample.HandleCount - $baseline.HandleCount

    Write-Host ""
    Write-Host "Sample #$i (t=$elapsed min):" -ForegroundColor Cyan
    Write-Host "  Working Set:     $([math]::Round($sample.WorkingSet / 1MB, 2)) MB ($([math]::Round($wsGrowth, 2))% growth)"
    Write-Host "  Private Memory:  $([math]::Round($sample.PrivateMemory / 1MB, 2)) MB ($([math]::Round($pmGrowth, 2))% growth)"
    Write-Host "  Handle Count:    $($sample.HandleCount) ($handleDelta delta)"
    Write-Host "  Thread Count:    $($sample.ThreadCount)"

    # Flag anomalies
    if ($wsGrowth -gt 5.0) {
        Write-Host "  WARNING: Working set growth >5% (actual: $([math]::Round($wsGrowth, 2))%)" -ForegroundColor Yellow
    }
    if ([math]::Abs($handleDelta) -gt 10) {
        Write-Host "  WARNING: Handle count changed by >10 (actual: $handleDelta)" -ForegroundColor Yellow
    }
    Write-Host ""
}

# Final analysis
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Memory Smoke Test Complete ($DurationMinutes minutes)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Terminate server gracefully
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Terminating server..." -ForegroundColor Yellow
$serverProcess.Kill()
$serverProcess.WaitForExit(5000)
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Server terminated" -ForegroundColor Green
Write-Host ""

# Final metrics
$finalSample = $samples[-1]
$wsGrowthFinal = (($finalSample.WorkingSet - $baseline.WorkingSet) / $baseline.WorkingSet) * 100
$pmGrowthFinal = (($finalSample.PrivateMemory - $baseline.PrivateMemory) / $baseline.PrivateMemory) * 100
$handleDeltaFinal = $finalSample.HandleCount - $baseline.HandleCount

Write-Host "Final Results:" -ForegroundColor Cyan
Write-Host ""
Write-Host "Baseline (t=0):"
Write-Host "  Working Set:     $([math]::Round($baseline.WorkingSet / 1MB, 2)) MB"
Write-Host "  Private Memory:  $([math]::Round($baseline.PrivateMemory / 1MB, 2)) MB"
Write-Host "  Handle Count:    $($baseline.HandleCount)"
Write-Host ""
Write-Host "Final (t=$DurationMinutes min):"
Write-Host "  Working Set:     $([math]::Round($finalSample.WorkingSet / 1MB, 2)) MB"
Write-Host "  Private Memory:  $([math]::Round($finalSample.PrivateMemory / 1MB, 2)) MB"
Write-Host "  Handle Count:    $($finalSample.HandleCount)"
Write-Host ""
Write-Host "Growth:" -ForegroundColor Yellow
Write-Host "  Working Set:     $([math]::Round($wsGrowthFinal, 2))%" -ForegroundColor $(if ($wsGrowthFinal -lt 1.0) { "Green" } elseif ($wsGrowthFinal -lt 5.0) { "Yellow" } else { "Red" })
Write-Host "  Private Memory:  $([math]::Round($pmGrowthFinal, 2))%" -ForegroundColor $(if ($pmGrowthFinal -lt 5.0) { "Green" } else { "Yellow" })
Write-Host "  Handle Count:    $handleDeltaFinal" -ForegroundColor $(if ([math]::Abs($handleDeltaFinal) -le 5) { "Green" } else { "Yellow" })
Write-Host ""

# Pass/fail determination
$passed = $true
$failures = @()

if ($wsGrowthFinal -ge 1.0) {
    $passed = $false
    $failures += "Working set growth >= 1% (actual: $([math]::Round($wsGrowthFinal, 2))%)"
}

if ($pmGrowthFinal -ge 5.0) {
    $passed = $false
    $failures += "Private memory growth >= 5% (actual: $([math]::Round($pmGrowthFinal, 2))%)"
}

if ([math]::Abs($handleDeltaFinal) -gt 5) {
    $passed = $false
    $failures += "Handle count delta > 5 (actual: $handleDeltaFinal)"
}

# Print verdict
Write-Host "========================================" -ForegroundColor Cyan
if ($passed) {
    Write-Host "VERDICT: PASS ✅" -ForegroundColor Green
    Write-Host ""
    Write-Host "Memory leak fix verified successful." -ForegroundColor Green
    Write-Host "Previous leak: 52.1% working set growth" -ForegroundColor Gray
    Write-Host "Current leak: $([math]::Round($wsGrowthFinal, 2))% working set growth" -ForegroundColor Green
    Write-Host ""
    Write-Host "Recommendation: APPROVE task-8 (24h soak test)" -ForegroundColor Green
    exit 0
} else {
    Write-Host "VERDICT: FAIL ❌" -ForegroundColor Red
    Write-Host ""
    Write-Host "Failures:" -ForegroundColor Red
    foreach ($failure in $failures) {
        Write-Host "  - $failure" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "Recommendation: DEBUG further before task-8" -ForegroundColor Red
    exit 1
}
