# Criterion #6 Coverage Measurement Script
# SRS §7.1 Requirement: ≥70% code coverage
# Phase 1 Verification Plan §3.6
# Monday Aug 18, 2026 - 12 PM - 3 PM execution window

param(
    [switch]$SkipTests = $false,
    [string]$OutputDir = ".",
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Stop"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

Write-Host "=== Criterion #6 Coverage Measurement ===" -ForegroundColor Cyan
Write-Host "Timestamp: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')" -ForegroundColor Gray
Write-Host ""

# Step 1: Verify compilation
Write-Host "[Step 1/5] Verifying test compilation..." -ForegroundColor Yellow
if (-not $SkipTests) {
    try {
        cargo test --workspace --all-features --no-run
        Write-Host "[OK] All tests compile successfully" -ForegroundColor Green
    } catch {
        Write-Host "[FAIL] Compilation failed" -ForegroundColor Red
        Write-Host "Error: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "SKIPPED (--SkipTests flag)" -ForegroundColor Gray
}

Write-Host ""

# Step 2: Run full test suite
Write-Host "[Step 2/5] Running full test suite..." -ForegroundColor Yellow
if (-not $SkipTests) {
    try {
        $testOutput = cargo test --workspace --all-features 2>&1
        $testOutput | Out-File -FilePath "$OutputDir\test-output-$timestamp.log"

        # Count test results
        $passed = ($testOutput | Select-String "test result: ok").Count
        if ($passed -gt 0) {
            Write-Host "[OK] Test suite passed" -ForegroundColor Green
        } else {
            Write-Host "[WARN] No test results found - check log" -ForegroundColor Yellow
        }
    } catch {
        Write-Host "[FAIL] Tests failed" -ForegroundColor Red
        Write-Host "Error: $_" -ForegroundColor Red
        Write-Host "Check: $OutputDir\test-output-$timestamp.log" -ForegroundColor Gray
        exit 1
    }
} else {
    Write-Host "SKIPPED (--SkipTests flag)" -ForegroundColor Gray
}

Write-Host ""

# Step 3: Generate coverage report with cargo-tarpaulin
Write-Host "[Step 3/5] Generating coverage report (this may take 5-10 minutes)..." -ForegroundColor Yellow

# Check if cargo-tarpaulin is installed
$tarpaulinCheck = Get-Command cargo-tarpaulin -ErrorAction SilentlyContinue
if (-not $tarpaulinCheck) {
    Write-Host "[WARN] cargo-tarpaulin not found - installing..." -ForegroundColor Yellow
    cargo install cargo-tarpaulin
}

try {
    # Generate HTML + JSON coverage reports
    $coverageCmd = "cargo tarpaulin --workspace --all-features --timeout 600 --out Html --out Json --output-dir `"$OutputDir`""

    if ($Verbose) {
        Write-Host "Executing: $coverageCmd" -ForegroundColor Gray
    }

    Invoke-Expression $coverageCmd

    Write-Host "[OK] Coverage report generated" -ForegroundColor Green
} catch {
    Write-Host "✗ Coverage generation failed" -ForegroundColor Red
    Write-Host "Error: $_" -ForegroundColor Red
    exit 1
}

Write-Host ""

# Step 4: Extract coverage percentage
Write-Host "[Step 4/5] Analyzing coverage results..." -ForegroundColor Yellow

$jsonReport = Get-Content "$OutputDir\tarpaulin-report.json" -Raw | ConvertFrom-Json
$avgCoverage = $jsonReport.files.coverage | Measure-Object -Average | Select-Object -ExpandProperty Average
$totalCoverage = [math]::Round($avgCoverage, 2)

Write-Host "Total Coverage: $totalCoverage%" -ForegroundColor $(if ($totalCoverage -ge 70) { "Green" } else { "Red" })

# Per-crate breakdown
Write-Host ""
Write-Host "Per-crate breakdown:" -ForegroundColor Cyan
$crateStats = $jsonReport.files | Group-Object {
    if ($_.file -match 'crates\\([^\\]+)\\') { $matches[1] } else { 'root' }
} | ForEach-Object {
    $crate = $_.Name
    $avgCov = [math]::Round(($_.Group.coverage | Measure-Object -Average).Average, 2)
    [PSCustomObject]@{
        Crate = $crate
        Coverage = $avgCov
        Status = if ($avgCov -ge 50) { "OK" } else { "WARN" }
    }
}

$crateStats | Format-Table -AutoSize

# Export to CSV
$crateStats | Export-Csv -Path "$OutputDir\coverage-by-crate-$timestamp.csv" -NoTypeInformation
Write-Host "[OK] Per-crate breakdown saved to coverage-by-crate-$timestamp.csv" -ForegroundColor Green

Write-Host ""

# Step 5: Generate summary
Write-Host "[Step 5/5] Generating summary..." -ForegroundColor Yellow

$summary = @"
# Criterion #6 Coverage Measurement Results
**Date**: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
**SRS Requirement**: §7.1 - ≥70% code coverage
**Phase**: Phase 1 Windows+Web MVP

## Overall Result
- **Total Coverage**: $totalCoverage%
- **Target**: 70%
- **Status**: $(if ($totalCoverage -ge 70) { "PASS" } else { "FAIL" })

## Per-Crate Breakdown
$(($crateStats | ForEach-Object { "- **$($_.Crate)**: $($_.Coverage)% $($_.Status)" }) -join "`n")

## Evidence Files
- HTML Report: `tarpaulin-report.html`
- JSON Report: `tarpaulin-report.json`
- Per-Crate CSV: `coverage-by-crate-$timestamp.csv`
- Test Output Log: `test-output-$timestamp.log`

## Next Steps
$(if ($totalCoverage -ge 70) {
"[PASS] Coverage target met - proceed to evidence collection (task-6)
[PASS] Report success to qa-lead
[PASS] Update Phase 1 verification checklist"
} else {
"[WARN] Coverage gap: $(70 - $totalCoverage)%
[WARN] Identify lowest-coverage crates for improvement
[WARN] Estimate effort to reach 70%
[WARN] Report gap to eng-director with timeline"
})

---
*Generated by: test-engineer-unit*
*Measurement script: measure-coverage.ps1*
"@

$summary | Out-File -FilePath "$OutputDir\coverage-summary-$timestamp.md"
Write-Host "[OK] Summary saved to coverage-summary-$timestamp.md" -ForegroundColor Green

Write-Host ""
Write-Host "=== Measurement Complete ===" -ForegroundColor Cyan
Write-Host "Open $OutputDir\tarpaulin-report.html to view detailed coverage report" -ForegroundColor Gray

# Exit with status based on coverage
if ($totalCoverage -ge 70) {
    exit 0
} else {
    exit 1
}
