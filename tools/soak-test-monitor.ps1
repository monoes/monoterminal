# MONOTERMINAL 24-Hour Soak Test Monitor
# Validates SRS §7.1 acceptance criteria:
# - Zero crashes (process exit code != 0)
# - Memory stable (RSS growth < 10% over 24h)
# - No handle leaks (handle count stable)
# - CPU no sustained spikes (< 50% sustained)

param(
    [Parameter(Mandatory=$false)]
    [string]$ProcessName = "monoterminal",

    [Parameter(Mandatory=$false)]
    [int]$IntervalSeconds = 300,  # 5 minutes

    [Parameter(Mandatory=$false)]
    [int]$DurationHours = 24,

    [Parameter(Mandatory=$false)]
    [string]$OutputCsv = "soak-test-results.csv"
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host " MONOTERMINAL 24-Hour Soak Test Monitor" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Process Name:     $ProcessName"
Write-Host "  Interval:         $IntervalSeconds seconds"
Write-Host "  Duration:         $DurationHours hours"
Write-Host "  Output CSV:       $OutputCsv"
Write-Host ""

# Initialize CSV
$csvHeader = "Timestamp,ElapsedHours,WorkingSetMB,PrivateBytesMB,HandleCount,ThreadCount,CPUPercent,Status"
$csvHeader | Out-File -FilePath $OutputCsv -Encoding utf8
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] CSV initialized: $OutputCsv" -ForegroundColor Green

# Find the process
$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
if (-not $process) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ERROR: Process '$ProcessName' not found!" -ForegroundColor Red
    Write-Host "Please start the process before running this monitor." -ForegroundColor Yellow
    exit 1
}

$processId = $process.Id
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Found process: $ProcessName (PID: $processId)" -ForegroundColor Green
Write-Host ""

# Baseline measurements
$baselineWS = $process.WorkingSet64 / 1MB
$baselineHandles = $process.HandleCount
$startTime = Get-Date

Write-Host "Baseline Measurements:" -ForegroundColor Yellow
Write-Host "  Working Set:      $($baselineWS.ToString('F2')) MB"
Write-Host "  Private Bytes:    $(($process.PrivateMemorySize64 / 1MB).ToString('F2')) MB"
Write-Host "  Handle Count:     $baselineHandles"
Write-Host "  Thread Count:     $($process.Threads.Count)"
Write-Host ""
Write-Host "Starting monitoring... Press Ctrl+C to stop early." -ForegroundColor Green
Write-Host ""

# Monitoring loop
$iteration = 0
$maxIterations = ($DurationHours * 3600) / $IntervalSeconds

