# Quick Result Analyzer for E2E Latency Benchmark
# Checks if p95 < 10ms target is met

$ErrorActionPreference = "Stop"
$ProjectRoot = "C:\Users\nokho\Desktop\projects\monoterminal"
$TargetP95Ms = 10.0

# Path to the real benchmark results (not mock)
$BenchmarkDir = Join-Path $ProjectRoot "target\criterion\e2e_lan_latency\real_master_rtt_loopback"
$EstimatesFile = Join-Path $BenchmarkDir "new\estimates.json"

if (-not (Test-Path $EstimatesFile)) {
    Write-Host "❌ No benchmark results found at: $EstimatesFile" -ForegroundColor Red
    Write-Host "Run: cargo bench --bench latency_e2e_lan" -ForegroundColor Yellow
    exit 1
}

# Parse JSON results
$Results = Get-Content $EstimatesFile -Raw | ConvertFrom-Json

# Extract key metrics
# Criterion stores times in nanoseconds
$MeanNs = $Results.mean.point_estimate
$MedianNs = $Results.median.point_estimate
$StdDevNs = $Results.std_dev.point_estimate

# Convert to milliseconds
$MeanMs = $MeanNs / 1000000.0
$MedianMs = $MedianNs / 1000000.0
$StdDevMs = $StdDevNs / 1000000.0

# Estimate p95 (mean + 1.645 * stddev for normal distribution)
# Note: This is an approximation; Criterion doesn't store p95 directly
$P95Ms = $MeanMs + (1.645 * $StdDevMs)

Write-Host ""
Write-Host "="*70 -ForegroundColor Cyan
Write-Host "E2E Latency Benchmark Results" -ForegroundColor Cyan
Write-Host "="*70 -ForegroundColor Cyan
Write-Host ""
Write-Host "Mean:   $([math]::Round($MeanMs, 2)) ms" -ForegroundColor White
Write-Host "Median: $([math]::Round($MedianMs, 2)) ms" -ForegroundColor White
Write-Host "StdDev: $([math]::Round($StdDevMs, 2)) ms" -ForegroundColor White
Write-Host "P95 (est): $([math]::Round($P95Ms, 2)) ms" -ForegroundColor $(if ($P95Ms -lt $TargetP95Ms) { "Green" } else { "Red" })
Write-Host ""
Write-Host "Target: p95 < $TargetP95Ms ms" -ForegroundColor Yellow
Write-Host ""

if ($P95Ms -lt $TargetP95Ms) {
    Write-Host "✅ PASS - Criterion #5 MET" -ForegroundColor Green
    Write-Host ""
    Write-Host "Result: p95 $([math]::Round($P95Ms, 2)) ms < $TargetP95Ms ms target" -ForegroundColor Green
    exit 0
} else {
    Write-Host "❌ FAIL - Criterion #5 NOT MET" -ForegroundColor Red
    Write-Host ""
    Write-Host "Result: p95 $([math]::Round($P95Ms, 2)) ms >= $TargetP95Ms ms target" -ForegroundColor Red
    Write-Host "Deficit: $([math]::Round($P95Ms - $TargetP95Ms, 2)) ms over budget" -ForegroundColor Red
    exit 1
}
