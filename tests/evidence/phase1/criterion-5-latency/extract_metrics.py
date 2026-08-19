#!/usr/bin/env python3
"""
Extract latency metrics from Criterion benchmark results.
Generates summary report with p50/p95/p99 values for Phase 1 gate verification.
"""

import json
import sys
from pathlib import Path
from typing import Dict, Optional

def extract_estimates(criterion_dir: Path) -> Dict[str, Dict[str, float]]:
    """
    Extract latency estimates from Criterion JSON output.

    Returns dict mapping benchmark_name -> {p50, p95, p99, mean} in milliseconds
    """
    results = {}

    # Find all estimates.json files in Criterion output
    for estimates_file in criterion_dir.rglob("*/new/estimates.json"):
        bench_name = estimates_file.parent.parent.name

        try:
            with open(estimates_file) as f:
                data = json.load(f)

            # Criterion stores values in nanoseconds
            # Convert to milliseconds for readability
            mean_ns = data.get("mean", {}).get("point_estimate", 0)
            median_ns = data.get("median", {}).get("point_estimate", 0)

            # p95/p99 not directly in estimates.json; would need raw.csv
            # For now, estimate from mean + std dev
            std_dev_ns = data.get("std_dev", {}).get("point_estimate", 0)

            results[bench_name] = {
                "mean_ms": mean_ns / 1_000_000,
                "median_ms": median_ns / 1_000_000,
                "std_dev_ms": std_dev_ns / 1_000_000,
                # Rough p95/p99 estimation (assumes normal distribution)
                "p95_est_ms": (median_ns + 1.645 * std_dev_ns) / 1_000_000,
                "p99_est_ms": (median_ns + 2.326 * std_dev_ns) / 1_000_000,
            }
        except Exception as e:
            print(f"Warning: Could not parse {estimates_file}: {e}", file=sys.stderr)

    return results

def format_report(results: Dict[str, Dict[str, float]], target_p95_ms: float = 10.0) -> str:
    """
    Format results as markdown report for Phase 1 gate verification.
    """
    lines = [
        "# Criterion #5 Latency Benchmark Results",
        "",
        f"**Phase 1 Gate Target:** p95 < {target_p95_ms}ms",
        "",
        "## Summary",
        "",
        "| Benchmark | Mean | Median (p50) | Est. p95 | Est. p99 | Status |",
        "|-----------|------|--------------|----------|----------|--------|"
    ]

    for name, metrics in sorted(results.items()):
        mean = metrics["mean_ms"]
        median = metrics["median_ms"]
        p95 = metrics["p95_est_ms"]
        p99 = metrics["p99_est_ms"]

        # Check if p95 meets target
        status = "✅ PASS" if p95 < target_p95_ms else "❌ FAIL"

        lines.append(
            f"| `{name}` | {mean:.3f}ms | {median:.3f}ms | {p95:.3f}ms | {p99:.3f}ms | {status} |"
        )

    lines.extend([
        "",
        "## Phase 1 Gate Verdict",
        ""
    ])

    # Overall verdict
    all_pass = all(m["p95_est_ms"] < target_p95_ms for m in results.values())

    if all_pass:
        lines.append(f"### ✅ PASS - All benchmarks < {target_p95_ms}ms p95")
    else:
        lines.append(f"### ❌ FAIL - One or more benchmarks exceed {target_p95_ms}ms p95")
        failing = [name for name, m in results.items() if m["p95_est_ms"] >= target_p95_ms]
        lines.append("")
        lines.append("**Failing benchmarks:**")
        for name in failing:
            lines.append(f"- `{name}`: {results[name]['p95_est_ms']:.3f}ms p95")

    lines.extend([
        "",
        "---",
        "",
        "**Note:** p95/p99 values are estimated from mean + std_dev.",
        "For exact percentiles, analyze `target/criterion/*/new/raw.csv`.",
        "",
        f"**Generated:** {Path.cwd()}",
    ])

    return "\n".join(lines)

def main():
    # Find Criterion output directory
    project_root = Path(__file__).parent.parent.parent.parent.parent
    criterion_dir = project_root / "target" / "criterion"

    if not criterion_dir.exists():
        print(f"Error: Criterion output not found at {criterion_dir}", file=sys.stderr)
        print("Run benchmarks first: cargo bench", file=sys.stderr)
        sys.exit(1)

    print(f"Parsing Criterion results from: {criterion_dir}")

    results = extract_estimates(criterion_dir)

    if not results:
        print("Warning: No benchmark results found", file=sys.stderr)
        sys.exit(1)

    print(f"\nFound {len(results)} benchmark(s)")

    # Generate report
    report = format_report(results, target_p95_ms=10.0)

    # Write to evidence directory
    output_file = Path(__file__).parent / "benchmark_results.md"
    output_file.write_text(report)
    print(f"\n✅ Report written to: {output_file}")

    # Also print to stdout
    print("\n" + "="*70)
    print(report)
    print("="*70)

    # Return exit code based on pass/fail
    all_pass = all(m["p95_est_ms"] < 10.0 for m in results.values())
    sys.exit(0 if all_pass else 1)

if __name__ == "__main__":
    main()
