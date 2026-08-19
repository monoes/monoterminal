/**
 * Smoke Test: Web Client Basic Connectivity
 *
 * Verifies that the web client can load and connect to the master daemon.
 * This is the foundational test that must pass before any other E2E tests.
 */

import { test, expect } from './fixtures/daemon';

test.describe('Web Client Smoke Tests', () => {
  test('web client loads and connects to daemon', async ({ page }) => {
    // Navigate to web client
    await page.goto('/');

    // Verify page loads with correct title
    await expect(page).toHaveTitle(/MONOTERMINAL/i);

    // Verify terminal container is present in DOM
    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible({ timeout: 10000 });

    // Verify WebSocket connection is established
    // The status indicator should show "Connected" within 5 seconds
    const statusIndicator = page.locator('[data-testid="connection-status"]');
    await expect(statusIndicator).toContainText(/connected/i, { timeout: 5000 });

    // Verify no console errors during load
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    // Allow page to settle
    await page.waitForTimeout(1000);

    // Should have no critical errors (allow non-critical warnings)
    const criticalErrors = consoleErrors.filter(err =>
      !err.includes('favicon') &&
      !err.includes('DevTools')
    );
    expect(criticalErrors).toHaveLength(0);
  });

  test('xterm.js terminal is initialized', async ({ page }) => {
    await page.goto('/');

    // Wait for xterm container
    const xtermScreen = page.locator('.xterm-screen');
    await expect(xtermScreen).toBeVisible({ timeout: 10000 });

    // Verify terminal has canvas element (xterm-text-layer)
    const textLayer = page.locator('.xterm-text-layer');
    await expect(textLayer).toBeVisible();

    // Verify terminal is interactive (has cursor)
    const cursor = page.locator('.xterm-cursor-layer');
    await expect(cursor).toBeVisible();
  });

  test('PWA manifest is loaded', async ({ page }) => {
    await page.goto('/');

    // Check for PWA manifest link
    const manifestLink = page.locator('link[rel="manifest"]');
    await expect(manifestLink).toHaveCount(1);

    const manifestHref = await manifestLink.getAttribute('href');
    expect(manifestHref).toBeTruthy();

    // Verify manifest file is accessible
    const manifestResponse = await page.goto(manifestHref!);
    expect(manifestResponse?.status()).toBe(200);

    // Verify manifest has required PWA fields
    const manifestJson = await manifestResponse?.json();
    expect(manifestJson.name).toBeTruthy();
    expect(manifestJson.icons).toBeTruthy();
    expect(manifestJson.start_url).toBeTruthy();
  });

  test('service worker registers successfully', async ({ page }) => {
    await page.goto('/');

    // Wait for service worker registration
    await page.waitForFunction(() => {
      return navigator.serviceWorker.ready;
    }, { timeout: 10000 });

    // Verify service worker is active
    const swState = await page.evaluate(() => {
      return navigator.serviceWorker.controller?.state;
    });

    expect(swState).toBe('activated');
  });
});
