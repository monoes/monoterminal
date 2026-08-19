/**
 * E2E Test: Mobile Browser Usability (Phase 1 Criterion #2)
 *
 * Verifies Phase 1 acceptance criterion #2:
 * "Web client usable end-to-end from iPhone/Android browser on same LAN"
 *
 * Test procedures from: docs/phase1-acceptance-verification-plan.md §3.2
 */

import { test, expect } from './fixtures/daemon';

test.describe('Phase 1 Criterion #2: Mobile Browser Usability', () => {
  test.use({
    // Run these tests on mobile viewports
    viewport: { width: 375, height: 667 }, // iPhone SE size
    hasTouch: true, // Enable touch support for touchscreen.tap()
  });

  test('mobile browser full workflow - iOS Safari viewport', async ({ page, browserName }) => {
    test.skip(browserName !== 'webkit', 'This test is for Safari/WebKit only');

    // 1. Navigate to web client from mobile device
    await page.goto('/');

    // 2. Verify PWA installability
    // Note: Playwright can't trigger real beforeinstallprompt, but we can check manifest
    const manifestLink = page.locator('link[rel="manifest"]');
    await expect(manifestLink).toHaveCount(1);

    // 3. Verify page renders correctly on mobile viewport
    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible({ timeout: 10000 });

    // 4. Connect to session (stub - actual session creation in task-10)
    // TODO: Implement session creation flow
    // await page.click('[data-testid="connect-button"]');
    // await page.fill('[data-testid="session-id-input"]', 'test-session-mobile');
    // await page.click('[data-testid="attach-button"]');

    // 5. Verify touch interaction is possible
    // Click on terminal to focus
    await terminal.click();

    // TODO: Type command and verify output (requires session implementation)
    // await page.keyboard.type('echo "Hello from mobile"\n');
    // await expect(terminal).toContainText('Hello from mobile', { timeout: 5000 });

    // 6. Verify touch scrolling works
    const xtermViewport = page.locator('.xterm-viewport');
    await expect(xtermViewport).toBeVisible();

    // Simulate touch scroll
    await page.touchscreen.tap(200, 300);

    // 7. Verify no layout breaks on mobile
    const viewportWidth = await page.evaluate(() => window.innerWidth);
    expect(viewportWidth).toBeLessThanOrEqual(375);

    // Terminal should not overflow viewport
    const terminalBox = await terminal.boundingBox();
    expect(terminalBox?.width).toBeLessThanOrEqual(viewportWidth);
  });

  test('mobile browser full workflow - Android Chrome viewport', async ({ page, browserName }) => {
    test.skip(browserName !== 'chromium', 'This test is for Chrome only');

    // 1. Navigate to web client
    await page.goto('/');

    // 2. Verify PWA install banner context (Chrome-specific)
    // Check for required PWA metadata
    const themeColorMeta = page.locator('meta[name="theme-color"]');
    await expect(themeColorMeta).toHaveCount(1);

    // 3. Verify terminal renders on Android viewport
    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible({ timeout: 10000 });

    // 4. Touch keyboard should appear when tapping terminal
    // Note: Playwright can't fully test real mobile keyboard, but we verify focus
    await terminal.click();
    const isFocused = await page.evaluate(() => {
      const activeElement = document.activeElement;
      return activeElement?.classList.contains('xterm') ||
             activeElement?.closest('.xterm') !== null;
    });
    expect(isFocused).toBeTruthy();

    // 5. Verify touch scrolling works
    const xtermViewport = page.locator('.xterm-viewport');
    await expect(xtermViewport).toBeVisible();

    // 6. Verify no console errors
    const consoleErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await page.waitForTimeout(2000);

    const criticalErrors = consoleErrors.filter(err =>
      !err.includes('favicon') &&
      !err.includes('DevTools')
    );
    expect(criticalErrors).toHaveLength(0);
  });

  test('PWA "Add to Home Screen" metadata present', async ({ page }) => {
    await page.goto('/');

    // Verify all required PWA metadata
    const checks = await page.evaluate(() => {
      return {
        hasManifest: !!document.querySelector('link[rel="manifest"]'),
        hasThemeColor: !!document.querySelector('meta[name="theme-color"]'),
        hasViewport: !!document.querySelector('meta[name="viewport"]'),
        hasAppleTouchIcon: !!document.querySelector('link[rel="apple-touch-icon"]'),
      };
    });

    expect(checks.hasManifest).toBeTruthy();
    expect(checks.hasThemeColor).toBeTruthy();
    expect(checks.hasViewport).toBeTruthy();
    expect(checks.hasAppleTouchIcon).toBeTruthy();
  });

  test('responsive layout adapts to portrait orientation', async ({ page }) => {
    // Set to mobile portrait (iPhone 12)
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/');

    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible();

    // Terminal should use full width in portrait
    const terminalBox = await terminal.boundingBox();
    expect(terminalBox?.width).toBeGreaterThan(300); // Reasonable mobile width
  });

  test('responsive layout adapts to landscape orientation', async ({ page }) => {
    // Set to mobile landscape
    await page.setViewportSize({ width: 844, height: 390 });
    await page.goto('/');

    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible();

    // Terminal should adapt to landscape
    const terminalBox = await terminal.boundingBox();
    expect(terminalBox?.width).toBeGreaterThan(700);
  });
});

/**
 * Manual Testing Checklist (to be performed on physical devices)
 *
 * iOS Safari (iPhone 12+, iOS 16+):
 * [ ] Can access http://<master-ip>:8080 on LAN
 * [ ] Terminal renders correctly (no layout breaks)
 * [ ] Touch keyboard appears when tapping terminal
 * [ ] Can type commands and see output
 * [ ] Touch scrolling works smoothly
 * [ ] PWA "Add to Home Screen" works
 * [ ] No console errors
 *
 * Android Chrome (Pixel 6+, Android 12+):
 * [ ] Same checklist as iOS
 * [ ] PWA install banner appears
 * [ ] Haptic feedback on key press (optional)
 *
 * Evidence: Video recording of full workflow on real devices required
 */
