# MONOTERMINAL 24-Hour Soak Test - Evidence Collection
#
# Purpose: Automated evidence collection for post-test analysis
# - Windows Event Logs (Application, System)
# - Performance Counter logs
# - Network statistics
# - Disk I/O stats
# - Process crash dumps (if any)
#
# Usage:
#   .\collect-evidence.ps1 -OutputDir ".\soak-results" -ProcessName "monoterminal"

param(
    [Parameter(Mandatory=$false)]
    [string]$ProcessName = "monoterminal",

    [Parameter(Mandatory=$false)]
    [string]$OutputDir = ".\soak-results",

    [Parameter(Mandatory=$false)]
    [int]$EventLogHours = 25  # Collect last 25 hours of events
)

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$evidenceDir = Join-Path $OutputDir "evidence-$timestamp"
New-Item -ItemType Directory -Force -Path $evidenceDir | Out-Null

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Evidence Collection" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Process:       $ProcessName"
Write-Host "Output Dir:    $evidenceDir"
Write-Host "Event Window:  Last $EventLogHours hours"
Write-Host "========================================`n"

# 1. Collect Windows Event Logs
Write-Host "[1/6] Collecting Windows Event Logs..." -ForegroundColor Yellow

$eventStart = (Get-Date).AddHours(-$EventLogHours)

# Application log (errors and warnings)
Get-WinEvent -FilterHashtable @{
    LogName='Application'
    Level=1,2,3  # Critical, Error, Warning
    StartTime=$eventStart
} -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, LevelDisplayName, ProviderName, Id, Message |
    Export-Csv -Path (Join-Path $evidenceDir "event-log-application.csv") -NoTypeInformation

# System log (errors and warnings)
Get-WinEvent -FilterHashtable @{
    LogName='System'
    Level=1,2,3
    StartTime=$eventStart
} -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, LevelDisplayName, ProviderName, Id, Message |
    Export-Csv -Path (Join-Path $evidenceDir "event-log-system.csv") -NoTypeInformation

# Process-specific events (crashes, hangs)
Get-WinEvent -FilterHashtable @{
    LogName='Application'
    ProviderName='Windows Error Reporting','Application Error','Application Hang'
    StartTime=$eventStart
} -ErrorAction SilentlyContinue |
    Select-Object TimeCreated, LevelDisplayName, Message |
    Export-Csv -Path (Join-Path $evidenceDir "event-log-crashes.csv") -NoTypeInformation

Write-Host "  ✓ Event logs saved" -ForegroundColor Green

# 2. Collect Performance Counter History (if available)
Write-Host "[2/6] Collecting Performance Counter data..." -ForegroundColor Yellow

# Get process performance counters
$process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue
if ($null -ne $process) {
    $perfData = @{
        ProcessName = $ProcessName
        PID = $process.Id
        CPU_Time_Seconds = $process.TotalProcessorTime.TotalSeconds
        WorkingSet_MB = [math]::Round($process.WorkingSet64 / 1MB, 2)
        PrivateBytes_MB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
        VirtualMemory_MB = [math]::Round($process.VirtualMemorySize64 / 1MB, 2)
        HandleCount = $process.HandleCount
        ThreadCount = $process.Threads.Count
        StartTime = $process.StartTime
        RunningDuration = (Get-Date) - $process.StartTime
    }

    $perfData | ConvertTo-Json | Out-File (Join-Path $evidenceDir "perf-counters-snapshot.json")
    Write-Host "  ✓ Performance snapshot saved" -ForegroundColor Green
} else {
    Write-Host "  ⚠ Process not running - skipping perf counters" -ForegroundColor Yellow
}

# 3. Collect Network Statistics
Write-Host "[3/6] Collecting network statistics..." -ForegroundColor Yellow

if ($null -ne $process) {
    # TCP connections
    Get-NetTCPConnection -OwningProcess $process.Id -ErrorAction SilentlyContinue |
        Select-Object LocalAddress, LocalPort, RemoteAddress, RemotePort, State, CreationTime |
        Export-Csv -Path (Join-Path $evidenceDir "network-tcp-connections.csv") -NoTypeInformation

    # WebSocket connection statistics (port 7777 per soak test infrastructure)
    $wsStats = Get-NetTCPConnection -OwningProcess $process.Id -LocalPort 7777 -ErrorAction SilentlyContinue |
        Group-Object State |
        Select-Object @{Name='State';Expression={$_.Name}}, Count

    $wsStats | Export-Csv -Path (Join-Path $evidenceDir "network-websocket-stats.csv") -NoTypeInformation

    Write-Host "  ✓ Network stats saved" -ForegroundColor Green
} else {
    Write-Host "  ⚠ Process not running - skipping network stats" -ForegroundColor Yellow
}

# Global network interface statistics
Get-NetAdapterStatistics |
    Select-Object Name, ReceivedBytes, SentBytes, ReceivedUnicastPackets, SentUnicastPackets |
    Export-Csv -Path (Join-Path $evidenceDir "network-interface-stats.csv") -NoTypeInformation

