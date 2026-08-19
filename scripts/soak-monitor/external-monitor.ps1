# MONOTERMINAL 24-Hour Soak Test - External Monitor
#
# Purpose: Independent monitoring that runs OUTSIDE the test process
# - Detects process crashes (daemon death)
# - Collects system-level metrics
# - Tracks network connections
# - Correlates with Windows Event Log
# - Alerts on anomalies
#
# Usage:
#   .\external-monitor.ps1 -ProcessName "monoterminal" -Duration 24 -AlertThreshold 8.0

param(
    [Parameter(Mandatory=$false)]
    [string]$ProcessName = "monoterminal",

    [Parameter(Mandatory=$false)]
    [int]$DurationHours = 24,

    [Parameter(Mandatory=$false)]
    [int]$SampleIntervalSeconds = 60,  # 1-minute samples

    [Parameter(Mandatory=$false)]
    [double]$MemoryGrowthAlertPercent = 8.0,  # Alert before test fails at 10%

    [Parameter(Mandatory=$false)]
    [string]$OutputDir = ".\soak-results",

    [Parameter(Mandatory=$false)]
    [string]$AlertEmail = "",  # Optional: email for critical alerts

    [Parameter(Mandatory=$false)]
    [switch]$EnableCrashDumps
)

# Ensure output directory exists
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$metricsFile = Join-Path $OutputDir "external-metrics-$timestamp.csv"
$alertsFile = Join-Path $OutputDir "alerts-$timestamp.log"
$eventsFile = Join-Path $OutputDir "system-events-$timestamp.log"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " MONOTERMINAL 24h Soak - External Monitor" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Process:       $ProcessName"
Write-Host "Duration:      $DurationHours hours"
Write-Host "Sample Rate:   ${SampleIntervalSeconds}s"
Write-Host "Alert Threshold: ${MemoryGrowthAlertPercent}%"
Write-Host "Output Dir:    $OutputDir"
Write-Host "========================================`n"

# Configure crash dump collection (Windows Error Reporting)
if ($EnableCrashDumps) {
    Write-Host "[SETUP] Enabling crash dump collection..." -ForegroundColor Yellow

    $dumpDir = Join-Path $OutputDir "crash-dumps"
    New-Item -ItemType Directory -Force -Path $dumpDir | Out-Null

    # Configure WER LocalDumps for the process
    $werKey = "HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\$ProcessName.exe"
    if (-not (Test-Path $werKey)) {
        New-Item -Path $werKey -Force | Out-Null
    }
    Set-ItemProperty -Path $werKey -Name "DumpFolder" -Value $dumpDir -Type ExpandString
    Set-ItemProperty -Path $werKey -Name "DumpType" -Value 2 -Type DWord  # Full dump
    Set-ItemProperty -Path $werKey -Name "DumpCount" -Value 10 -Type DWord

    Write-Host "[SETUP] Crash dumps will be saved to: $dumpDir" -ForegroundColor Green
}

# Initialize CSV with headers
$csvHeaders = "Timestamp,ElapsedHours,ProcessExists,PID,CPU%,WorkingSetMB,PrivateBytesMB,HandleCount,ThreadCount,TCPConnections,WebSocketConnections,PageFaultsDelta,MemoryGrowth%"
Set-Content -Path $metricsFile -Value $csvHeaders

# Alert helper
function Write-Alert {
    param([string]$Level, [string]$Message)

    $alertMsg = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] [$Level] $Message"
    Add-Content -Path $alertsFile -Value $alertMsg

    $color = switch ($Level) {
        "CRITICAL" { "Red" }
        "WARNING"  { "Yellow" }
        "INFO"     { "Cyan" }
        default    { "White" }
    }

    Write-Host $alertMsg -ForegroundColor $color

    # Email alert for critical issues (if configured)
    if ($Level -eq "CRITICAL" -and $AlertEmail) {
        # TODO: Implement email alerting (Send-MailMessage or webhook)
        Write-Host "[EMAIL] Would send alert to $AlertEmail" -ForegroundColor Magenta
    }
}

# Get baseline metrics
Write-Host "[BASELINE] Waiting for process to start..." -ForegroundColor Yellow

$process = $null
$startTime = Get-Date
$timeout = (Get-Date).AddMinutes(5)  # 5-minute timeout to find process

while ($null -eq $process -and (Get-Date) -lt $timeout) {
    $process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $process) {
        Start-Sleep -Seconds 2
    }
}

if ($null -eq $process) {
    Write-Alert "CRITICAL" "Process '$ProcessName' not found within 5 minutes. Exiting."
    exit 1
}

$baselinePID = $process.Id
$baselineMemoryMB = $process.WorkingSet64 / 1MB
$baselineHandles = $process.HandleCount
$previousPageFaults = $process.PagedMemorySize64

Write-Host "`n[BASELINE] Process found!" -ForegroundColor Green
Write-Host "  PID:          $baselinePID"
Write-Host "  Memory (WS):  $([math]::Round($baselineMemoryMB, 2)) MB"
Write-Host "  Handles:      $baselineHandles"
Write-Host "`n[MONITORING] Starting external monitoring...`n" -ForegroundColor Green

Write-Alert "INFO" "Monitoring started for PID $baselinePID (baseline memory: $([math]::Round($baselineMemoryMB, 2)) MB)"

# Main monitoring loop
$endTime = $startTime.AddHours($DurationHours)
$lastCPUTime = $process.TotalProcessorTime
$lastSampleTime = Get-Date

