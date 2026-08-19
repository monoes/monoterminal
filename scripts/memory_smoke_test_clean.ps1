# Memory Smoke Test Script (No I/O Redirection)
# Quick test to rule out PowerShell pipe buffer issues

param(
    [int]$DurationMinutes = 60,
    [int]$SampleIntervalMinutes = 5
)

Write-Host "MONOTERMINAL Memory Smoke Test (No I/O Redirection)" -ForegroundColor Cyan
Write-Host "====================================================" -ForegroundColor Cyan
Write-Host ""

# Start server WITHOUT output redirection
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Starting server..." -ForegroundColor Yellow
$env:SOAK_TEST_MODE = "1"
$serverProcess = Start-Process -FilePath ".\target\debug\monoterminal.exe" `
    -ArgumentList "--dev-mode --bind-addr 127.0.0.1:18080" `
    -NoNewWindow `
    -PassThru

if (-not $serverProcess) {
    Write-Host "ERROR: Failed to start server" -ForegroundColor Red
    exit 1
}

Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Server started (PID: $($serverProcess.Id))" -ForegroundColor Green
Write-Host "NOTE: Server output will appear in this console (no redirection)" -ForegroundColor Gray

# Wait for initialization
Start-Sleep -Seconds 5

if ($serverProcess.HasExited) {
    Write-Host "ERROR: Server exited during startup (exit code: $($serverProcess.ExitCode))" -ForegroundColor Red
    exit 1
}

# Baseline
$process = Get-Process -Id $serverProcess.Id
$baseline = @{
    WorkingSet = $process.WorkingSet64
    PrivateMemory = $process.PrivateMemorySize64
    HandleCount = $process.HandleCount
}

Write-Host ""
Write-Host "Baseline (t=0):" -ForegroundColor Cyan
Write-Host "  Working Set:    $([math]::Round($baseline.WorkingSet / 1MB, 2)) MB"
Write-Host "  Private Memory: $([math]::Round($baseline.PrivateMemory / 1MB, 2)) MB"
Write-Host "  Handle Count:   $($baseline.HandleCount)"
Write-Host ""

# Sample loop
$samples = @()
$sampleCount = [math]::Floor($DurationMinutes / $SampleIntervalMinutes)

for ($i = 1; $i -le $sampleCount; $i++) {
    $elapsed = $i * $SampleIntervalMinutes

    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Waiting $SampleIntervalMinutes minutes (until t=$elapsed)..." -ForegroundColor Gray
    Start-Sleep -Seconds ($SampleIntervalMinutes * 60)

    # Check crash
    if ($serverProcess.HasExited) {
        Write-Host ""
        Write-Host "FAILURE: Server crashed at t=$elapsed minutes" -ForegroundColor Red
        Write-Host "Exit code: $($serverProcess.ExitCode)" -ForegroundColor Red
        exit 1
    }

    # Sample
    $process = Get-Process -Id $serverProcess.Id
    $sample = @{
        ElapsedMinutes = $elapsed
        WorkingSet = $process.WorkingSet64
        PrivateMemory = $process.PrivateMemorySize64
        HandleCount = $process.HandleCount
    }
    $samples += $sample

    $wsGrowth = (($sample.WorkingSet - $baseline.WorkingSet) / $baseline.WorkingSet) * 100
    $pmGrowth = (($sample.PrivateMemory - $baseline.PrivateMemory) / $baseline.PrivateMemory) * 100
    $handleDelta = $sample.HandleCount - $baseline.HandleCount

    Write-Host ""
    Write-Host "Sample #$i (t=$elapsed):" -ForegroundColor Cyan
    Write-Host "  Working Set:    $([math]::Round($sample.WorkingSet / 1MB, 2)) MB ($([math]::Round($wsGrowth, 2))%)"
    Write-Host "  Private Memory: $([math]::Round($sample.PrivateMemory / 1MB, 2)) MB ($([math]::Round($pmGrowth, 2))%)"
    Write-Host "  Handle Count:   $($sample.HandleCount) ($handleDelta delta)"

    if ($wsGrowth -gt 5.0) {
        Write-Host "  WARNING: WS growth >5%" -ForegroundColor Yellow
    }
    if ([math]::Abs($handleDelta) -gt 10) {
        Write-Host "  WARNING: Handle delta >10" -ForegroundColor Yellow
    }
}

# Terminate
Write-Host ""
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Test complete - terminating server..." -ForegroundColor Yellow
$serverProcess.Kill()
$serverProcess.WaitForExit(5000)
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Server terminated" -ForegroundColor Green

# Final analysis
$finalSample = $samples[-1]
$wsGrowthFinal = (($finalSample.WorkingSet - $baseline.WorkingSet) / $baseline.WorkingSet) * 100
$pmGrowthFinal = (($finalSample.PrivateMemory - $baseline.PrivateMemory) / $baseline.PrivateMemory) * 100
$handleDeltaFinal = $finalSample.HandleCount - $baseline.HandleCount

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Final Results:" -ForegroundColor Cyan
Write-Host "  WS Growth:  $([math]::Round($wsGrowthFinal, 2))%" -ForegroundColor $(if ($wsGrowthFinal -lt 1.0) { "Green" } else { "Red" })
Write-Host "  PM Growth:  $([math]::Round($pmGrowthFinal, 2))%" -ForegroundColor $(if ($pmGrowthFinal -lt 5.0) { "Green" } else { "Red" })
Write-Host "  Handle Delta: $handleDeltaFinal" -ForegroundColor $(if ([math]::Abs($handleDeltaFinal) -le 5) { "Green" } else { "Red" })
Write-Host ""

# Verdict
if ($wsGrowthFinal -lt 1.0 -and $pmGrowthFinal -lt 5.0 -and [math]::Abs($handleDeltaFinal) -le 5) {
    Write-Host "VERDICT: PASS [OK]" -ForegroundColor Green
    exit 0
} else {
    Write-Host "VERDICT: FAIL [X]" -ForegroundColor Red
    exit 1
}
