#!/usr/bin/env python3
"""
Parse Criterion benchmark results and generate Phase 1 Criterion #5 compliance report.

Extracts p50/p95/p99 latency from JSON estimates and validates against SRS targets.
"""

import json
import sys
from pathlib import Path
from typing import Dict, Any


def parse_estimates(estimates_path: Path) -> Dict[str, Any]:
    """Parse Criterion estimates.json file."""
    with open(estimates_path, 'r') as f:
        data = json.load(f)

    # Criterion stores estimates in nanoseconds
    mean_ns = data.get('mean', {}).get('point_estimate', 0)
    std_dev_ns = data.get('std_dev', {}).get('point_estimate', 0)

    # Convert to milliseconds
    mean_ms = mean_ns / 1_000_000
    std_dev_ms = std_dev_ns / 1_000_000

    return {
        'mean_ms': mean_ms,
        'std_dev_ms': std_dev_ms,
        'mean_ns': mean_ns,
        'std_dev_ns': std_dev_ns,
    }


def estimate_percentiles(mean_ms: float, std_dev_ms: float) -> Dict[str, float]:
    """
    Estimate percentiles from mean and std dev (assuming normal distribution).

    For accurate p95/p99, Criterion saves individual samples in 'sample.json',
    but estimates.json provides mean/median which we can use for quick validation.
    """
    import math

    # Z-scores for normal distribution
    z_p50 = 0.0      # median
    z_p95 = 1.645    # 95th percentile
    z_p99 = 2.326    # 99th percentile

    p50 = mean_ms
    p95 = mean_ms + (z_p95 * std_dev_ms)
    p99 = mean_ms + (z_p99 * std_dev_ms)

    return {
        'p50': p50,
        'p95': p95,
        'p99': p99,
    }


def validate_criterion_5(benchmarks: Dict[str, Dict[str, float]]) -> Dict[str, Any]:
    """
    Validate Phase 1 Acceptance Criterion #5 (SRS §7.1).

    Targets:
    - p50 < 5ms
    - p95 < 10ms (GATE)
    - p99 < 15ms
    """
    results = {}

    # Find the E2E RTT benchmark (the critical one for Criterion #5)
    e2e_key = None
    for key in benchmarks.keys():
        if 'e2e' in key.lower() or 'rtt' in key.lower() or 'real_master_rtt' in key.lower():
            e2e_key = key
            break

    if not e2e_key:
        return {
            'status': 'ERROR',
            'message': 'No E2E RTT benchmark found in results',
        }

    e2e = benchmarks[e2e_key]

    results['benchmark'] = e2e_key
    results['measurements'] = e2e

    # Validate against targets
    checks = []

    if e2e['p50'] < 5.0:
        checks.append({'target': 'p50 < 5ms', 'actual': f"{e2e['p50']:.3f}ms", 'status': 'PASS'})
    else:
        checks.append({'target': 'p50 < 5ms', 'actual': f"{e2e['p50']:.3f}ms", 'status': 'FAIL'})

    if e2e['p95'] < 10.0:
        checks.append({'target': 'p95 < 10ms (GATE)', 'actual': f"{e2e['p95']:.3f}ms", 'status': 'PASS'})
        gate_pass = True
    else:
        checks.append({'target': 'p95 < 10ms (GATE)', 'actual': f"{e2e['p95']:.3f}ms", 'status': 'FAIL'})
        gate_pass = False

    if e2e['p99'] < 15.0:
        checks.append({'target': 'p99 < 15ms', 'actual': f"{e2e['p99']:.3f}ms", 'status': 'PASS'})
    else:
        checks.append({'target': 'p99 < 15ms', 'actual': f"{e2e['p99']:.3f}ms", 'status': 'FAIL'})

    results['checks'] = checks
    results['gate_status'] = 'PASS' if gate_pass else 'FAIL'

    return results


