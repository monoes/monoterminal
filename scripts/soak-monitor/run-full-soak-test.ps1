# MONOTERMINAL 24-Hour Soak Test - Full Orchestration Script
#
# Purpose: Coordinates the soak test execution with external monitoring
# - Starts the external monitor in background
# - Runs the soak test (Cargo test)
# - Collects evidence on completion or failure
# - Generates summary report
#
# Usage:
#   .\run-full-soak-test.ps1 -DurationHours 24
#   .\run-full-soak-test.ps1 -DurationHours 1  # Quick validation

param(
    [Parameter(Mandatory=$false)]
    [int]$DurationHours = 24,

    [Parameter(Mandatory=$false)]
    [string]$ProcessName = "monoterminal",

    [Parameter(Mandatory=$false)]
    [string]$OutputDir = ".\soak-results",

    [Parameter(Mandatory=$false)]
    [switch]$EnableCrashDumps,

    [Parameter(Mandatory=$false)]
    [string]$AlertEmail = ""
)

# Ensure cargo is in PATH
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "User") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "Machine")
}

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$runOutputDir = Join-Path $OutputDir "run-$timestamp"
New-Item -ItemType Directory -Force -Path $runOutputDir | Out-Null

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " MONOTERMINAL 24-Hour Soak Test - Full Run" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Duration:      $DurationHours hours"
Write-Host "Process:       $ProcessName"
Write-Host "Output Dir:    $runOutputDir"
Write-Host "Crash Dumps:   $EnableCrashDumps"
Write-Host "Alert Email:   $(if ($AlertEmail) { $AlertEmail } else { 'None' })"
Write-Host "========================================`n"

# Step 1: Start external monitor in background
Write-Host "[STEP 1/4] Starting external monitor..." -ForegroundColor Yellow

$monitorScriptPath = Join-Path $PSScriptRoot "external-monitor.ps1"
if (-not (Test-Path $monitorScriptPath)) {
    Write-Host "ERROR: External monitor script not found: $monitorScriptPath" -ForegroundColor Red
    exit 1
}

$monitorArgs = @{
    ProcessName = $ProcessName
    DurationHours = $DurationHours
    SampleIntervalSeconds = 60
    MemoryGrowthAlertPercent = 8.0
    OutputDir = $runOutputDir
    AlertEmail = $AlertEmail
}

if ($EnableCrashDumps) {
    $monitorArgs['EnableCrashDumps'] = $true
}

$monitorJob = Start-Job -ScriptBlock {
    param($ScriptPath, $Args)
    & $ScriptPath @Args
} -ArgumentList $monitorScriptPath, $monitorArgs

Write-Host "  ✓ External monitor started (Job ID: $($monitorJob.Id))" -ForegroundColor Green
Write-Host "    Monitoring will run in parallel with the soak test`n"

# Step 2: Run the soak test
Write-Host "[STEP 2/4] Running soak test (Cargo test)..." -ForegroundColor Yellow
Write-Host "  This will take approximately $DurationHours hours...`n" -ForegroundColor Cyan

$soakTestStart = Get-Date

# Set environment variables for test
$env:SOAK_DURATION_HOURS = $DurationHours
$env:SOAK_TEST_MODE = "1"  # Enables JSON logging in daemon

# Navigate to crates/master directory
$originalLocation = Get-Location
$masterCrateDir = Join-Path (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))) "crates\master"

if (-not (Test-Path $masterCrateDir)) {
    Write-Host "ERROR: Master crate directory not found: $masterCrateDir" -ForegroundColor Red
    Stop-Job $monitorJob
    Remove-Job $monitorJob
    exit 1
}

Set-Location $masterCrateDir

# Run the soak test
$testOutput = Join-Path $runOutputDir "soak-test-output.log"
$testSuccess = $false

try {
    Write-Host "  Command: cargo test --release --test stability_24h -- --ignored --nocapture" -ForegroundColor Cyan
    Write-Host "  Output:  $testOutput`n" -ForegroundColor Cyan

    cargo test --release --test stability_24h -- --ignored --nocapture 2>&1 | Tee-Object -FilePath $testOutput

    if ($LASTEXITCODE -eq 0) {
        $testSuccess = $true
        Write-Host "`n  ✓ Soak test PASSED" -ForegroundColor Green
    } else {
        Write-Host "`n  ✗ Soak test FAILED (exit code: $LASTEXITCODE)" -ForegroundColor Red
    }
}
catch {
    Write-Host "`n  ✗ Soak test FAILED with exception: $_" -ForegroundColor Red
}
finally {
    Set-Location $originalLocation
}

$soakTestDuration = (Get-Date) - $soakTestStart