while ((Get-Date) -lt $endTime) {
    Start-Sleep -Seconds $SampleIntervalSeconds

    $now = Get-Date
    $elapsed = ($now - $startTime).TotalHours
    $elapsedFormatted = [math]::Round($elapsed, 2)

    # Check if process still exists
    $process = Get-Process -Id $baselinePID -ErrorAction SilentlyContinue

    if ($null -eq $process) {
        Write-Alert "CRITICAL" "Process $ProcessName (PID $baselinePID) has CRASHED or exited unexpectedly!"
        Write-Alert "CRITICAL" "Test FAILED at $elapsedFormatted hours"

        # Collect system event logs around crash time
        $crashTime = Get-Date
        $eventStart = $crashTime.AddMinutes(-5)
        Get-WinEvent -FilterHashtable @{
            LogName='Application','System'
            Level=1,2,3  # Critical, Error, Warning
            StartTime=$eventStart
        } -ErrorAction SilentlyContinue |
            Format-Table TimeCreated, LevelDisplayName, ProviderName, Message -AutoSize |
            Out-File -FilePath $eventsFile -Append

        exit 1
    }

    # Refresh process info
    $process.Refresh()

    # Calculate CPU usage
    $currentTime = Get-Date
    $timeDelta = ($currentTime - $lastSampleTime).TotalSeconds
    $cpuDelta = ($process.TotalProcessorTime - $lastCPUTime).TotalSeconds
    $cpuPercent = [math]::Round(($cpuDelta / $timeDelta / $env:NUMBER_OF_PROCESSORS) * 100, 2)

    $lastCPUTime = $process.TotalProcessorTime
    $lastSampleTime = $currentTime

    # Memory metrics
    $workingSetMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
    $privateBytesMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
    $handleCount = $process.HandleCount
    $threadCount = $process.Threads.Count

    # Page faults delta (indicator of memory thrashing)
    $currentPageFaults = $process.PagedMemorySize64
    $pageFaultsDelta = $currentPageFaults - $previousPageFaults
    $previousPageFaults = $currentPageFaults

    # Memory growth calculation
    $memoryGrowth = [math]::Round((($workingSetMB - $baselineMemoryMB) / $baselineMemoryMB) * 100, 2)

    # Network connections
    $tcpConnections = (Get-NetTCPConnection -OwningProcess $baselinePID -ErrorAction SilentlyContinue | Measure-Object).Count

    # WebSocket connections (port 7777 per soak test infrastructure)
    $wsConnections = (Get-NetTCPConnection -OwningProcess $baselinePID -LocalPort 7777 -State Established -ErrorAction SilentlyContinue | Measure-Object).Count

    # Write to CSV
    $csvRow = "$now,$elapsedFormatted,True,$baselinePID,$cpuPercent,$workingSetMB,$privateBytesMB,$handleCount,$threadCount,$tcpConnections,$wsConnections,$pageFaultsDelta,$memoryGrowth"
    Add-Content -Path $metricsFile -Value $csvRow

    # Console output (every 5 minutes)
    if ([math]::Floor($elapsed * 12) % 5 -eq 0) {  # Every 5 minutes
        Write-Host ("[{0:0.0}h / {1}h] Memory: {2} MB ({3:+0.0;-0.0}%) | Handles: {4} | Threads: {5} | WS Conns: {6}" -f
            $elapsed, $DurationHours, $workingSetMB, $memoryGrowth, $handleCount, $threadCount, $wsConnections)
    }

    # === ALERTING LOGIC ===

    # Alert: Memory growth exceeds threshold
    if ($memoryGrowth -gt $MemoryGrowthAlertPercent) {
        Write-Alert "WARNING" "Memory growth ${memoryGrowth}% exceeds threshold ${MemoryGrowthAlertPercent}% (WS: $workingSetMB MB)"
    }

    # Alert: Handle count anomaly (>50% increase from baseline)
    $handleGrowth = (($handleCount - $baselineHandles) / $baselineHandles) * 100
    if ($handleGrowth -gt 50) {
        Write-Alert "WARNING" "Handle count increased by ${handleGrowth}% (current: $handleCount, baseline: $baselineHandles)"
    }

    # Alert: High CPU usage (sustained >80%)
    if ($cpuPercent -gt 80) {
        Write-Alert "WARNING" "High CPU usage: ${cpuPercent}%"
    }

    # Alert: WebSocket connection drops
    if ($wsConnections -eq 0 -and $elapsed -gt 0.1) {  # After first 6 minutes
        Write-Alert "INFO" "No active WebSocket connections detected"
    }
}

# Test completed successfully
Write-Host "`n========================================" -ForegroundColor Green
Write-Host " Monitoring Complete - Test PASSED" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green

$finalProcess = Get-Process -Id $baselinePID -ErrorAction SilentlyContinue
if ($null -ne $finalProcess) {
    $finalMemoryMB = [math]::Round($finalProcess.WorkingSet64 / 1MB, 2)
    $finalGrowth = [math]::Round((($finalMemoryMB - $baselineMemoryMB) / $baselineMemoryMB) * 100, 2)

    Write-Host "Final Stats:"
    Write-Host "  Memory (WS): $finalMemoryMB MB (growth: ${finalGrowth}%)"
    Write-Host "  Handles:     $($finalProcess.HandleCount)"
    Write-Host "  Threads:     $($finalProcess.Threads.Count)"
}

Write-Host "`nResults saved to:"
Write-Host "  Metrics:  $metricsFile" -ForegroundColor Cyan
Write-Host "  Alerts:   $alertsFile" -ForegroundColor Cyan
Write-Host "  Events:   $eventsFile" -ForegroundColor Cyan

Write-Alert "INFO" "Monitoring completed successfully after $DurationHours hours"

# Cleanup WER configuration
if ($EnableCrashDumps) {
    Remove-Item -Path "HKLM:\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\$ProcessName.exe" -Recurse -Force -ErrorAction SilentlyContinue
}

exit 0
