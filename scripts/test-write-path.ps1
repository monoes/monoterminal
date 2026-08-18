# Write Path Diagnostic Test
# 5-minute timeout, reliable logging, single iteration

$ErrorActionPreference = "Stop"

Write-Host "=== Write Path Diagnostic Test ===" -ForegroundColor Cyan
Write-Host "Timeout: 5 minutes" -ForegroundColor Yellow
Write-Host "Goal: Verify write path logging works" -ForegroundColor Yellow
Write-Host ""

# Kill any existing processes
Write-Host "Cleaning up existing processes..." -ForegroundColor Gray
Get-Process -Name "monoterminal*","latency*" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

# Set environment for short test
$env:LATENCY_SHORT_TEST = "10"
$env:RUST_LOG = "info,monoterminal_master=info"

# Output file
$outputFile = ".\write-path-diagnostic-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"

Write-Host "Output file: $outputFile" -ForegroundColor Green
Write-Host "Starting test with 5-minute timeout..." -ForegroundColor Cyan
Write-Host ""

# Run with 5-minute timeout
$job = Start-Job -ScriptBlock {
    param($logFile)
    Set-Location $using:PWD
    $env:LATENCY_SHORT_TEST = "10"
    $env:RUST_LOG = "info,monoterminal_master=info"
    cargo bench --bench latency_e2e_lan 2>&1 | Tee-Object -FilePath $logFile
} -ArgumentList $outputFile

# Wait with 5-minute timeout
$timeout = 300  # 5 minutes
$elapsed = 0
$completed = $false

while ($elapsed -lt $timeout) {
    if ($job.State -eq "Completed") {
        $completed = $true
        break
    }
    Start-Sleep -Seconds 5
    $elapsed += 5

    # Show progress every 30 seconds
    if ($elapsed % 30 -eq 0) {
        Write-Host "Elapsed: $elapsed seconds / $timeout seconds" -ForegroundColor Gray
    }
}

if (-not $completed) {
    Write-Host ""
    Write-Host "TIMEOUT: Test exceeded 5 minutes" -ForegroundColor Red
    Stop-Job -Job $job
    Remove-Job -Job $job -Force

    Write-Host "Checking partial output..." -ForegroundColor Yellow
} else {
    Write-Host ""
    Write-Host "Test completed within timeout" -ForegroundColor Green
    Receive-Job -Job $job
    Remove-Job -Job $job
}

# Analyze output
Write-Host ""
Write-Host "=== Analyzing Output ===" -ForegroundColor Cyan

if (Test-Path $outputFile) {
    $size = (Get-Item $outputFile).Length
    Write-Host "Log file size: $size bytes" -ForegroundColor Gray

    if ($size -gt 0) {
        Write-Host ""
        Write-Host "=== Checking for Write Path Logs ===" -ForegroundColor Cyan

        $writeLogsPresent = Select-String -Path $outputFile -Pattern "WRITE:" -Quiet
        $inputDataPresent = Select-String -Path $outputFile -Pattern "InputData" -Quiet
        $ptyWritePresent = Select-String -Path $outputFile -Pattern "PTY write" -Quiet

        if ($writeLogsPresent) {
            Write-Host "✅ Write path logs found!" -ForegroundColor Green
            Write-Host ""
            Write-Host "Sample write logs:" -ForegroundColor Yellow
            Select-String -Path $outputFile -Pattern "WRITE:" | Select-Object -First 10
        } else {
            Write-Host "❌ No write path logs found" -ForegroundColor Red
        }

        if ($inputDataPresent) {
            Write-Host ""
            Write-Host "✅ InputData processing logs found!" -ForegroundColor Green
            Write-Host ""
            Write-Host "Sample InputData logs:" -ForegroundColor Yellow
            Select-String -Path $outputFile -Pattern "InputData" | Select-Object -First 5
        } else {
            Write-Host "❌ No InputData logs found" -ForegroundColor Red
        }

        if ($ptyWritePresent) {
            Write-Host ""
            Write-Host "✅ PTY write logs found!" -ForegroundColor Green
            Write-Host ""
            Write-Host "Sample PTY write logs:" -ForegroundColor Yellow
            Select-String -Path $outputFile -Pattern "PTY write" | Select-Object -First 5
        } else {
            Write-Host "❌ No PTY write logs found" -ForegroundColor Red
        }

        # Check for errors
        Write-Host ""
        Write-Host "=== Checking for Errors ===" -ForegroundColor Cyan
        $errors = Select-String -Path $outputFile -Pattern "ERROR|error:|failed|Failed" | Select-Object -First 10
        if ($errors) {
            Write-Host "Errors found:" -ForegroundColor Red
            $errors | ForEach-Object { Write-Host $_.Line -ForegroundColor Red }
        } else {
            Write-Host "No errors found in captured output" -ForegroundColor Green
        }

    } else {
        Write-Host "❌ Log file is empty" -ForegroundColor Red
    }
} else {
    Write-Host "❌ Log file not created" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== Test Complete ===" -ForegroundColor Cyan
Write-Host "Log file: $outputFile" -ForegroundColor Gray
