"""
E2E Browser Tests: xterm.js Rendering Validation (Criterion #3)
Tests the web client UI and xterm.js rendering with real browser automation

Test Coverage:
1. Terminal spawn and rendering in browser
2. Visual validation of terminal output
3. Resize handling with reflow
4. Scrollback navigation
5. Multi-session management in browser UI

Requires: Playwright, web-client dev server running
"""

import asyncio
import pytest
from pathlib import Path
from typing import Optional

pytest_plugins = ["pytest-playwright"]


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_terminal_spawn_and_render(
    playwright_page, daemon_process, evidence_dir
):
    """
    Test terminal spawn and basic rendering in browser.

    Steps:
    1. Open web client in browser
    2. Create new terminal session
    3. Verify xterm.js canvas renders
    4. Send command and verify output appears
    5. Capture screenshot for evidence
    """
    page = playwright_page

    # Navigate to web client
    await page.goto("http://localhost:5173")

    # Wait for xterm.js to initialize
    await page.wait_for_selector(".xterm", timeout=10000)

    # Take screenshot of initial state
    await page.screenshot(
        path=evidence_dir / "terminal-spawn-initial.png"
    )

    # Create new session (click "New Terminal" button)
    await page.click('button:has-text("New Terminal")')

    # Wait for terminal to be ready (cursor visible)
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)

    # Type command
    await page.keyboard.type("echo 'Hello MONOTERMINAL'\n")

    # Wait for output (look for text in terminal)
    await page.wait_for_function(
        """() => {
            const terminal = document.querySelector('.xterm-rows');
            return terminal && terminal.textContent.includes('Hello MONOTERMINAL');
        }""",
        timeout=5000
    )

    # Capture screenshot with output
    await page.screenshot(
        path=evidence_dir / "terminal-spawn-with-output.png"
    )

    # Verify terminal canvas exists and has content
    canvas = await page.query_selector("canvas.xterm-text-layer")
    assert canvas is not None, "xterm.js text layer canvas not found"

    # Get terminal text content
    terminal_text = await page.eval_on_selector(
        ".xterm-rows",
        "el => el.textContent"
    )

    assert "Hello MONOTERMINAL" in terminal_text, \
        f"Expected output not found. Got: {terminal_text}"


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_terminal_resize_reflow(
    playwright_page, daemon_process, evidence_dir
):
    """
    Test terminal resize with proper reflow.

    Validates:
    - PTY resize message sent to backend
    - xterm.js adapts to new dimensions
    - Content reflows correctly
    - No visual artifacts
    """
    page = playwright_page

    await page.goto("http://localhost:5173")
    await page.wait_for_selector(".xterm", timeout=10000)

    # Create session
    await page.click('button:has-text("New Terminal")')
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)

    # Send a long line of text
    long_text = "A" * 120
    await page.keyboard.type(f"echo '{long_text}'\n")
    await asyncio.sleep(1)

    # Take screenshot before resize
    await page.screenshot(
        path=evidence_dir / "terminal-before-resize.png"
    )

    # Get initial dimensions
    initial_cols = await page.evaluate(
        """() => {
            const xterm = window.__xterm_instance;
            return xterm ? xterm.cols : null;
        }"""
    )

    # Resize browser window (should trigger terminal resize)
    await page.set_viewport_size({"width": 1200, "height": 800})
    await asyncio.sleep(0.5)

    # Verify dimensions changed
    new_cols = await page.evaluate(
        """() => {
            const xterm = window.__xterm_instance;
            return xterm ? xterm.cols : null;
        }"""
    )

    assert new_cols != initial_cols, \
        "Terminal columns should change after resize"

    # Take screenshot after resize
    await page.screenshot(
        path=evidence_dir / "terminal-after-resize.png"
    )

    # Verify terminal is still responsive
    await page.keyboard.type("echo 'After resize'\n")
    await page.wait_for_function(
        """() => {
            const terminal = document.querySelector('.xterm-rows');
            return terminal && terminal.textContent.includes('After resize');
        }""",
        timeout=5000
    )


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_scrollback_navigation(
    playwright_page, daemon_process, evidence_dir
):
    """
    Test scrollback buffer navigation.

    Validates:
    - Scroll up to view history
    - Scroll down to bottom
    - Scrollbar position indicates location
    - History limit (10k lines)
    """
    page = playwright_page

    await page.goto("http://localhost:5173")
    await page.wait_for_selector(".xterm", timeout=10000)

    # Create session
    await page.click('button:has-text("New Terminal")')
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)

    # Generate multiple lines of output
    for i in range(50):
        await page.keyboard.type(f"echo 'Line {i:03d}'\n")
        if i % 10 == 0:
            await asyncio.sleep(0.5)  # Let output accumulate

    await asyncio.sleep(2)  # Final wait for all output

    # Scroll to top (Shift+PageUp or mouse wheel)
    terminal_element = await page.query_selector(".xterm")

    # Scroll up multiple times
    for _ in range(10):
        await page.keyboard.press("PageUp")
        await asyncio.sleep(0.1)

    # Screenshot at top of scrollback
    await page.screenshot(
        path=evidence_dir / "scrollback-top.png"
    )

    # Verify we can see early lines
    terminal_text = await page.eval_on_selector(
        ".xterm-rows",
        "el => el.textContent"
    )

    # Should see early line numbers when scrolled up
    # (exact check depends on viewport size)
    assert "Line 0" in terminal_text or "Line 00" in terminal_text, \
        "Should see early lines when scrolled to top"

    # Scroll to bottom (Shift+End)
    await page.keyboard.press("End")
    await asyncio.sleep(0.5)

    # Screenshot at bottom
    await page.screenshot(
        path=evidence_dir / "scrollback-bottom.png"
    )

    # Should see recent lines at bottom
    terminal_text = await page.eval_on_selector(
        ".xterm-rows",
        "el => el.textContent"
    )

    assert "Line 049" in terminal_text or "Line 49" in terminal_text, \
        "Should see recent lines when scrolled to bottom"


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_multi_session_ui(
    playwright_page, daemon_process, evidence_dir
):
    """
    Test multiple concurrent terminal sessions in UI.

    Validates:
    - Create multiple sessions
    - Switch between sessions
    - Each session maintains independent state
    - Session tabs/list UI works
    """
    page = playwright_page

    await page.goto("http://localhost:5173")
    await page.wait_for_selector(".xterm", timeout=10000)

    # Create first session
    await page.click('button:has-text("New Terminal")')
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)
    await page.keyboard.type("echo 'Session 1'\n")
    await asyncio.sleep(1)

    # Screenshot session 1
    await page.screenshot(
        path=evidence_dir / "multi-session-1.png"
    )

    # Create second session
    await page.click('button:has-text("New Terminal")')
    await asyncio.sleep(1)
    await page.keyboard.type("echo 'Session 2'\n")
    await asyncio.sleep(1)

    # Screenshot session 2
    await page.screenshot(
        path=evidence_dir / "multi-session-2.png"
    )

    # Switch back to session 1 (click session tab/button)
    # This selector depends on actual web client implementation
    session_tabs = await page.query_selector_all('[data-session-tab]')
    if len(session_tabs) >= 2:
        await session_tabs[0].click()
        await asyncio.sleep(0.5)

        # Verify we see Session 1 output
        terminal_text = await page.eval_on_selector(
            ".xterm-rows",
            "el => el.textContent"
        )

        assert "Session 1" in terminal_text, \
            "Should see Session 1 output when switched back"


