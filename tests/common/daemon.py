"""
Daemon management utilities for E2E tests
Helper functions for starting, stopping, and checking daemon health
"""

import asyncio
import time
from typing import Optional

import aiohttp


async def wait_for_daemon_ready(
    base_url: str,
    timeout: float = 10.0,
    check_interval: float = 0.5
) -> bool:
    """
    Wait for daemon to be ready to accept connections.

    Args:
        base_url: Base URL of the daemon (e.g., "http://127.0.0.1:8080")
        timeout: Maximum time to wait (seconds)
        check_interval: Time between checks (seconds)

    Returns:
        True if daemon is ready, False if timeout
    """
    start_time = time.time()

    while time.time() - start_time < timeout:
        try:
            async with aiohttp.ClientSession() as session:
                # Try to connect to health endpoint
                # TODO: Update URL once actual health endpoint is implemented
                async with session.get(f"{base_url}/health", timeout=1.0) as response:
                    if response.status == 200:
                        return True
        except (aiohttp.ClientError, asyncio.TimeoutError):
            pass

        await asyncio.sleep(check_interval)

    return False


async def check_daemon_health(base_url: str) -> dict:
    """
    Check daemon health status.

    Args:
        base_url: Base URL of the daemon

    Returns:
        Health check response as dict
    """
    async with aiohttp.ClientSession() as session:
        async with session.get(f"{base_url}/health") as response:
            response.raise_for_status()
            return await response.json()
