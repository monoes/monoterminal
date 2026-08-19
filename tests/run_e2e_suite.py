#!/usr/bin/env python3
"""
E2E Test Suite Runner - Monday Execution Script
Automated orchestration of E2E tests for Phase 1 Criterion #3 verification

Usage:
    python tests/run_e2e_suite.py [--skip-browser] [--skip-soak] [--verbose]
"""

import argparse
import subprocess
import sys
import time
from pathlib import Path
from datetime import datetime
import os


class Colors:
    """ANSI color codes for terminal output."""
    HEADER = '\033[95m'
    OKBLUE = '\033[94m'
    OKCYAN = '\033[96m'
    OKGREEN = '\033[92m'
    WARNING = '\033[93m'
    FAIL = '\033[91m'
    ENDC = '\033[0m'
    BOLD = '\033[1m'


def print_header(message):
    """Print colored header."""
    print(f"\n{Colors.HEADER}{Colors.BOLD}{'=' * 70}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{message}{Colors.ENDC}")
    print(f"{Colors.HEADER}{Colors.BOLD}{'=' * 70}{Colors.ENDC}\n")


def print_step(message):
    """Print colored step message."""
    print(f"{Colors.OKCYAN}▶ {message}{Colors.ENDC}")


def print_success(message):
    """Print success message."""
    print(f"{Colors.OKGREEN}✓ {message}{Colors.ENDC}")


def print_error(message):
    """Print error message."""
    print(f"{Colors.FAIL}✗ {message}{Colors.ENDC}")


def print_warning(message):
    """Print warning message."""
    print(f"{Colors.WARNING}⚠ {message}{Colors.ENDC}")


def run_command(cmd, cwd=None, env=None, check=True):
    """Run shell command and return result."""
    print_step(f"Running: {' '.join(cmd)}")

    result = subprocess.run(
        cmd,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True
    )

    if result.returncode == 0:
        print_success(f"Command completed successfully")
    else:
        print_error(f"Command failed with exit code {result.returncode}")
        if result.stderr:
            print(f"STDERR:\n{result.stderr}")

    if check and result.returncode != 0:
        sys.exit(1)

    return result


def check_prerequisites():
    """Check that all prerequisites are installed."""
    print_header("Checking Prerequisites")

    checks = [
        ("Python", ["python", "--version"]),
        ("Rust", ["cargo", "--version"]),
        ("Node.js", ["node", "--version"]),
        ("pytest", ["pytest", "--version"]),
        ("Playwright", ["playwright", "--version"]),
    ]

    all_ok = True

    for name, cmd in checks:
        print_step(f"Checking {name}...")
        result = subprocess.run(cmd, capture_output=True, text=True)

        if result.returncode == 0:
            version = result.stdout.strip().split('\n')[0]
            print_success(f"{name}: {version}")
        else:
            print_error(f"{name} not found")
            all_ok = False

    if not all_ok:
        print_error("Prerequisites check failed. Please install missing dependencies.")
        sys.exit(1)

    print_success("All prerequisites satisfied")


def build_daemon(project_root):
    """Build the Rust daemon."""
    print_header("Building Rust Daemon")

    print_step("Building monoterminal-master in release mode...")
    run_command(
        ["cargo", "build", "--release", "--bin", "monoterminal-master"],
        cwd=project_root
    )

    binary_path = project_root / "target" / "release" / "monoterminal-master.exe"

    if binary_path.exists():
        print_success(f"Daemon built: {binary_path}")
    else:
        print_error(f"Daemon binary not found at {binary_path}")
        sys.exit(1)


def install_web_client_deps(project_root):
    """Install web client dependencies."""
    print_header("Installing Web Client Dependencies")

    web_client_dir = project_root / "web-client"

    if not web_client_dir.exists():
        print_warning("web-client directory not found - skipping")
        return False

    print_step("Running npm ci...")
    run_command(["npm", "ci"], cwd=web_client_dir)

    print_success("Web client dependencies installed")
    return True


def start_web_client(project_root):
    """Start web client dev server in background."""
    print_header("Starting Web Client Dev Server")

    web_client_dir = project_root / "web-client"

    if not web_client_dir.exists():
        print_warning("web-client directory not found - browser tests will be skipped")
        return None

    print_step("Starting npm run dev in background...")

    # Start dev server in background
    process = subprocess.Popen(
        ["npm", "run", "dev"],
        cwd=web_client_dir,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )

    # Wait for server to start
    print_step("Waiting for dev server to start (10 seconds)...")
    time.sleep(10)

    # Check if process is still running
    if process.poll() is not None:
        print_error("Dev server failed to start")
        stdout, stderr = process.communicate()
        print(f"STDOUT:\n{stdout}")
        print(f"STDERR:\n{stderr}")
        return None

    print_success("Web client dev server started")
    return process


