#!/usr/bin/env pwsh
<#
.SYNOPSIS
    24-hour soak test execution for Phase 2 gate Criterion #7 validation

.DESCRIPTION
    Executes the stability_24h Rust test with extended duration and checkpoint monitoring.
    Adapted from task-31 (30-minute test) for full 24-hour validation.

.PARAMETER DurationHours
    Test duration in hours (default: 24)

.PARAMETER OutputIntervalMinutes
    Checkpoint reporting interval in minutes (default: 60)

.EXAMPLE
    .\soak-test.ps1 -DurationHours 24 -OutputIntervalMinutes 60
#>

param(
    [double]$DurationHours = 24,
    [int]$OutputIntervalMinutes = 60
)

$ErrorActionPreference = "Continue"

# Configuration
$SessionCount = 10
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$EvidenceDir = Join-Path $ProjectRoot "evidence\soak-test"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runDir = Join-Path $EvidenceDir "run-$timestamp"

# Create evidence directory
if (-not (Test-Path $EvidenceDir)) {
    New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
}
if (-not (Test-Path $runDir)) {
    New-Item -ItemType Directory -Path $runDir -Force | Out-Null
}

$logFile = Join-Path $runDir "soak-test-execution.log"
$metricsFile = Join-Path $runDir "memory-metrics.csv"

function Write-Log {
    param([string]$Message)
    $timestamped = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] $Message"
    Write-Host $timestamped
    Add-Content -Path $logFile -Value $timestamped
}

function Get-ProcessMemory {
    param([int]$ProcessId)

    try {
        $process = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
        if ($null -eq $process) {
            return $null
        }

        return @{
            WorkingSetMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
            PrivateBytesMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
            HandleCount = $process.HandleCount
            ThreadCount = $process.Threads.Count
        }
    } catch {
        return $null
    }
}

Write-Log "=========================================="
Write-Log "MONOTERMINAL 24-Hour Soak Test - Phase 2"
Write-Log "=========================================="
Write-Log "Duration: $DurationHours hours"
Write-Log "Session count: $SessionCount"
Write-Log "Checkpoint interval: $OutputIntervalMinutes minutes"
Write-Log "Evidence directory: $runDir"
Write-Log "=========================================="

# Set environment variable for test duration
$env:SOAK_DURATION_HOURS = $DurationHours
$env:SOAK_TEST_MODE = "1"

# Navigate to master crate
$originalLocation = Get-Location
$masterCrateDir = Join-Path $PSScriptRoot "..\crates\master"

if (-not (Test-Path $masterCrateDir)) {
    Write-Log "ERROR: Master crate directory not found: $masterCrateDir"
    exit 1
}

Set-Location $masterCrateDir

Write-Log ""
Write-Log "Starting soak test..."
Write-Log "Expected completion: $((Get-Date).AddHours($DurationHours))"
Write-Log ""

# Launch test in background and capture PID
$testStart = Get-Date
$testOutputFile = Join-Path $runDir "test-output.log"

$job = Start-Job -ScriptBlock {
    param($CrateDir, $DurationHours, $OutputFile)
    Set-Location $CrateDir
    $env:SOAK_DURATION_HOURS = $DurationHours
    $env:SOAK_TEST_MODE = "1"
    cargo test --release --test stability_24h -- --ignored --nocapture 2>&1 | Tee-Object -FilePath $OutputFile
    $LASTEXITCODE
} -ArgumentList $masterCrateDir, $DurationHours, $testOutputFile

Write-Log "Test launched (Job ID: $($job.Id))"
Write-Log "Waiting 30 seconds for test to initialize..."
Start-Sleep -Seconds 30

# Find test process
$testProcess = Get-Process | Where-Object { $_.ProcessName -match 'stability_24h' } | Select-Object -First 1

if ($null -eq $testProcess) {
    Write-Log "ERROR: Test process not found after 30 seconds"
    Stop-Job $job
    Remove-Job $job
    Set-Location $originalLocation
    exit 1
}

Write-Log "Test process found (PID: $($testProcess.Id))"

# Get baseline memory
$baseline = Get-ProcessMemory -ProcessId $testProcess.Id
if ($null -eq $baseline) {
    Write-Log "ERROR: Could not get baseline memory metrics"
    Stop-Job $job
    Remove-Job $job
    Set-Location $originalLocation
    exit 1
}

Write-Log ""
Write-Log "Baseline memory:"
Write-Log "  Working Set:    $($baseline.WorkingSetMB) MB"
Write-Log "  Private Bytes:  $($baseline.PrivateBytesMB) MB"
Write-Log "  Handle Count:   $($baseline.HandleCount)"
Write-Log "  Thread Count:   $($baseline.ThreadCount)"
Write-Log ""

# Initialize metrics CSV
Add-Content -Path $metricsFile -Value "Timestamp,ElapsedMinutes,WorkingSetMB,PrivateBytesMB,HandleCount,ThreadCount,MemoryGrowthPercent"

# Monitoring loop
$checkpointMinutes = $OutputIntervalMinutes
$totalMinutes = $DurationHours * 60
$elapsedMinutes = 0