# 4. Collect Disk I/O Statistics
Write-Host "[4/6] Collecting disk I/O statistics..." -ForegroundColor Yellow

Get-PhysicalDisk |
    Select-Object DeviceId, FriendlyName, MediaType, OperationalStatus, HealthStatus, Size |
    Export-Csv -Path (Join-Path $evidenceDir "disk-physical.csv") -NoTypeInformation

# Logical disk stats
Get-Volume |
    Select-Object DriveLetter, FileSystemLabel, FileSystem, Size, SizeRemaining, HealthStatus |
    Export-Csv -Path (Join-Path $evidenceDir "disk-logical.csv") -NoTypeInformation

Write-Host "  ✓ Disk stats saved" -ForegroundColor Green

# 5. Collect System Information
Write-Host "[5/6] Collecting system information..." -ForegroundColor Yellow

$sysInfo = @{
    ComputerName = $env:COMPUTERNAME
    OSVersion = [System.Environment]::OSVersion.VersionString
    ProcessorCount = $env:NUMBER_OF_PROCESSORS
    TotalMemory_GB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB, 2)
    Uptime = (Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime
    PowerShellVersion = $PSVersionTable.PSVersion.ToString()
    Timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
}

$sysInfo | ConvertTo-Json | Out-File (Join-Path $evidenceDir "system-info.json")
Write-Host "  ✓ System info saved" -ForegroundColor Green

# 6. Collect Structured Logs (JSON format from SOAK_TEST_MODE)
Write-Host "[6/7] Collecting structured logs..." -ForegroundColor Yellow

$soakLogsDir = ".\soak-logs"
if (Test-Path $soakLogsDir) {
    $logFiles = Get-ChildItem -Path $soakLogsDir -Filter "monoterminal.log*" -File -ErrorAction SilentlyContinue

    if ($logFiles) {
        $structuredLogsDir = Join-Path $evidenceDir "structured-logs"
        New-Item -ItemType Directory -Force -Path $structuredLogsDir | Out-Null

        $logFiles | ForEach-Object {
            Copy-Item -Path $_.FullName -Destination $structuredLogsDir -Force
            Write-Host "    Collected: $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Cyan
        }

        Write-Host "  ✓ $($logFiles.Count) structured log file(s) collected" -ForegroundColor Green
    } else {
        Write-Host "  ⚠ No structured logs found (SOAK_TEST_MODE may not have been enabled)" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ⚠ soak-logs directory not found - structured logging not used" -ForegroundColor Yellow
}

# 7. Check for Crash Dumps
Write-Host "[7/7] Checking for crash dumps..." -ForegroundColor Yellow

$crashDumpDirs = @(
    "$env:LOCALAPPDATA\CrashDumps",
    "$env:ProgramData\Microsoft\Windows\WER\ReportQueue",
    (Join-Path $OutputDir "crash-dumps")
)

$foundDumps = @()
foreach ($dir in $crashDumpDirs) {
    if (Test-Path $dir) {
        $dumps = Get-ChildItem -Path $dir -Filter "*.dmp" -File -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTime -gt $eventStart }

        if ($dumps) {
            $foundDumps += $dumps

            # Copy to evidence directory
            $dumpEvidenceDir = Join-Path $evidenceDir "crash-dumps"
            New-Item -ItemType Directory -Force -Path $dumpEvidenceDir | Out-Null

            $dumps | ForEach-Object {
                Copy-Item -Path $_.FullName -Destination $dumpEvidenceDir -Force
                Write-Host "    Found: $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Cyan
            }
        }
    }
}

if ($foundDumps.Count -eq 0) {
    Write-Host "  ✓ No crash dumps found (good!)" -ForegroundColor Green
} else {
    Write-Host "  ⚠ $($foundDumps.Count) crash dump(s) found and copied" -ForegroundColor Yellow
}

# Summary Report
Write-Host "`n========================================" -ForegroundColor Green
Write-Host " Evidence Collection Complete" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host "Evidence saved to: $evidenceDir" -ForegroundColor Cyan
Write-Host "`nCollected:"
Write-Host "  • Windows Event Logs (Application, System, Crashes)"
Write-Host "  • Performance Counter snapshot"
Write-Host "  • Network connection statistics"
Write-Host "  • Disk I/O statistics"
Write-Host "  • System information"
Write-Host "  • Structured logs (JSON): $(if (Test-Path (Join-Path $evidenceDir "structured-logs")) { (Get-ChildItem (Join-Path $evidenceDir "structured-logs") | Measure-Object).Count } else { 0 }) file(s)"
Write-Host "  • Crash dumps: $($foundDumps.Count) file(s)"

# Create summary manifest
$manifest = @{
    CollectionTime = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    ProcessName = $ProcessName
    EventLogWindow_Hours = $EventLogHours
    CrashDumpsFound = $foundDumps.Count
    EvidenceDirectory = $evidenceDir
}

$manifest | ConvertTo-Json | Out-File (Join-Path $evidenceDir "evidence-manifest.json")

Write-Host "`nManifest: $(Join-Path $evidenceDir "evidence-manifest.json")" -ForegroundColor Cyan

exit 0