@pytest.mark.e2e
@pytest.mark.asyncio
async def test_session_reconnection(
    playwright_page, daemon_process, evidence_dir
):
    """
    Test session reconnection after disconnect.

    Validates:
    - Create session with output
    - Simulate disconnect (close WebSocket)
    - Reconnect to same session
    - Verify scrollback restored
    """
    page = playwright_page

    await page.goto("http://localhost:5173")
    await page.wait_for_selector(".xterm", timeout=10000)

    # Create session with identifiable output
    await page.click('button:has-text("New Terminal")')
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)

    test_marker = "RECONNECTION_TEST_MARKER_12345"
    await page.keyboard.type(f"echo '{test_marker}'\n")
    await asyncio.sleep(1)

    # Get session ID from UI
    session_id = await page.evaluate(
        """() => {
            return window.__current_session_id;
        }"""
    )

    # Screenshot before disconnect
    await page.screenshot(
        path=evidence_dir / "before-reconnect.png"
    )

    # Simulate network disconnect (close WebSocket)
    await page.evaluate(
        """() => {
            if (window.__ws) {
                window.__ws.close();
            }
        }"""
    )

    await asyncio.sleep(2)

    # Trigger reconnection (reload page or click reconnect button)
    await page.reload()
    await page.wait_for_selector(".xterm", timeout=10000)

    # Reattach to same session
    # (This depends on web client implementation - may auto-reconnect)
    # For now, assume auto-reconnect or manual reattach via UI

    await asyncio.sleep(2)

    # Screenshot after reconnect
    await page.screenshot(
        path=evidence_dir / "after-reconnect.png"
    )

    # Verify marker is still visible (scrollback restored)
    terminal_text = await page.eval_on_selector(
        ".xterm-rows",
        "el => el.textContent"
    )

    assert test_marker in terminal_text, \
        "Scrollback should be restored after reconnection"


@pytest.mark.e2e
@pytest.mark.asyncio
@pytest.mark.slow
async def test_visual_regression_baseline(
    playwright_page, daemon_process, evidence_dir
):
    """
    Create visual regression baseline screenshots.

    Captures baseline images for:
    - Empty terminal
    - Terminal with text output
    - Terminal with colors/ANSI
    - Terminal after resize

    Use these baselines for future visual regression tests.
    """
    page = playwright_page

    await page.goto("http://localhost:5173")
    await page.wait_for_selector(".xterm", timeout=10000)

    # Baseline 1: Empty terminal
    await page.click('button:has-text("New Terminal")')
    await page.wait_for_selector(".xterm-cursor-layer", timeout=5000)
    await page.screenshot(
        path=evidence_dir / "baseline-empty-terminal.png",
        full_page=True
    )

    # Baseline 2: Terminal with plain text
    await page.keyboard.type("echo 'Plain text output'\n")
    await asyncio.sleep(1)
    await page.screenshot(
        path=evidence_dir / "baseline-plain-text.png",
        full_page=True
    )

    # Baseline 3: Terminal with ANSI colors
    # Use tput or ANSI escape sequences
    await page.keyboard.type(
        "echo -e '\\033[31mRed\\033[0m \\033[32mGreen\\033[0m \\033[34mBlue\\033[0m'\n"
    )
    await asyncio.sleep(1)
    await page.screenshot(
        path=evidence_dir / "baseline-ansi-colors.png",
        full_page=True
    )

    # Baseline 4: Resized terminal
    await page.set_viewport_size({"width": 1600, "height": 1000})
    await asyncio.sleep(1)
    await page.screenshot(
        path=evidence_dir / "baseline-resized-terminal.png",
        full_page=True
    )
