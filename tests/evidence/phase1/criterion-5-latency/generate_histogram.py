#!/usr/bin/env python3
"""
Generate latency histogram from Criterion raw data.
Marks p50, p95, p99 thresholds for Phase 1 gate visualization.
"""

import csv
import sys
from pathlib import Path
from typing import List, Tuple

def parse_raw_data(raw_csv: Path) -> List[float]:
    """
    Parse Criterion raw.csv and extract sample times in milliseconds.

    Criterion raw.csv format:
    sample_measured_value,iteration_count
    123456,1
    ...

    Returns list of per-iteration times in milliseconds.
    """
    samples = []

    with open(raw_csv) as f:
        reader = csv.DictReader(f)
        for row in reader:
            # measured_value is in nanoseconds, iteration_count is number of iterations
            measured_ns = float(row["sample_measured_value"])
            iter_count = int(row["iteration_count"])

            # Per-iteration time
            per_iter_ns = measured_ns / iter_count
            per_iter_ms = per_iter_ns / 1_000_000

            samples.append(per_iter_ms)

    return samples

def calculate_percentiles(samples: List[float]) -> Tuple[float, float, float]:
    """Calculate p50, p95, p99 from samples."""
    sorted_samples = sorted(samples)
    n = len(sorted_samples)

    p50_idx = int(n * 0.50)
    p95_idx = int(n * 0.95)
    p99_idx = int(n * 0.99)

    return (
        sorted_samples[p50_idx],
        sorted_samples[p95_idx],
        sorted_samples[p99_idx],
    )

def generate_ascii_histogram(samples: List[float], p50: float, p95: float, p99: float, target_p95: float = 10.0) -> str:
    """
    Generate ASCII histogram for terminal display.
    """
    import math

    # Create bins
    min_val = min(samples)
    max_val = max(samples)
    bin_count = 40
    bin_width = (max_val - min_val) / bin_count

    bins = [0] * bin_count
    for sample in samples:
        bin_idx = int((sample - min_val) / bin_width)
        if bin_idx >= bin_count:
            bin_idx = bin_count - 1
        bins[bin_idx] += 1

    # Normalize to max height of 20 chars
    max_count = max(bins)
    max_height = 20

    lines = [
        "Latency Histogram",
        "=" * 60,
        ""
    ]

    for i in range(max_height, 0, -1):
        line = f"{i*max_count//max_height:>6} | "
        for count in bins:
            if count * max_height // max_count >= i:
                line += "█"
            else:
                line += " "
        lines.append(line)

    # X-axis
    lines.append(f"{'':>7}|" + "-" * bin_count)

    # Labels
    lines.append(f"{'':>7} {min_val:.2f}ms{'':<{bin_count-15}}{max_val:.2f}ms")

    # Statistics
    lines.extend([
        "",
        f"p50 (median):  {p50:.3f}ms",
        f"p95:           {p95:.3f}ms  {'✅ PASS' if p95 < target_p95 else '❌ FAIL'} (target < {target_p95}ms)",
        f"p99:           {p99:.3f}ms",
        f"Samples:       {len(samples):,}",
    ])

    return "\n".join(lines)

def main():
    # Find Criterion raw data
    project_root = Path(__file__).parent.parent.parent.parent.parent
    criterion_dir = project_root / "target" / "criterion"

    if len(sys.argv) > 1:
        bench_name = sys.argv[1]
    else:
        # Default to full_rtt_simulation
        bench_name = "websocket_serialization/full_rtt_simulation"

    raw_csv = criterion_dir / bench_name / "new" / "raw.csv"

    if not raw_csv.exists():
        print(f"Error: Raw data not found at {raw_csv}", file=sys.stderr)
        print("\nAvailable benchmarks:", file=sys.stderr)
        for p in criterion_dir.glob("*/new/raw.csv"):
            print(f"  - {p.parent.parent.name}", file=sys.stderr)
        sys.exit(1)

    print(f"Reading raw data from: {raw_csv}\n")

    samples = parse_raw_data(raw_csv)
    p50, p95, p99 = calculate_percentiles(samples)

    # Generate ASCII histogram
    histogram = generate_ascii_histogram(samples, p50, p95, p99, target_p95=10.0)
    print(histogram)

    # Save to file
    output_file = Path(__file__).parent / f"histogram_{bench_name.replace('/', '_')}.txt"
    output_file.write_text(histogram)
    print(f"\n✅ Saved to: {output_file}")

    # Return exit code based on p95 target
    sys.exit(0 if p95 < 10.0 else 1)

if __name__ == "__main__":
    main()
