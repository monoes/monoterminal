# MONOTERMINAL - Run All Performance Benchmarks
# Executes all Phase 1 performance validation benchmarks
# Generates consolidated HTML report

param(
    [Parameter(Mandatory=$false)]
    [switch]$SkipSoak = $false,

    [Parameter(Mandatory=$false)]
    [int]$SoakDurationHours = 1
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host " MONOTERMINAL Performance Benchmark Suite" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

$startTime = Get-Date

# Navigate to master crate
Push-Location -Path "$PSScriptRoot\..\crates\master"

try {
    # 1. FPS Rendering Benchmark
    Write-Host "[1/4] Running FPS Rendering Benchmark..." -ForegroundColor Yellow
    Write-Host "  Validates: SRS §7.1 Criterion #1 (60 FPS)" -ForegroundColor Gray
    cargo bench --bench fps_rendering
    if ($LASTEXITCODE -ne 0) {
        throw "FPS benchmark failed!"
    }
    Write-Host "  ✅ FPS benchmark complete" -ForegroundColor Green
    Write-Host ""

    # 2. WebSocket Latency Benchmark
    Write-Host "[2/4] Running WebSocket Latency Benchmark..." -ForegroundColor Yellow
    Write-Host "  Validates: SRS §7.1 Criterion #5 (<10ms p95)" -ForegroundColor Gray
    cargo bench --bench websocket_latency
    if ($LASTEXITCODE -ne 0) {
        throw "Latency benchmark failed!"
    }
    Write-Host "  ✅ Latency benchmark complete" -ForegroundColor Green
    Write-Host ""

    # 3. PTY Throughput Benchmark
    Write-Host "[3/4] Running PTY Throughput Benchmark..." -ForegroundColor Yellow
    Write-Host "  Validates: SRS §6.1 PTY performance" -ForegroundColor Gray
    cargo bench --bench pty_throughput
    if ($LASTEXITCODE -ne 0) {
        throw "PTY benchmark failed!"
    }
    Write-Host "  ✅ PTY benchmark complete" -ForegroundColor Green
    Write-Host ""

    # 4. Soak Test (optional)
    if (-not $SkipSoak) {
        Write-Host "[4/4] Running Soak Test ($SoakDurationHours hour(s))..." -ForegroundColor Yellow
        Write-Host "  Validates: SRS §7.1 Criterion #7 (zero crashes)" -ForegroundColor Gray
        Write-Host "  Note: This will take $SoakDurationHours hour(s)" -ForegroundColor Gray

        $env:SOAK_DURATION_HOURS = $SoakDurationHours
        cargo test --release --test stability_24h -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) {
            throw "Soak test failed!"
        }
        Write-Host "  ✅ Soak test complete" -ForegroundColor Green
    } else {
        Write-Host "[4/4] Skipping Soak Test (use -SkipSoak:$false to include)" -ForegroundColor Gray
    }

    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host " All Benchmarks Complete!" -ForegroundColor Cyan
    Write-Host "==================================================" -ForegroundColor Cyan

    $elapsed = (Get-Date) - $startTime
    Write-Host "Total runtime: $($elapsed.TotalMinutes.ToString('F2')) minutes" -ForegroundColor Cyan
    Write-Host ""

    # Print report locations
    Write-Host "Reports generated:" -ForegroundColor Yellow

    $reportDir = "..\..\target\criterion"
    if (Test-Path $reportDir) {
        Write-Host "  • FPS Rendering:      $reportDir\fps_rendering\report\index.html" -ForegroundColor Gray
        Write-Host "  • WebSocket Latency:  $reportDir\websocket_latency\report\index.html" -ForegroundColor Gray
        Write-Host "  • PTY Throughput:     $reportDir\pty_throughput\report\index.html" -ForegroundColor Gray
        Write-Host "  • Combined Report:    $reportDir\report\index.html" -ForegroundColor Gray
        Write-Host ""

        Write-Host "Open combined report?" -ForegroundColor Yellow
        Write-Host "  start $reportDir\report\index.html" -ForegroundColor Gray
        Write-Host ""

        $openReport = Read-Host "Open now? (y/n)"
        if ($openReport -eq 'y') {
            Start-Process "$reportDir\report\index.html"
        }
    } else {
        Write-Host "  Warning: Criterion output directory not found" -ForegroundColor Red
    }

} catch {
    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Red
    Write-Host " Benchmark Suite Failed!" -ForegroundColor Red
    Write-Host "==================================================" -ForegroundColor Red
    Write-Host "Error: $_" -ForegroundColor Red
    Pop-Location
    exit 1
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  1. Review HTML reports" -ForegroundColor Gray
Write-Host "  2. Verify Phase 1 acceptance criteria:" -ForegroundColor Gray
Write-Host "     • FPS p50 ≥ 60" -ForegroundColor Gray
Write-Host "     • Latency p95 < 10ms" -ForegroundColor Gray
Write-Host "     • Soak test: zero crashes" -ForegroundColor Gray
Write-Host "  3. Upload evidence to tests/evidence/phase1/" -ForegroundColor Gray
Write-Host "  4. Report to qa-lead via org_send" -ForegroundColor Gray
Write-Host ""

exit 0
