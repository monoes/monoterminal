/**
 * E2E Test: Embedded Dashboard (Phase 1 Criterion #4)
 *
 * Verifies Phase 1 acceptance criterion #4:
 * "Embedded dashboard reflects live master state with no separate service to start"
 *
 * Test procedures from: docs/phase1-acceptance-verification-plan.md §3.4
 */

import { test, expect } from './fixtures/daemon';

test.describe('Phase 1 Criterion #4: Embedded Dashboard (No Separate Service)', () => {

  test('dashboard embedded in web client - same port/domain', async ({ page }) => {
    // 1. Navigate to web client
    await page.goto('/');
    await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });

    // 2. Open dashboard panel (should be in same page, not separate service)
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');

    await expect(dashboardToggle).toBeVisible();
    await dashboardToggle.click();

    // 3. Verify dashboard shows monomind org status
    const orgStatus = page.locator('[data-testid="org-status"]');
    await expect(orgStatus).toBeVisible({ timeout: 3000 });

    // 4. Verify agent count is displayed
    const agentCount = page.locator('[data-testid="agent-count"]');
    await expect(agentCount).toContainText(/\d+ agents?/i);
  });

  test('health check executes and displays result', async ({ page }) => {
    await page.goto('/');

    // Open dashboard
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardToggle.click();

    // Click health check button
    const healthCheckButton = page.locator('[data-testid="run-health-check"]');
    await expect(healthCheckButton).toBeVisible();
    await healthCheckButton.click();

    // Verify health check result appears within 10 seconds
    const healthResult = page.locator('[data-testid="health-result"]');
    await expect(healthResult).toContainText(/✅|✓|pass/i, { timeout: 10000 });
  });

  test('one-click upgrade button present', async ({ page }) => {
    await page.goto('/');

    // Open dashboard
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardToggle.click();

    // Verify upgrade button exists and is clickable
    const upgradeButton = page.locator('[data-testid="upgrade-button"]');
    await expect(upgradeButton).toBeVisible();
    await expect(upgradeButton).toBeEnabled();
  });

  test('dashboard uses same WebSocket connection - no separate port', async ({ page }) => {
    // Track all network requests
    const requests: string[] = [];
    page.on('request', req => requests.push(req.url()));

    await page.goto('/');

    // TODO: Open dashboard
    // const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    // await dashboardToggle.click();

    // Wait for dashboard to load
    await page.waitForTimeout(2000);

    // Verify NO requests to separate port (e.g., :9000, :9001, etc.)
    const separatePortRequests = requests.filter(url => {
      // Dashboard should NOT use a different port than 8080
      return url.includes(':9000') ||
             url.includes(':9001') ||
             url.includes(':3000') ||
             (url.includes('localhost') && !url.includes(':8080'));
    });

    expect(separatePortRequests).toHaveLength(0);
  });

  test('no separate authentication required - uses session JWT', async ({ page }) => {
    const requests: string[] = [];
    page.on('request', req => requests.push(req.url()));

    await page.goto('/');

    // TODO: Open dashboard
    // const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    // await dashboardToggle.click();

    await page.waitForTimeout(2000);

    // Assert: No separate OAuth flow or token exchange
    const oauthRequests = requests.filter(url =>
      url.includes('/oauth') ||
      url.includes('/token') ||
      url.includes('/auth/callback')
    );

    expect(oauthRequests).toHaveLength(0);
  });

  test('dashboard shows live monomind state', async ({ page }) => {
    await page.goto('/');

    // Open dashboard
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardToggle.click();

    // Verify dashboard displays required monomind information
    // Expected fields:
    // - Current org name
    // - Active agents list
    // - Run status (running/stopped)
    // - Health check button
    // - Upgrade button

    const orgName = page.locator('[data-testid="org-name"]');
    await expect(orgName).toBeVisible();

    const agentsList = page.locator('[data-testid="agents-list"]');
    await expect(agentsList).toBeVisible();

    const runStatus = page.locator('[data-testid="run-status"]');
    await expect(runStatus).toContainText(/running|stopped/i);
  });

  test('dashboard accessible from main UI - not separate tab', async ({ page, context }) => {
    await page.goto('/');

    // Track all opened pages/tabs
    const pagesBefore = context.pages().length;

    // Open dashboard
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardToggle.click();

    await page.waitForTimeout(1000);

    // Verify NO new browser tab was opened
    const pagesAfter = context.pages().length;
    expect(pagesAfter).toBe(pagesBefore); // Should be same page, not new tab

    // Dashboard should be visible within same page
    const dashboard = page.locator('[data-testid="dashboard-panel"]');
    await expect(dashboard).toBeVisible();
  });

  test('dashboard can be toggled open and closed', async ({ page }) => {
    await page.goto('/');

    // Toggle dashboard open
    const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardToggle.click();

    // Verify dashboard is visible
    const dashboard = page.locator('[data-testid="dashboard-panel"]');
    await expect(dashboard).toBeVisible();

    // Toggle dashboard closed
    await dashboardToggle.click();
    await expect(dashboard).not.toBeVisible();
  });
});

/**
 * Manual Verification Checklist (from verification plan §3.4):
 *
 * 1. Open web client at http://<master-ip>:8080
 * 2. Verify dashboard toggle/tab exists in UI
 * 3. Click dashboard → should open immediately (no separate login)
 * 4. Verify shows:
 *    [ ] Current org name
 *    [ ] Active agents list
 *    [ ] Run status (running/stopped)
 *    [ ] Health check button
 *    [ ] Upgrade button
 * 5. Run health check → should complete within 10s
 * 6. Verify NO separate browser tab or port (NOT http://localhost:9000/dashboard)
 *
 * Evidence Required:
 * - E2E test report (this file)
 * - Screenshot showing dashboard panel in web client
 * - Network trace showing single WebSocket connection
 *
 * Implementation Notes:
 *
 * This is a STUB for task-9 (framework setup).
 * Actual implementation in task-10 after:
 * - Dashboard UI component (frontend-lead)
 * - Monomind bridge API endpoints
 * - WebSocket message handlers for dashboard data
 *
 * Dependencies:
 * - task-12: Monomind Dashboard Implementation (frontend-lead)
 */