while ($job.State -eq 'Running' -and $elapsedMinutes -lt $totalMinutes) {
    Start-Sleep -Seconds 60  # Check every minute
    $elapsedMinutes++

    # Get current memory stats
    $current = Get-ProcessMemory -ProcessId $testProcess.Id

    if ($null -ne $current) {
        $growthPercent = [math]::Round((($current.WorkingSetMB - $baseline.WorkingSetMB) / $baseline.WorkingSetMB) * 100, 2)

        # Log to CSV every minute
        $csvLine = "$(Get-Date -Format 'o'),$elapsedMinutes,$($current.WorkingSetMB),$($current.PrivateBytesMB),$($current.HandleCount),$($current.ThreadCount),$growthPercent"
        Add-Content -Path $metricsFile -Value $csvLine

        # Checkpoint reporting
        if ($elapsedMinutes % $checkpointMinutes -eq 0) {
            $hoursElapsed = [math]::Round($elapsedMinutes / 60, 1)
            Write-Log "=========================================="
            Write-Log "CHECKPOINT: Hour $hoursElapsed / $DurationHours"
            Write-Log "=========================================="
            Write-Log "Working Set:    $($current.WorkingSetMB) MB (baseline: $($baseline.WorkingSetMB) MB)"
            Write-Log "Private Bytes:  $($current.PrivateBytesMB) MB (baseline: $($baseline.PrivateBytesMB) MB)"
            Write-Log "Handle Count:   $($current.HandleCount) (baseline: $($baseline.HandleCount))"
            Write-Log "Thread Count:   $($current.ThreadCount) (baseline: $($baseline.ThreadCount))"
            Write-Log "Memory Growth:  $growthPercent%"
            Write-Log "=========================================="

            # Alert if growth exceeds 15%
            if ($growthPercent -gt 15) {
                Write-Log "⚠️  WARNING: Memory growth ($growthPercent%) exceeds 15% threshold"
            }
        }
    }
}

# Wait for job to complete
Write-Log ""
Write-Log "Test runtime complete, waiting for job to finish..."
$job | Wait-Job | Out-Null

$exitCode = Receive-Job -Job $job -ErrorAction SilentlyContinue
Remove-Job $job

$testEnd = Get-Date
$testDuration = $testEnd - $testStart

# Final metrics
$final = Get-ProcessMemory -ProcessId $testProcess.Id
if ($null -eq $final) {
    # Process may have exited, get last known values from CSV
    $lastLine = Get-Content $metricsFile | Select-Object -Last 1
    if ($lastLine) {
        $parts = $lastLine -split ','
        $final = @{
            WorkingSetMB = [double]$parts[2]
            PrivateBytesMB = [double]$parts[3]
            HandleCount = [int]$parts[4]
            ThreadCount = [int]$parts[5]
        }
    }
}

Write-Log ""
Write-Log "=========================================="
Write-Log "SOAK TEST COMPLETE"
Write-Log "=========================================="
Write-Log "Duration:       $($testDuration.TotalHours) hours (requested: $DurationHours)"
Write-Log "Start Time:     $testStart"
Write-Log "End Time:       $testEnd"
Write-Log "Exit Code:      $exitCode"
Write-Log ""

if ($null -ne $final) {
    $finalGrowth = [math]::Round((($final.WorkingSetMB - $baseline.WorkingSetMB) / $baseline.WorkingSetMB) * 100, 2)
    Write-Log "FINAL MEMORY METRICS:"
    Write-Log "  Working Set:    $($final.WorkingSetMB) MB (baseline: $($baseline.WorkingSetMB) MB, growth: $finalGrowth%)"
    Write-Log "  Private Bytes:  $($final.PrivateBytesMB) MB (baseline: $($baseline.PrivateBytesMB) MB)"
    Write-Log "  Handle Count:   $($final.HandleCount) (baseline: $($baseline.HandleCount))"
    Write-Log "  Thread Count:   $($final.ThreadCount) (baseline: $($baseline.ThreadCount))"
    Write-Log ""

    # Verdict
    if ($exitCode -eq 0 -and $finalGrowth -le 10) {
        Write-Log "✅ SOAK TEST PASSED"
        Write-Log "   - Zero crashes"
        Write-Log "   - Memory growth within 10% threshold ($finalGrowth%)"
        $verdict = "PASSED"
    } else {
        Write-Log "❌ SOAK TEST FAILED"
        if ($exitCode -ne 0) {
            Write-Log "   - Test exited with code $exitCode"
        }
        if ($finalGrowth -gt 10) {
            Write-Log "   - Memory growth exceeded 10% threshold ($finalGrowth%)"
        }
        $verdict = "FAILED"
    }
} else {
    Write-Log "⚠️  Could not determine final metrics"
    $verdict = "INCOMPLETE"
}

# Save summary
$summary = @{
    Verdict = $verdict
    DurationRequested = $DurationHours
    DurationActual = $testDuration.TotalHours
    StartTime = $testStart.ToString("o")
    EndTime = $testEnd.ToString("o")
    ExitCode = $exitCode
    BaselineMemoryMB = $baseline.WorkingSetMB
    FinalMemoryMB = if ($final) { $final.WorkingSetMB } else { $null }
    MemoryGrowthPercent = if ($final) { $finalGrowth } else { $null }
    MetricsFile = $metricsFile
    LogFile = $logFile
    TestOutputFile = $testOutputFile
}

$summaryFile = Join-Path $runDir "SUMMARY.json"
$summary | ConvertTo-Json -Depth 5 | Out-File -FilePath $summaryFile

Write-Log "=========================================="
Write-Log "Evidence saved to: $runDir"
Write-Log "Summary: $summaryFile"
Write-Log "Metrics: $metricsFile"
Write-Log "=========================================="

Set-Location $originalLocation

exit $(if ($verdict -eq "PASSED") { 0 } else { 1 })
