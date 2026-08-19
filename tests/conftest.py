"""
MONOTERMINAL E2E Test Configuration
Pytest fixtures and test utilities shared across all tests
"""

import asyncio
import os
import subprocess
import time
from pathlib import Path
from typing import Optional

import psutil
import pytest


# ============================================================================
# Session-scoped fixtures (run once per test session)
# ============================================================================

@pytest.fixture(scope="session")
def project_root() -> Path:
    """Return the project root directory."""
    return Path(__file__).parent.parent


@pytest.fixture(scope="session")
def master_binary(project_root: Path) -> Path:
    """
    Path to the master daemon binary.
    Build it if not found.
    """
    binary_path = project_root / "target" / "debug" / "monoterminal-master.exe"

    if not binary_path.exists():
        pytest.fail(
            f"Master binary not found at {binary_path}. "
            "Run 'cargo build' first."
        )

    return binary_path


@pytest.fixture(scope="session")
def web_client_url() -> str:
    """
    URL for the web client.
    Assumes web dev server is running on port 5173 (Vite default).
    """
    return "http://localhost:5173"


# ============================================================================
# Function-scoped fixtures (run once per test)
# ============================================================================

@pytest.fixture
async def daemon_process(master_binary: Path, tmp_path: Path):
    """
    Start the master daemon process for a single test.
    Automatically cleans up on test completion.

    Yields:
        DaemonHandle with methods to interact with the daemon
    """
    # Create temporary config
    config_path = tmp_path / "config.toml"
    config_path.write_text("""
[server]
listen_address = "127.0.0.1:0"  # Random port
enable_tls = false  # Disable TLS for testing

[auth]
require_auth = false  # Disable auth for basic tests

[logging]
level = "debug"
""")

    # Start daemon
    process = subprocess.Popen(
        [str(master_binary), "--config", str(config_path)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    # Wait for daemon to start (TODO: parse stdout for actual port)
    await asyncio.sleep(2)

    if process.poll() is not None:
        stdout, stderr = process.communicate()
        pytest.fail(
            f"Daemon failed to start:\nSTDOUT:\n{stdout}\nSTDERR:\n{stderr}"
        )

    handle = DaemonHandle(process, port=8080)  # TODO: Parse actual port

    yield handle

    # Cleanup: terminate daemon
    handle.terminate()


class DaemonHandle:
    """Handle for interacting with a running daemon process."""

    def __init__(self, process: subprocess.Popen, port: int):
        self.process = process
        self.port = port
        self.base_url = f"ws://127.0.0.1:{port}"

    def is_running(self) -> bool:
        """Check if daemon is still running."""
        return self.process.poll() is None

    def terminate(self, timeout: float = 5.0):
        """Gracefully terminate the daemon."""
        if not self.is_running():
            return

        try:
            # Try graceful shutdown first
            self.process.terminate()
            self.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            # Force kill if graceful shutdown fails
            self.process.kill()
            self.process.wait(timeout=1.0)

    def get_logs(self) -> tuple[str, str]:
        """Get stdout and stderr from the daemon."""
        if self.is_running():
            return "", ""  # Process still running

        stdout, stderr = self.process.communicate()
        return stdout, stderr


@pytest.fixture
def sample_jwt() -> str:
    """
    Generate a sample JWT token for testing.
    TODO: Replace with actual Ed25519 signing once auth module is complete.
    """
    return "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9.sample.token"


@pytest.fixture
def test_session_id() -> str:
    """Generate a test session ID (UUID format)."""
    import uuid
    return str(uuid.uuid4())


# ============================================================================
# Markers and custom pytest hooks
# ============================================================================

def pytest_configure(config):
    """Register custom markers."""
    config.addinivalue_line(
        "markers", "e2e: End-to-end tests requiring full system"
    )
    config.addinivalue_line(
        "markers", "integration: Integration tests for multi-component workflows"
    )
    config.addinivalue_line(
        "markers", "requires_daemon: Tests that need master daemon running"
    )


def pytest_collection_modifyitems(config, items):
    """
    Automatically mark tests based on their location.
    """
    for item in items:
        # Mark all tests in e2e/ as e2e tests
        if "e2e" in str(item.fspath):
            item.add_marker(pytest.mark.e2e)

        # Mark all tests in integration/ as integration tests
        if "integration" in str(item.fspath):
            item.add_marker(pytest.mark.integration)


# ============================================================================
# Async utilities
# ============================================================================

@pytest.fixture
def event_loop():
    """Create an event loop for async tests."""
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


# ============================================================================
# Import Playwright fixtures
# ============================================================================

# Import Playwright fixtures from conftest_playwright.py
# This makes them available to all tests
try:
    from tests.conftest_playwright import (
        browser,
        firefox_browser,
        webkit_browser,
        browser_context,
        playwright_page,
        evidence_dir,
        mobile_context,
        mobile_page,
        cross_browser_page,
    )
except ImportError:
    # Playwright not installed - browser tests will be skipped
    pass
