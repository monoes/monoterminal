# Cross-Platform Memory Profiling Script (Windows)
# Phase 3 Week 7 Day 2 - task-64
#
# Measures memory usage of monoterminal daemon over time
# Validates SRS §1.3 memory targets and Phase 2 baselines

param(
    [string]$ProcessName = "monoterminal",
    [int]$IntervalSeconds = 30,
    [int]$DurationMinutes = 5,
    [string]$OutputPath = "tests\evidence\phase3\memory-profile-$(Get-Date -Format 'yyyyMMdd-HHmmss').csv"
)

Write-Host "=== MONOTERMINAL Memory Profiling ==="
Write-Host "Process: $ProcessName"
Write-Host "Interval: $IntervalSeconds seconds"
Write-Host "Duration: $DurationMinutes minutes"
Write-Host "Output: $OutputPath"
Write-Host ""

# Initialize
$samples = @()
$iterations = ($DurationMinutes * 60) / $IntervalSeconds
$startTime = Get-Date

Write-Host "Starting profiling at $startTime..."
Write-Host ""

for ($i = 0; $i -lt $iterations; $i++) {
    $process = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue

    if ($process) {
        $elapsed = (Get-Date) - $startTime

        $sample = [PSCustomObject]@{
            Timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
            ElapsedSeconds = [math]::Round($elapsed.TotalSeconds, 0)
            ProcessId = $process.Id
            WorkingSetMB = [math]::Round($process.WorkingSet64 / 1MB, 2)
            PrivateBytesMB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
            VirtualMemoryMB = [math]::Round($process.VirtualMemorySize64 / 1MB, 2)
            PagedMemoryMB = [math]::Round($process.PagedMemorySize64 / 1MB, 2)
            NonPagedMemoryMB = [math]::Round($process.NonpagedSystemMemorySize64 / 1MB, 2)
            ThreadCount = $process.Threads.Count
            HandleCount = $process.HandleCount
        }

        $samples += $sample

        Write-Host "[$($sample.Timestamp)] Elapsed: $($sample.ElapsedSeconds)s | WS: $($sample.WorkingSetMB)MB | Private: $($sample.PrivateBytesMB)MB | Threads: $($sample.ThreadCount) | Handles: $($sample.HandleCount)"
    }
    else {
        Write-Warning "[$((Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))] Process '$ProcessName' not found"
    }

    # Don't sleep on last iteration
    if ($i -lt ($iterations - 1)) {
        Start-Sleep -Seconds $IntervalSeconds
    }
}

Write-Host ""
Write-Host "=== Profiling Complete ==="
Write-Host ""

if ($samples.Count -gt 0) {
    # Calculate statistics
    $avgWS = [math]::Round(($samples | Measure-Object -Property WorkingSetMB -Average).Average, 2)
    $maxWS = ($samples | Measure-Object -Property WorkingSetMB -Maximum).Maximum
    $minWS = ($samples | Measure-Object -Property WorkingSetMB -Minimum).Minimum

    $avgPrivate = [math]::Round(($samples | Measure-Object -Property PrivateBytesMB -Average).Average, 2)
    $maxPrivate = ($samples | Measure-Object -Property PrivateBytesMB -Maximum).Maximum
    $minPrivate = ($samples | Measure-Object -Property PrivateBytesMB -Minimum).Minimum

    $avgThreads = [math]::Round(($samples | Measure-Object -Property ThreadCount -Average).Average, 0)
    $maxThreads = ($samples | Measure-Object -Property ThreadCount -Maximum).Maximum

    $avgHandles = [math]::Round(($samples | Measure-Object -Property HandleCount -Average).Average, 0)
    $maxHandles = ($samples | Measure-Object -Property HandleCount -Maximum).Maximum

    # Calculate growth rate
    $firstWS = $samples[0].WorkingSetMB
    $lastWS = $samples[$samples.Count - 1].WorkingSetMB
    $growthPct = if ($firstWS -gt 0) { [math]::Round((($lastWS - $firstWS) / $firstWS) * 100, 2) } else { 0 }

    Write-Host "=== Statistics ==="
    Write-Host ""
    Write-Host "Working Set (MB):"
    Write-Host "  Min:     $minWS MB"
    Write-Host "  Max:     $maxWS MB"
    Write-Host "  Average: $avgWS MB"
    Write-Host ""
    Write-Host "Private Bytes (MB):"
    Write-Host "  Min:     $minPrivate MB"
    Write-Host "  Max:     $maxPrivate MB"
    Write-Host "  Average: $avgPrivate MB"
    Write-Host ""
    Write-Host "Threads:"
    Write-Host "  Average: $avgThreads"
    Write-Host "  Max:     $maxThreads"
    Write-Host ""
    Write-Host "Handles:"
    Write-Host "  Average: $avgHandles"
    Write-Host "  Max:     $maxHandles"
    Write-Host ""
    Write-Host "Memory Growth:"
    Write-Host "  Initial: $firstWS MB"
    Write-Host "  Final:   $lastWS MB"
    Write-Host "  Change:  $growthPct%"
    Write-Host ""

    # SRS Validation
    Write-Host "=== SRS Validation ==="
    Write-Host ""

    # Phase 2 reference: 30-min soak test = 5.5% growth acceptable
    $acceptableGrowth = 10.0
    $growthStatus = if ([math]::Abs($growthPct) -le $acceptableGrowth) { "PASS" } else { "FAIL" }
    Write-Host "Memory Growth: $growthPct% (Target: <$acceptableGrowth%) - $growthStatus"

    # Export to CSV
    $samples | Export-Csv -Path $OutputPath -NoTypeInformation
    Write-Host ""
    Write-Host "Results exported to: $OutputPath"
}
else {
    Write-Warning "No samples collected. Process may not have been running."
}

Write-Host ""
Write-Host "=== Profiling Complete ==="