def run_e2e_tests(project_root, skip_browser=False, verbose=False):
    """Run E2E test suite."""
    print_header("Running E2E Test Suite")

    tests_dir = project_root / "tests"
    evidence_dir = tests_dir / "evidence" / "phase1"
    evidence_dir.mkdir(parents=True, exist_ok=True)

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")

    # Build pytest command
    pytest_cmd = [
        "pytest",
        "-v",
        "-m", "e2e and not soak",  # Exclude soak tests (24h)
    ]

    if verbose:
        pytest_cmd.append("-vv")

    # Add HTML report
    html_report = evidence_dir / f"e2e-report-{timestamp}.html"
    pytest_cmd.extend(["--html", str(html_report), "--self-contained-html"])

    # Add JSON report
    json_report = evidence_dir / f"e2e-report-{timestamp}.json"
    pytest_cmd.extend(["--json-report", f"--json-report-file={json_report}"])

    # Run protocol tests
    print_step("Running protocol-level E2E tests...")
    protocol_result = subprocess.run(
        pytest_cmd + ["tests/e2e/test_session_flow.py"],
        cwd=project_root
    )

    # Run browser tests (if not skipped)
    browser_result = None
    if not skip_browser:
        print_step("Running browser rendering E2E tests...")
        browser_result = subprocess.run(
            pytest_cmd + ["tests/e2e/test_browser_rendering.py"],
            cwd=project_root
        )

    # Run integration tests
    print_step("Running integration tests...")
    integration_result = subprocess.run(
        pytest_cmd + ["tests/integration/"],
        cwd=project_root
    )

    # Summarize results
    print_header("Test Results Summary")

    all_passed = True

    if protocol_result.returncode == 0:
        print_success("Protocol E2E tests: PASSED")
    else:
        print_error("Protocol E2E tests: FAILED")
        all_passed = False

    if browser_result:
        if browser_result.returncode == 0:
            print_success("Browser E2E tests: PASSED")
        else:
            print_error("Browser E2E tests: FAILED")
            all_passed = False
    else:
        print_warning("Browser E2E tests: SKIPPED")

    if integration_result.returncode == 0:
        print_success("Integration tests: PASSED")
    else:
        print_error("Integration tests: FAILED")
        all_passed = False

    print(f"\n{Colors.BOLD}HTML Report:{Colors.ENDC} {html_report}")
    print(f"{Colors.BOLD}JSON Report:{Colors.ENDC} {json_report}")
    print(f"{Colors.BOLD}Evidence Directory:{Colors.ENDC} {evidence_dir}")

    return all_passed


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Run MONOTERMINAL E2E test suite")
    parser.add_argument(
        "--skip-browser",
        action="store_true",
        help="Skip browser-based rendering tests"
    )
    parser.add_argument(
        "--skip-soak",
        action="store_true",
        help="Skip soak tests (default: already excluded)"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Verbose pytest output"
    )
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Skip daemon build (use existing binary)"
    )

    args = parser.parse_args()

    # Determine project root
    script_path = Path(__file__).resolve()
    project_root = script_path.parent.parent

    print_header(f"MONOTERMINAL E2E Test Suite - Phase 1 Criterion #3")
    print(f"Project root: {project_root}")
    print(f"Timestamp: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")

    try:
        # Check prerequisites
        check_prerequisites()

        # Build daemon
        if not args.skip_build:
            build_daemon(project_root)
        else:
            print_warning("Skipping daemon build (using existing binary)")

        # Install web client deps
        has_web_client = install_web_client_deps(project_root)

        # Start web client dev server
        web_server_process = None
        if has_web_client and not args.skip_browser:
            web_server_process = start_web_client(project_root)

        # Run E2E tests
        all_passed = run_e2e_tests(project_root, args.skip_browser, args.verbose)

        # Final result
        print_header("Execution Complete")

        if all_passed:
            print_success("ALL TESTS PASSED ✓")
            exit_code = 0
        else:
            print_error("SOME TESTS FAILED ✗")
            exit_code = 1

        sys.exit(exit_code)

    except KeyboardInterrupt:
        print_warning("\nExecution interrupted by user")
        sys.exit(130)

    except Exception as e:
        print_error(f"Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

    finally:
        # Cleanup: stop web server if running
        if 'web_server_process' in locals() and web_server_process:
            print_step("Stopping web client dev server...")
            web_server_process.terminate()
            web_server_process.wait(timeout=5)
            print_success("Web server stopped")


if __name__ == "__main__":
    main()