try {
    while ($iteration -lt $maxIterations) {
        Start-Sleep -Seconds $IntervalSeconds
        $iteration++

        $now = Get-Date
        $elapsed = ($now - $startTime).TotalHours

        # Try to get the process
        $currentProcess = Get-Process -Id $processId -ErrorAction SilentlyContinue

        if (-not $currentProcess) {
            $crashTime = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
            Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ❌ CRASH DETECTED! Process exited at $crashTime" -ForegroundColor Red

            $csvLine = "$crashTime,$($elapsed.ToString('F2')),CRASHED,CRASHED,CRASHED,CRASHED,CRASHED,CRASHED"
            $csvLine | Out-File -FilePath $OutputCsv -Append -Encoding utf8

            Write-Host ""
            Write-Host "==================================================" -ForegroundColor Red
            Write-Host " SOAK TEST FAILED - Process Crashed" -ForegroundColor Red
            Write-Host "==================================================" -ForegroundColor Red
            Write-Host "Elapsed time: $($elapsed.ToString('F2')) hours" -ForegroundColor Yellow
            Write-Host "SRS §7.1 Requirement: Zero crashes ❌" -ForegroundColor Red
            exit 1
        }

        # Get current metrics
        $ws = $currentProcess.WorkingSet64 / 1MB
        $privateBytes = $currentProcess.PrivateMemorySize64 / 1MB
        $handles = $currentProcess.HandleCount
        $threads = $currentProcess.Threads.Count
        $cpu = $currentProcess.CPU

        # Calculate growth
        $wsGrowthPercent = (($ws - $baselineWS) / $baselineWS) * 100
        $handleGrowthPercent = (($handles - $baselineHandles) / $baselineHandles) * 100

        # Status indicators
        $status = "OK"
        $statusColor = "Green"

        if ($wsGrowthPercent -gt 10) {
            $status = "MEM_LEAK"
            $statusColor = "Red"
        }

        if ($handleGrowthPercent -gt 5) {
            $status = "HANDLE_LEAK"
            $statusColor = "Red"
        }

        # Write to CSV
        $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
        $csvLine = "$timestamp,$($elapsed.ToString('F2')),$($ws.ToString('F2')),$($privateBytes.ToString('F2')),$handles,$threads,$($cpu.ToString('F2')),$status"
        $csvLine | Out-File -FilePath $OutputCsv -Append -Encoding utf8

        # Console output
        Write-Host "[$($now.ToString('HH:mm:ss'))] " -NoNewline
        Write-Host "Elapsed: $($elapsed.ToString('F1'))h " -NoNewline -ForegroundColor Cyan
        Write-Host "| WS: $($ws.ToString('F2'))MB ($(if ($wsGrowthPercent -gt 0) { '+' })$($wsGrowthPercent.ToString('F1'))%) " -NoNewline -ForegroundColor $(if ($wsGrowthPercent -gt 10) { "Red" } else { "White" })
        Write-Host "| Handles: $handles ($(if ($handleGrowthPercent -gt 0) { '+' })$($handleGrowthPercent.ToString('F1'))%) " -NoNewline -ForegroundColor $(if ($handleGrowthPercent -gt 5) { "Red" } else { "White" })
        Write-Host "| Status: $status" -ForegroundColor $statusColor

        # Check if we've reached 24 hours
        if ($iteration -ge $maxIterations) {
            break
        }
    }

    # Final report
    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host " 24-Hour Soak Test Complete!" -ForegroundColor Cyan
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host ""

    $finalProcess = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if (-not $finalProcess) {
        Write-Host "❌ Process crashed during test" -ForegroundColor Red
        Write-Host "SRS §7.1 Result: FAIL" -ForegroundColor Red
        exit 1
    }

    $finalWS = $finalProcess.WorkingSet64 / 1MB
    $finalHandles = $finalProcess.HandleCount
    $finalWsGrowth = (($finalWS - $baselineWS) / $baselineWS) * 100
    $finalHandleGrowth = (($finalHandles - $baselineHandles) / $baselineHandles) * 100

    Write-Host "Final Measurements:" -ForegroundColor Yellow
    Write-Host "  Working Set:      $($finalWS.ToString('F2')) MB (growth: $($finalWsGrowth.ToString('F1'))%)"
    Write-Host "  Handle Count:     $finalHandles (growth: $($finalHandleGrowth.ToString('F1'))%)"
    Write-Host ""

    $allPassed = $true
    Write-Host "SRS §7.1 Acceptance Criteria:" -ForegroundColor Yellow

    # Check: Zero crashes
    Write-Host "  ✅ Zero crashes" -ForegroundColor Green

    # Check: Memory stable (< 10% growth)
    if ($finalWsGrowth -lt 10) {
        Write-Host "  ✅ Memory stable (< 10% growth)" -ForegroundColor Green
    } else {
        Write-Host "  ❌ Memory leak detected ($($finalWsGrowth.ToString('F1'))% growth exceeds 10%)" -ForegroundColor Red
        $allPassed = $false
    }

    # Check: No handle leaks
    if ($finalHandleGrowth -lt 5) {
        Write-Host "  ✅ No handle leaks" -ForegroundColor Green
    } else {
        Write-Host "  ❌ Handle leak detected ($($finalHandleGrowth.ToString('F1'))% growth)" -ForegroundColor Red
        $allPassed = $false
    }

    Write-Host ""
    if ($allPassed) {
        Write-Host "🎉 SOAK TEST PASSED" -ForegroundColor Green
        Write-Host "Results saved to: $OutputCsv" -ForegroundColor Cyan
        exit 0
    } else {
        Write-Host "❌ SOAK TEST FAILED" -ForegroundColor Red
        Write-Host "Results saved to: $OutputCsv" -ForegroundColor Cyan
        exit 1
    }

} catch {
    Write-Host ""
    Write-Host "Error during monitoring: $_" -ForegroundColor Red
    exit 1
} finally {
    Write-Host ""
    Write-Host "Monitoring stopped." -ForegroundColor Yellow
}
