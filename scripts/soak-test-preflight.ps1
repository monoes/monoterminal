#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Pre-flight checklist for 24h soak test execution

.DESCRIPTION
    Validates environment is ready for 24h soak test:
    - Cargo toolchain available
    - Project builds successfully
    - System resources adequate
    - Power settings configured
    - No conflicting processes
    - Evidence directory ready

.EXAMPLE
    .\soak-test-preflight.ps1
#>

$ErrorActionPreference = "Stop"

$checks = @{
    Passed = 0
    Failed = 0
    Warnings = 0
}

function Write-CheckResult {
    param(
        [string]$Check,
        [string]$Status,
        [string]$Message
    )

    $icon = switch ($Status) {
        "PASS" { "✅"; $checks.Passed++ }
        "FAIL" { "❌"; $checks.Failed++ }
        "WARN" { "⚠️ "; $checks.Warnings++ }
    }

    Write-Host "$icon $Check`: $Message"
}

Write-Host "==================================================="
Write-Host "24h Soak Test Pre-Flight Checklist"
Write-Host "==================================================="
Write-Host ""

# 1. Cargo toolchain
Write-Host "Checking Rust toolchain..."
try {
    $cargoVersion = cargo --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-CheckResult "Cargo installed" "PASS" $cargoVersion
    } else {
        Write-CheckResult "Cargo installed" "FAIL" "Cargo not found in PATH"
    }
} catch {
    Write-CheckResult "Cargo installed" "FAIL" "Cargo command failed: $_"
}

# 2. PowerShell version (needed for memory stats)
Write-Host ""
Write-Host "Checking PowerShell..."
$psVersion = $PSVersionTable.PSVersion
if ($psVersion.Major -ge 5) {
    Write-CheckResult "PowerShell version" "PASS" "v$psVersion"
} else {
    Write-CheckResult "PowerShell version" "FAIL" "v$psVersion (need v5+)"
}

# 3. Project builds
Write-Host ""
Write-Host "Checking project build..."
Push-Location $PSScriptRoot\..
try {
    $buildOutput = cargo build --release --test stability_24h 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-CheckResult "Project builds" "PASS" "Release build successful"
    } else {
        Write-CheckResult "Project builds" "FAIL" "Build failed (see output above)"
    }
} catch {
    Write-CheckResult "Project builds" "FAIL" "Build command failed: $_"
} finally {
    Pop-Location
}

# 4. System memory
Write-Host ""
Write-Host "Checking system resources..."
try {
    $mem = Get-CimInstance Win32_OperatingSystem
    $totalGB = [math]::Round($mem.TotalVisibleMemorySize / 1024 / 1024, 2)
    $freeGB = [math]::Round($mem.FreePhysicalMemory / 1024 / 1024, 2)
    $freePercent = [math]::Round(($freeGB / $totalGB) * 100, 1)

    if ($freeGB -gt 4) {
        Write-CheckResult "Available memory" "PASS" "$freeGB GB free ($freePercent% of $totalGB GB)"
    } elseif ($freeGB -gt 2) {
        Write-CheckResult "Available memory" "WARN" "$freeGB GB free (tight but may work)"
    } else {
        Write-CheckResult "Available memory" "FAIL" "$freeGB GB free (need at least 2GB)"
    }
} catch {
    Write-CheckResult "Available memory" "WARN" "Could not check: $_"
}

# 5. Disk space
Write-Host ""
try {
    $drive = Get-PSDrive -Name C
    $freeGB = [math]::Round($drive.Free / 1GB, 2)

    if ($freeGB -gt 10) {
        Write-CheckResult "Disk space" "PASS" "$freeGB GB free on C:"
    } elseif ($freeGB -gt 5) {
        Write-CheckResult "Disk space" "WARN" "$freeGB GB free (should be enough)"
    } else {
        Write-CheckResult "Disk space" "FAIL" "$freeGB GB free (need at least 5GB)"
    }
} catch {
    Write-CheckResult "Disk space" "WARN" "Could not check: $_"
}

