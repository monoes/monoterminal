/**
 * E2E Auth Integration Tests
 * Phase 5: Integration tests for Ed25519 challenge-response authentication
 *
 * Tests Criteria #2-4 per SRS v1.2
 * Backend ETA: 1-2h (security-engineer)
 * TODO: Remove test.skip() once backend integrated
 */

import { test, expect } from '@playwright/test';

test.describe('Auth Flow - Full E2E', () => {
  test.skip('should generate Ed25519 keypair on first launch', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(1000);
    
    const hasKeypair = await page.evaluate(async () => {
      const dbName = 'monoterminal-auth';
      const storeName = 'keypairs';
      return new Promise((resolve) => {
        const request = indexedDB.open(dbName);
        request.onsuccess = () => {
          const db = request.result;
          const tx = db.transaction(storeName, 'readonly');
          const store = tx.objectStore(storeName);
          const getReq = store.get('default');
          getReq.onsuccess = () => resolve(getReq.result !== undefined);
          getReq.onerror = () => resolve(false);
        };
        request.onerror = () => resolve(false);
      });
    });
    expect(hasKeypair).toBe(true);
  });

  test.skip('should complete full challenge-response auth flow', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(5000);
    
    // Verify terminal is active after auth
    const terminal = page.locator('.xterm');
    await expect(terminal).toBeVisible();
    
    // Try sending input
    await page.keyboard.type('echo "auth test"');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(1000);
  });
});

test.describe('Monomind Detection - Criterion #3', () => {
  test.skip('should display install suggestion when monomind missing', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(5000);
    
    const suggestion = page.locator('[data-testid="monomind-install-suggestion"]');
    await expect(suggestion).toBeVisible({ timeout: 5000 });
    await expect(suggestion).toContainText(/Install monomind/i);
  });
});

test.describe('Embedded Dashboard - Criterion #4', () => {
  test.skip('should load dashboard in web client after auth', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(3000);
    
    const dashboardButton = page.locator('[data-testid="dashboard-toggle"]');
    await dashboardButton.click();
    
    const dashboard = page.locator('[data-testid="monomind-dashboard"]');
    await expect(dashboard).toBeVisible();
  });

  test.skip('should execute health check via same WebSocket', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(3000);
    await page.locator('[data-testid="dashboard-toggle"]').click();
    await page.locator('[data-testid="health-check-button"]').click();
    await page.waitForTimeout(2000);
    
    const healthStatus = page.locator('[data-testid="health-status"]');
    await expect(healthStatus).toBeVisible();
  });
});
