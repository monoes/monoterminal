"""
Test Infrastructure Verification Script
Verify that all dependencies and tools are installed correctly
"""

import sys
import subprocess
from pathlib import Path


def check_python_version():
    """Verify Python 3.11+ is installed."""
    print("Checking Python version...")
    version = sys.version_info
    if version.major == 3 and version.minor >= 11:
        print(f"  ✓ Python {version.major}.{version.minor}.{version.micro}")
        return True
    else:
        print(f"  ✗ Python {version.major}.{version.minor}.{version.micro} (need 3.11+)")
        return False


def check_dependencies():
    """Verify all Python dependencies are installed."""
    print("\nChecking Python dependencies...")

    required = [
        "pytest",
        "pytest_asyncio",
        "websockets",
        "aiohttp",
        "protobuf",
        "cryptography",
        "psutil",
    ]

    all_installed = True
    for package in required:
        try:
            __import__(package)
            print(f"  ✓ {package}")
        except ImportError:
            print(f"  ✗ {package} (not installed)")
            all_installed = False

    return all_installed


def check_rust_toolchain():
    """Verify Rust toolchain is installed."""
    print("\nChecking Rust toolchain...")
    try:
        result = subprocess.run(
            ["cargo", "--version"],
            capture_output=True,
            text=True,
            check=True
        )
        version = result.stdout.strip()
        print(f"  ✓ {version}")
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("  ✗ cargo not found")
        return False


def check_project_structure():
    """Verify project structure is correct."""
    print("\nChecking project structure...")

    project_root = Path(__file__).parent.parent

    required_paths = [
        "tests/conftest.py",
        "tests/pytest.ini",
        "tests/requirements.txt",
        "tests/common/__init__.py",
        "tests/common/protocol.py",
        "tests/common/daemon.py",
        "tests/e2e/test_session_flow.py",
        "tests/integration/test_websocket_handshake.py",
        "tests/integration/test_multi_client_attach.py",
        "tests/integration/test_protocol_compatibility.py",
        "tests/integration/test_monomind_integration.py",
        "crates/master/Cargo.toml",
        "crates/protocol/Cargo.toml",
        "proto/monoterminal/v1/messages.proto",
    ]

    all_exist = True
    for path_str in required_paths:
        path = project_root / path_str
        if path.exists():
            print(f"  ✓ {path_str}")
        else:
            print(f"  ✗ {path_str} (missing)")
            all_exist = False

    return all_exist


def check_master_binary():
    """Check if master daemon binary exists."""
    print("\nChecking master daemon binary...")

    project_root = Path(__file__).parent.parent
    binary_path = project_root / "target" / "debug" / "monoterminal-master.exe"

    if binary_path.exists():
        print(f"  ✓ {binary_path}")
        return True
    else:
        print(f"  ✗ {binary_path}")
        print("    Run 'cargo build --package monoterminal-master' to build it")
        return False


def run_sample_test():
    """Run a sample test to verify pytest works."""
    print("\nRunning sample pytest dry-run...")
    try:
        result = subprocess.run(
            ["pytest", "--collect-only", "tests/"],
            capture_output=True,
            text=True,
            cwd=Path(__file__).parent.parent,
            check=True
        )

        # Count collected tests
        lines = result.stdout.split("\n")
        test_count = sum(1 for line in lines if "<Function" in line or "<Method" in line)

        print(f"  ✓ pytest working ({test_count} tests collected)")
        return True
    except subprocess.CalledProcessError as e:
        print(f"  ✗ pytest failed: {e}")
        print(e.stdout)
        print(e.stderr)
        return False


def main():
    """Run all verification checks."""
    print("=" * 60)
    print("MONOTERMINAL Test Infrastructure Verification")
    print("=" * 60)

    checks = [
        ("Python Version", check_python_version),
        ("Python Dependencies", check_dependencies),
        ("Rust Toolchain", check_rust_toolchain),
        ("Project Structure", check_project_structure),
        ("Master Binary", check_master_binary),
        ("Pytest Functionality", run_sample_test),
    ]

    results = []
    for name, check_func in checks:
        try:
            result = check_func()
            results.append((name, result))
        except Exception as e:
            print(f"  ✗ Error during check: {e}")
            results.append((name, False))

    # Summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)

    passed = sum(1 for _, result in results if result)
    total = len(results)

    for name, result in results:
        status = "✓ PASS" if result else "✗ FAIL"
        print(f"{status:8} | {name}")

    print("=" * 60)
    print(f"Result: {passed}/{total} checks passed")

    if passed == total:
        print("\n✓ All checks passed! Test infrastructure is ready.")
        print("  Next: Wait for task-6 and task-13 to complete, then run:")
        print("    pytest tests/e2e/ -v")
        return 0
    else:
        print("\n✗ Some checks failed. Fix issues before running tests.")
        print("  Install missing dependencies:")
        print("    pip install -r tests/requirements.txt")
        return 1


if __name__ == "__main__":
    sys.exit(main())