# Step 3: Wait for external monitor to complete and collect its output
Write-Host "`n[STEP 3/4] Waiting for external monitor to complete..." -ForegroundColor Yellow

$monitorJob | Wait-Job -Timeout 300 | Out-Null  # 5-minute timeout for monitor cleanup

if ($monitorJob.State -eq 'Completed') {
    $monitorOutput = Receive-Job -Job $monitorJob
    $monitorOutputFile = Join-Path $runOutputDir "external-monitor-output.log"
    $monitorOutput | Out-File -FilePath $monitorOutputFile
    Write-Host "  ✓ External monitor completed" -ForegroundColor Green
    Write-Host "    Output: $monitorOutputFile"
}
else {
    Write-Host "  ⚠ External monitor still running - stopping..." -ForegroundColor Yellow
    Stop-Job $monitorJob
    $monitorOutput = Receive-Job -Job $monitorJob
    $monitorOutputFile = Join-Path $runOutputDir "external-monitor-output-incomplete.log"
    $monitorOutput | Out-File -FilePath $monitorOutputFile
}

Remove-Job $monitorJob

# Step 4: Collect evidence
Write-Host "`n[STEP 4/4] Collecting evidence..." -ForegroundColor Yellow

$evidenceScriptPath = Join-Path $PSScriptRoot "collect-evidence.ps1"
if (Test-Path $evidenceScriptPath) {
    & $evidenceScriptPath -ProcessName $ProcessName -OutputDir $runOutputDir -EventLogHours ($DurationHours + 1)
}
else {
    Write-Host "  ⚠ Evidence collection script not found: $evidenceScriptPath" -ForegroundColor Yellow
}

# Generate summary report
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host " Soak Test Summary Report" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$summaryReport = @{
    TestResult = if ($testSuccess) { "PASSED" } else { "FAILED" }
    DurationRequested = "${DurationHours}h"
    DurationActual = "$([math]::Round($soakTestDuration.TotalHours, 2))h"
    StartTime = $soakTestStart.ToString("yyyy-MM-dd HH:mm:ss")
    EndTime = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    OutputDirectory = $runOutputDir
    TestOutputLog = $testOutput
    MonitorOutputLog = $monitorOutputFile
}

# Find metrics file
$metricsFile = Get-ChildItem -Path $runOutputDir -Filter "external-metrics-*.csv" | Select-Object -First 1
if ($metricsFile) {
    $summaryReport['ExternalMetricsCSV'] = $metricsFile.FullName
}

# Find alerts file
$alertsFile = Get-ChildItem -Path $runOutputDir -Filter "alerts-*.log" | Select-Object -First 1
if ($alertsFile) {
    $summaryReport['AlertsLog'] = $alertsFile.FullName

    # Count alerts by level
    $alertContent = Get-Content $alertsFile.FullName -ErrorAction SilentlyContinue
    $criticalAlerts = ($alertContent | Select-String -Pattern '\[CRITICAL\]').Count
    $warningAlerts = ($alertContent | Select-String -Pattern '\[WARNING\]').Count

    $summaryReport['CriticalAlerts'] = $criticalAlerts
    $summaryReport['WarningAlerts'] = $warningAlerts
}

# Save summary report
$summaryFile = Join-Path $runOutputDir "SUMMARY.json"
$summaryReport | ConvertTo-Json -Depth 5 | Out-File -FilePath $summaryFile

Write-Host "Test Result:    $($summaryReport.TestResult)" -ForegroundColor $(if ($testSuccess) { "Green" } else { "Red" })
Write-Host "Duration:       $($summaryReport.DurationActual) (requested: $($summaryReport.DurationRequested))"
Write-Host "Start Time:     $($summaryReport.StartTime)"
Write-Host "End Time:       $($summaryReport.EndTime)"

if ($summaryReport.ContainsKey('CriticalAlerts')) {
    Write-Host "Critical Alerts: $($summaryReport.CriticalAlerts)" -ForegroundColor $(if ($summaryReport.CriticalAlerts -gt 0) { "Red" } else { "Green" })
    Write-Host "Warning Alerts:  $($summaryReport.WarningAlerts)" -ForegroundColor $(if ($summaryReport.WarningAlerts -gt 0) { "Yellow" } else { "Green" })
}

Write-Host "`nAll results saved to: $runOutputDir" -ForegroundColor Cyan
Write-Host "Summary report:      $summaryFile" -ForegroundColor Cyan

# Display summary content
Write-Host "`n========================================" -ForegroundColor Cyan
Get-Content $summaryFile | Write-Host
Write-Host "========================================`n" -ForegroundColor Cyan

# Exit with test result
exit $(if ($testSuccess) { 0 } else { 1 })
