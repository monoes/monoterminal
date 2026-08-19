# Monday 9 AM Quick-Check Script
# Determines which scenario we're in: A (unblocked), B (different error), C (ed25519 error)
# Execute IMMEDIATELY after devops-lead confirms toolchain ready

$ErrorActionPreference = "Continue"  # Don't stop on first error
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$logFile = "test-results-monday-9am-$timestamp.txt"

Write-Host "=== Monday 9 AM Quick-Check ===" -ForegroundColor Cyan
Write-Host "Timestamp: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')" -ForegroundColor Gray
Write-Host ""

# Step 1: Verify cargo is available
Write-Host "[Check 1/3] Verifying Rust toolchain..." -ForegroundColor Yellow
$cargoCheck = Get-Command cargo -ErrorAction SilentlyContinue
if ($cargoCheck) {
    $cargoVersion = cargo --version
    Write-Host "✓ Cargo available: $cargoVersion" -ForegroundColor Green
} else {
    Write-Host "✗ Cargo not found - toolchain not ready" -ForegroundColor Red
    Write-Host "ACTION: Wait for devops-lead confirmation" -ForegroundColor Yellow
    exit 1
}

Write-Host ""

# Step 2: Run workspace test compilation + execution
Write-Host "[Check 2/3] Running full test suite..." -ForegroundColor Yellow
Write-Host "This determines which scenario we're in (A/B/C)..." -ForegroundColor Gray

$testStartTime = Get-Date
$testOutput = cargo test --workspace --all-features 2>&1 | Tee-Object -FilePath $logFile
$testEndTime = Get-Date
$testDuration = ($testEndTime - $testStartTime).TotalSeconds

Write-Host ""

# Step 3: Analyze results and determine scenario
Write-Host "[Check 3/3] Analyzing results..." -ForegroundColor Yellow

$hasCompileError = $testOutput | Select-String "error\[E\d+\]|error: could not compile"
$hasEd25519Error = $testOutput | Select-String "SigningKey|ed25519|Keypair"
$hasTestSuccess = $testOutput | Select-String "test result: ok"

Write-Host ""
Write-Host "=== SCENARIO DETERMINATION ===" -ForegroundColor Cyan

if ($hasTestSuccess) {
    # Scenario A: Unblocked!
    Write-Host "✅ SCENARIO A: Tests compile and pass cleanly!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Criterion #6 is UNBLOCKED - no compilation errors found" -ForegroundColor Green
    Write-Host "Pre-diagnostic work was correct - ed25519 code is already valid" -ForegroundColor Green
    Write-Host ""
    Write-Host "NEXT STEPS:" -ForegroundColor Yellow
    Write-Host "1. Report to eng-director: 'Criterion #6 unblocked, measuring coverage now'" -ForegroundColor White
    Write-Host "2. Execute: .\measure-coverage.ps1" -ForegroundColor White
    Write-Host "3. Expected completion: 11 AM (4 hours ahead of schedule)" -ForegroundColor White
    Write-Host ""
    Write-Host "Test execution time: $([math]::Round($testDuration, 1))s" -ForegroundColor Gray

    exit 0

} elseif ($hasCompileError -and $hasEd25519Error) {
    # Scenario C: ed25519 error in unexpected location
    Write-Host "⚠️  SCENARIO C: ed25519 compilation error found" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "This is unexpected - audit was thorough." -ForegroundColor Gray
    Write-Host "Likely causes:" -ForegroundColor Gray
    Write-Host "  - Transitive dependency using ed25519-dalek 1.x" -ForegroundColor Gray
    Write-Host "  - Hidden test file not caught by audit" -ForegroundColor Gray
    Write-Host "  - Generated code using old API" -ForegroundColor Gray
    Write-Host ""
    Write-Host "ERROR PREVIEW:" -ForegroundColor Red
    $testOutput | Select-String "SigningKey|ed25519" | Select-Object -First 10 | ForEach-Object {
        Write-Host "  $_" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "NEXT STEPS:" -ForegroundColor Yellow
    Write-Host "1. Review full error log: $logFile" -ForegroundColor White
    Write-Host "2. Escalate to security-engineer with error details" -ForegroundColor White
    Write-Host "3. Follow original timeline: fix by 12 PM" -ForegroundColor White

    exit 1

} elseif ($hasCompileError) {
    # Scenario B: Different compilation error
    Write-Host "⚠️  SCENARIO B: Compilation error (NOT ed25519)" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "The ed25519 audit was correct - error is in a different subsystem" -ForegroundColor Gray
    Write-Host ""
    Write-Host "ERROR PREVIEW:" -ForegroundColor Red
    $testOutput | Select-String "error\[E\d+\]|error: could not compile" | Select-Object -First 10 | ForEach-Object {
        Write-Host "  $_" -ForegroundColor Red
    }
    Write-Host ""
    Write-Host "NEXT STEPS:" -ForegroundColor Yellow
    Write-Host "1. Review full error log: $logFile" -ForegroundColor White
    Write-Host "2. Escalate to rust-backend-lead (NOT security-engineer)" -ForegroundColor White
    Write-Host "3. Follow original timeline: fix by 12 PM, measure by 3 PM" -ForegroundColor White

    exit 1

} else {
    # Unexpected state
    Write-Host "⚠️  UNEXPECTED: Cannot determine scenario" -ForegroundColor Magenta
    Write-Host ""
    Write-Host "Test output doesn't match expected patterns" -ForegroundColor Gray
    Write-Host ""
    Write-Host "NEXT STEPS:" -ForegroundColor Yellow
    Write-Host "1. Review full log: $logFile" -ForegroundColor White
    Write-Host "2. Manually inspect output" -ForegroundColor White
    Write-Host "3. Report to eng-director for guidance" -ForegroundColor White

    exit 2
}