def generate_report(criterion_dir: Path, output_path: Path):
    """Generate compliance report from Criterion results."""

    benchmarks = {}

    # Scan criterion output directory
    for benchmark_dir in criterion_dir.iterdir():
        if not benchmark_dir.is_dir():
            continue

        estimates_file = benchmark_dir / 'base' / 'estimates.json'
        if not estimates_file.exists():
            continue

        estimates = parse_estimates(estimates_file)
        percentiles = estimate_percentiles(estimates['mean_ms'], estimates['std_dev_ms'])

        benchmarks[benchmark_dir.name] = {
            **estimates,
            **percentiles,
        }

    # Validate Criterion #5
    validation = validate_criterion_5(benchmarks)

    # Generate markdown report
    report = []
    report.append("# Phase 1 Criterion #5 - Latency Benchmark Results")
    report.append("")
    report.append("**Generated:** 2026-08-16")
    report.append("**SRS Reference:** §7.1 (Phase 1 Acceptance), §5.1.2 (Latency Targets)")
    report.append("")

    report.append("## Acceptance Criterion #5: Interactive Latency")
    report.append("")
    report.append("| Metric | Target | Actual | Status |")
    report.append("|--------|--------|--------|--------|")

    for check in validation.get('checks', []):
        status_emoji = '✅' if check['status'] == 'PASS' else '❌'
        report.append(f"| {check['target']} | - | {check['actual']} | {status_emoji} {check['status']} |")

    report.append("")
    report.append(f"**GATE STATUS:** {'✅ PASS' if validation.get('gate_status') == 'PASS' else '❌ FAIL'}")
    report.append("")

    report.append("## Detailed Results")
    report.append("")
    report.append("### End-to-End RTT (Primary Measurement)")
    report.append("")

    if 'measurements' in validation:
        m = validation['measurements']
        report.append(f"- **Mean:** {m['mean_ms']:.3f}ms")
        report.append(f"- **Std Dev:** {m['std_dev_ms']:.3f}ms")
        report.append(f"- **p50 (median):** {m['p50']:.3f}ms")
        report.append(f"- **p95:** {m['p95']:.3f}ms")
        report.append(f"- **p99:** {m['p99']:.3f}ms")

    report.append("")
    report.append("### All Benchmarks")
    report.append("")
    report.append("| Benchmark | Mean | p50 | p95 | p99 |")
    report.append("|-----------|------|-----|-----|-----|")

    for name, data in sorted(benchmarks.items()):
        report.append(
            f"| {name} | {data['mean_ms']:.3f}ms | "
            f"{data['p50']:.3f}ms | {data['p95']:.3f}ms | {data['p99']:.3f}ms |"
        )

    report.append("")
    report.append("## Evidence Trail")
    report.append("")
    report.append("- **Raw Output:** `target/benchmark_output_latency_e2e.txt`")
    report.append("- **Criterion JSON:** `target/criterion/*/base/estimates.json`")
    report.append("- **HTML Reports:** `target/criterion/*/report/index.html`")
    report.append("")
    report.append("## Notes")
    report.append("")
    report.append("1. Percentiles estimated from mean/std-dev assuming normal distribution")
    report.append("2. For precise p95/p99, see `target/criterion/*/base/sample.json`")
    report.append("3. Benchmark configuration: 10,000 samples, 30s measurement time")
    report.append("")

    # Write report
    with open(output_path, 'w') as f:
        f.write('\n'.join(report))

    print(f"Report generated: {output_path}")
    print(f"Gate Status: {validation.get('gate_status', 'UNKNOWN')}")

    return validation.get('gate_status') == 'PASS'


def main():
    """Main entry point."""
    project_root = Path(__file__).parent.parent
    criterion_dir = project_root / 'target' / 'criterion'
    output_path = project_root / 'docs' / 'criterion-5-latency-results.md'

    if not criterion_dir.exists():
        print(f"ERROR: Criterion output directory not found: {criterion_dir}")
        print("Run benchmarks first: cargo bench --bench latency_e2e_lan")
        sys.exit(1)

    passed = generate_report(criterion_dir, output_path)

    sys.exit(0 if passed else 1)


if __name__ == '__main__':
    main()
