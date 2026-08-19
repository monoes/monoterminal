"""
Playwright Fixtures for Browser-Based E2E Tests
Provides browser automation fixtures for xterm.js rendering validation
"""

import pytest
from pathlib import Path
from playwright.async_api import async_playwright, Browser, BrowserContext, Page


@pytest.fixture(scope="session")
async def browser():
    """
    Session-scoped browser instance.
    Uses Chromium by default for cross-platform consistency.
    """
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,  # Set to False for debugging
            args=[
                "--disable-dev-shm-usage",  # Avoid /dev/shm issues in containers
                "--no-sandbox",  # Required in some CI environments
            ]
        )
        yield browser
        await browser.close()


@pytest.fixture(scope="session")
async def firefox_browser():
    """Firefox browser for cross-browser testing."""
    async with async_playwright() as p:
        browser = await p.firefox.launch(headless=True)
        yield browser
        await browser.close()


@pytest.fixture(scope="session")
async def webkit_browser():
    """WebKit browser (Safari engine) for cross-browser testing."""
    async with async_playwright() as p:
        browser = await p.webkit.launch(headless=True)
        yield browser
        await browser.close()


@pytest.fixture
async def browser_context(browser: Browser) -> BrowserContext:
    """
    Function-scoped browser context with isolation.
    Each test gets a fresh context (cookies, localStorage, etc. cleared).
    """
    context = await browser.new_context(
        viewport={"width": 1280, "height": 720},
        locale="en-US",
        timezone_id="America/New_York",
    )
    yield context
    await context.close()


@pytest.fixture
async def playwright_page(browser_context: BrowserContext) -> Page:
    """
    Function-scoped page for browser tests.
    Automatically closes after test completion.
    """
    page = await browser_context.new_page()

    # Enable console logging for debugging
    page.on("console", lambda msg: print(f"[Browser Console] {msg.text}"))

    # Expose xterm instance for testing (requires web client to set window.__xterm_instance)
    await page.add_init_script("""
        window.__test_helpers = {
            getTerminalText: () => {
                const rows = document.querySelector('.xterm-rows');
                return rows ? rows.textContent : '';
            },
            getTerminalDimensions: () => {
                const xterm = window.__xterm_instance;
                return xterm ? { rows: xterm.rows, cols: xterm.cols } : null;
            }
        };
    """)

    yield page
    await page.close()


@pytest.fixture
def evidence_dir(project_root: Path) -> Path:
    """
    Directory for storing test evidence (screenshots, logs, reports).
    Creates timestamped subdirectory for each test run.
    """
    from datetime import datetime

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    evidence_path = project_root / "tests" / "evidence" / "phase1" / f"run-{timestamp}"
    evidence_path.mkdir(parents=True, exist_ok=True)

    return evidence_path


@pytest.fixture
async def mobile_context(browser: Browser) -> BrowserContext:
    """
    Browser context with mobile viewport for mobile testing.
    Simulates Android Chrome.
    """
    context = await browser.new_context(
        viewport={"width": 375, "height": 667},  # iPhone SE dimensions
        user_agent="Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) "
                   "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0 Mobile/15E148 Safari/604.1",
        device_scale_factor=2,
        is_mobile=True,
        has_touch=True,
    )
    yield context
    await context.close()


@pytest.fixture
async def mobile_page(mobile_context: BrowserContext) -> Page:
    """Page fixture with mobile viewport."""
    page = await mobile_context.new_page()
    page.on("console", lambda msg: print(f"[Mobile Browser] {msg.text}"))
    yield page
    await page.close()


# Cross-browser testing helpers

@pytest.fixture(params=["chromium", "firefox", "webkit"])
async def cross_browser_page(request, project_root: Path, evidence_dir: Path) -> Page:
    """
    Parameterized fixture for cross-browser testing.
    Runs the same test on Chromium, Firefox, and WebKit.
    """
    browser_type = request.param

    async with async_playwright() as p:
        if browser_type == "chromium":
            browser = await p.chromium.launch(headless=True)
        elif browser_type == "firefox":
            browser = await p.firefox.launch(headless=True)
        elif browser_type == "webkit":
            browser = await p.webkit.launch(headless=True)
        else:
            raise ValueError(f"Unknown browser type: {browser_type}")

        context = await browser.new_context(
            viewport={"width": 1280, "height": 720}
        )

        page = await context.new_page()
        page.on("console", lambda msg: print(f"[{browser_type}] {msg.text}"))

        yield page

        # Screenshot on failure
        if request.node.rep_call.failed:
            screenshot_path = evidence_dir / f"failure-{browser_type}-{request.node.name}.png"
            await page.screenshot(path=screenshot_path)

        await page.close()
        await context.close()
        await browser.close()


@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    """
    Hook to access test results for screenshot-on-failure.
    Stores result in item for fixture access.
    """
    outcome = yield
    rep = outcome.get_result()
    setattr(item, f"rep_{rep.when}", rep)