# 6. Power settings (laptop battery check)
Write-Host ""
Write-Host "Checking power configuration..."
try {
    $battery = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue

    if ($null -eq $battery) {
        Write-CheckResult "Power source" "PASS" "Desktop system (no battery)"
    } else {
        $batteryPercent = $battery.EstimatedChargeRemaining
        $isCharging = $battery.BatteryStatus -eq 2

        if ($isCharging) {
            Write-CheckResult "Power source" "PASS" "Laptop plugged in (battery: $batteryPercent%)"
        } else {
            Write-CheckResult "Power source" "FAIL" "Laptop NOT plugged in (battery: $batteryPercent%)"
        }
    }
} catch {
    Write-CheckResult "Power source" "WARN" "Could not check battery status"
}

# 7. Sleep settings
Write-Host ""
try {
    $sleepTimeout = powercfg /query SCHEME_CURRENT SUB_SLEEP STANDBYIDLE 2>&1 | Select-String "Current AC Power Setting Index:" | ForEach-Object { $_ -replace ".*: 0x", "" }

    if ($sleepTimeout) {
        $sleepSeconds = [Convert]::ToInt32($sleepTimeout, 16)
        $sleepMinutes = $sleepSeconds / 60

        if ($sleepSeconds -eq 0) {
            Write-CheckResult "Sleep timeout" "PASS" "Sleep disabled"
        } elseif ($sleepMinutes -gt 1440) {
            Write-CheckResult "Sleep timeout" "PASS" "Sleep after $sleepMinutes min (>24h)"
        } else {
            Write-CheckResult "Sleep timeout" "WARN" "Sleep after $sleepMinutes min (may interrupt test)"
        }
    } else {
        Write-CheckResult "Sleep timeout" "WARN" "Could not determine sleep timeout"
    }
} catch {
    Write-CheckResult "Sleep timeout" "WARN" "Could not check: $_"
}

# 8. Evidence directory
Write-Host ""
$evidenceDir = Join-Path $PSScriptRoot "..\evidence\soak-test"
if (-not (Test-Path $evidenceDir)) {
    try {
        New-Item -ItemType Directory -Path $evidenceDir -Force | Out-Null
        Write-CheckResult "Evidence directory" "PASS" "Created: $evidenceDir"
    } catch {
        Write-CheckResult "Evidence directory" "FAIL" "Could not create: $_"
    }
} else {
    Write-CheckResult "Evidence directory" "PASS" "Exists: $evidenceDir"
}

# 9. Soak test infrastructure (orchestrator script)
Write-Host ""
$orchestratorScript = Join-Path $PSScriptRoot "soak-monitor\run-full-soak-test.ps1"
if (Test-Path $orchestratorScript) {
    Write-CheckResult "Orchestrator script" "PASS" "Found: run-full-soak-test.ps1"
} else {
    Write-CheckResult "Orchestrator script" "WARN" "Not found: $orchestratorScript (SRE suite may not be installed)"
}

# 10. Test file exists
Write-Host ""
$testFile = Join-Path $PSScriptRoot "..\crates\master\tests\soak\stability_24h.rs"
if (Test-Path $testFile) {
    Write-CheckResult "Test file" "PASS" "Found: stability_24h.rs"
} else {
    Write-CheckResult "Test file" "FAIL" "Not found: $testFile"
}

# Summary
Write-Host ""
Write-Host "==================================================="
Write-Host "Pre-Flight Summary"
Write-Host "==================================================="
Write-Host "Passed:   $($checks.Passed)"
Write-Host "Warnings: $($checks.Warnings)"
Write-Host "Failed:   $($checks.Failed)"
Write-Host "==================================================="

if ($checks.Failed -gt 0) {
    Write-Host ""
    Write-Host "❌ PRE-FLIGHT FAILED - Fix issues above before running test"
    exit 1
} elseif ($checks.Warnings -gt 0) {
    Write-Host ""
    Write-Host "⚠️  PRE-FLIGHT PASSED WITH WARNINGS - Review warnings before proceeding"
    exit 0
} else {
    Write-Host ""
    Write-Host "✅ PRE-FLIGHT PASSED - System ready for 24h soak test"
    Write-Host ""
    Write-Host "To run the test with full monitoring:"
    Write-Host "  cd scripts\soak-monitor"
    Write-Host "  .\run-full-soak-test.ps1 -DurationHours 24"
    Write-Host ""
    Write-Host "For quick 1-hour validation:"
    Write-Host "  .\run-full-soak-test.ps1 -DurationHours 1"
    Write-Host ""
    exit 0
}
