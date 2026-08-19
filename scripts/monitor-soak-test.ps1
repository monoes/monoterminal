#!/usr/bin/env pwsh
<#
.SYNOPSIS
    External monitoring script for 24h soak test
    Runs independently of the test process for additional observability

.DESCRIPTION
    Monitors system resources during soak test execution:
    - System-wide memory and CPU
    - Network connections (WebSocket stability)
    - Process crashes (event log)
    - Disk I/O
    - Generates timestamped CSV log for analysis

.PARAMETER TestProcessName
    Name of the test process to monitor (default: "stability_24h")

.PARAMETER SampleIntervalSeconds
    Sampling interval in seconds (default: 300 = 5 minutes)

.PARAMETER OutputDir
    Directory for output files (default: .\evidence\soak-monitoring)

.EXAMPLE
    .\monitor-soak-test.ps1 -TestProcessName "stability_24h" -SampleIntervalSeconds 300
#>

param(
    [string]$TestProcessName = "stability_24h",
    [int]$SampleIntervalSeconds = 300,
    [string]$OutputDir = ".\evidence\soak-monitoring"
)

$ErrorActionPreference = "Continue"

# Create output directory
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$csvPath = Join-Path $OutputDir "system-metrics-$timestamp.csv"
$logPath = Join-Path $OutputDir "monitor-log-$timestamp.txt"

function Write-Log {
    param([string]$Message)
    $timestamped = "[$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')] $Message"
    Write-Host $timestamped
    Add-Content -Path $logPath -Value $timestamped
}

function Get-SystemMetrics {
    $metrics = @{
        Timestamp = Get-Date -Format "o"
        TotalMemoryMB = 0
        AvailableMemoryMB = 0
        MemoryUsagePercent = 0
        CPUUsagePercent = 0
        DiskReadBytesPerSec = 0
        DiskWriteBytesPerSec = 0
        NetworkBytesPerSec = 0
        ProcessCount = 0
        ThreadCount = 0
        HandleCount = 0
        WebSocketConnections = 0
    }

    try {
        # Memory
        $mem = Get-CimInstance Win32_OperatingSystem
        $metrics.TotalMemoryMB = [math]::Round($mem.TotalVisibleMemorySize / 1024, 2)
        $metrics.AvailableMemoryMB = [math]::Round($mem.FreePhysicalMemory / 1024, 2)
        $metrics.MemoryUsagePercent = [math]::Round((($mem.TotalVisibleMemorySize - $mem.FreePhysicalMemory) / $mem.TotalVisibleMemorySize) * 100, 2)

        # CPU (average over 5 seconds)
        $cpu = Get-Counter '\Processor(_Total)\% Processor Time' -SampleInterval 5 -MaxSamples 1
        $metrics.CPUUsagePercent = [math]::Round($cpu.CounterSamples[0].CookedValue, 2)

        # Process counts
        $allProcesses = Get-Process
        $metrics.ProcessCount = $allProcesses.Count
        $metrics.ThreadCount = ($allProcesses | Measure-Object -Property Threads -Sum).Sum
        $metrics.HandleCount = ($allProcesses | Measure-Object -Property HandleCount -Sum).Sum

        # Network connections (established TCP connections)
        $connections = Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue
        $metrics.WebSocketConnections = ($connections | Where-Object { $_.LocalPort -eq 7777 }).Count

        # Disk I/O (if counter available)
        try {
            $diskRead = Get-Counter '\PhysicalDisk(_Total)\Disk Read Bytes/sec' -ErrorAction SilentlyContinue
            $diskWrite = Get-Counter '\PhysicalDisk(_Total)\Disk Write Bytes/sec' -ErrorAction SilentlyContinue
            $metrics.DiskReadBytesPerSec = [math]::Round($diskRead.CounterSamples[0].CookedValue, 0)
            $metrics.DiskWriteBytesPerSec = [math]::Round($diskWrite.CounterSamples[0].CookedValue, 0)
        } catch {
            # Counter not available
        }

    } catch {
        Write-Log "WARNING: Failed to collect some metrics: $_"
    }

    return $metrics
}

function Get-RecentCrashes {
    # Check Windows Event Log for application crashes in last 5 minutes
    $since = (Get-Date).AddMinutes(-5)

    try {
        $crashes = Get-WinEvent -FilterHashtable @{
            LogName = 'Application'
            Level = 2  # Error
            StartTime = $since
        } -ErrorAction SilentlyContinue | Where-Object {
            $_.Message -match 'crash|exception|fault|terminated unexpectedly'
        }

        return $crashes.Count
    } catch {
        return 0
    }
}

# Write CSV header
$header = "Timestamp,TotalMemoryMB,AvailableMemoryMB,MemoryUsagePercent,CPUUsagePercent," +
          "DiskReadBytesPerSec,DiskWriteBytesPerSec,NetworkBytesPerSec," +
          "ProcessCount,ThreadCount,HandleCount,WebSocketConnections,RecentCrashes"
Set-Content -Path $csvPath -Value $header

Write-Log "==================================================="
Write-Log "MONOTERMINAL 24h Soak Test - External Monitor"
Write-Log "==================================================="
Write-Log "Test Process: $TestProcessName"
Write-Log "Sample Interval: $SampleIntervalSeconds seconds"
Write-Log "CSV Output: $csvPath"
Write-Log "Log Output: $logPath"
Write-Log "==================================================="
Write-Log ""

$iteration = 0
$startTime = Get-Date

try {
    Write-Log "Starting monitoring... Press Ctrl+C to stop."

    while ($true) {
        $iteration++
        $elapsed = (Get-Date) - $startTime
        $elapsedHours = [math]::Round($elapsed.TotalHours, 2)

        # Collect metrics
        $metrics = Get-SystemMetrics
        $crashes = Get-RecentCrashes

        # Write to CSV
        $row = "$($metrics.Timestamp),$($metrics.TotalMemoryMB),$($metrics.AvailableMemoryMB)," +
               "$($metrics.MemoryUsagePercent),$($metrics.CPUUsagePercent)," +
               "$($metrics.DiskReadBytesPerSec),$($metrics.DiskWriteBytesPerSec),0," +
               "$($metrics.ProcessCount),$($metrics.ThreadCount),$($metrics.HandleCount)," +
               "$($metrics.WebSocketConnections),$crashes"
        Add-Content -Path $csvPath -Value $row

        # Log summary every 10 iterations (50 minutes)
        if ($iteration % 10 -eq 0) {
            Write-Log ("Iteration $iteration (${elapsedHours}h elapsed) - " +
                      "Mem: $($metrics.MemoryUsagePercent)% | " +
                      "CPU: $($metrics.CPUUsagePercent)% | " +
                      "WS Conns: $($metrics.WebSocketConnections) | " +
                      "Crashes: $crashes")
        }

        # Alert on anomalies
        if ($metrics.MemoryUsagePercent -gt 90) {
            Write-Log "ALERT: System memory usage high: $($metrics.MemoryUsagePercent)%"
        }

        if ($crashes -gt 0) {
            Write-Log "ALERT: $crashes recent crashes detected in event log"
        }

        # Sleep until next sample
        Start-Sleep -Seconds $SampleIntervalSeconds
    }

} finally {
    Write-Log ""
    Write-Log "==================================================="
    Write-Log "Monitoring stopped"
    Write-Log "Total iterations: $iteration"
    Write-Log "Total runtime: $elapsedHours hours"
    Write-Log "CSV: $csvPath"
    Write-Log "==================================================="
}
