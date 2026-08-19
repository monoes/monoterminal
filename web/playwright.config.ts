import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright E2E Test Configuration - MONOTERMINAL Phase 1
 *
 * Tests Phase 1 acceptance criteria #2, #3, #4:
 * - Web client mobile browser usability (iOS/Android)
 * - Monomind detection & dismissal
 * - Embedded dashboard (no separate service)
 *
 * See: docs/phase1-acceptance-verification-plan.md
 */
export default defineConfig({
  testDir: './e2e',

  // E2E tests should run sequentially for daemon isolation
  fullyParallel: false,

  // Fail CI if test.only is committed
  forbidOnly: !!process.env.CI,

  // Retry on CI to handle transient network issues
  retries: process.env.CI ? 2 : 0,

  // One worker to ensure daemon isolation
  workers: 1,

  // Reporter
  reporter: process.env.CI ? 'github' : 'html',

  // Shared settings for all tests
  use: {
    // Base URL for web client (served by preview server on 8080)
    baseURL: 'http://localhost:8080',

    // Accept self-signed certificates for local development
    ignoreHTTPSErrors: true,

    // Collect trace on first retry for debugging
    trace: 'on-first-retry',

    // Screenshot on failure
    screenshot: 'only-on-failure',

    // Video on failure
    video: 'retain-on-failure',
  },

  // Test projects for different browser/device combinations
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },

    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },

    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },

    // Phase 1 Criterion #2: Mobile browser testing
    {
      name: 'mobile-chrome',
      use: { ...devices['Pixel 5'] },
    },

    {
      name: 'mobile-safari',
      use: { ...devices['iPhone 12'] },
    },
  ],

  // Development server configuration
  // Start both daemon and web client for E2E tests
  webServer: [
    {
      // Start Rust daemon first (WebSocket server on port 5000)
      // Use cargo run to auto-rebuild if source changed (ensures latest fixes applied)
      // --dev-mode: Skip JWT verification for UI/smoke tests (per security-engineer)
      // For full auth testing, run tests manually without --dev-mode
      command: 'cargo run --bin monoterminal -- --dev-mode',
      cwd: 'C:\\Users\\nokho\\Desktop\\projects\\monoterminal',
      port: 5000,  // Backend listens on 5000 per logs
      timeout: 120000,  // 2min timeout for rebuild + startup
      reuseExistingServer: !process.env.CI,
      stdout: 'pipe',
      stderr: 'pipe',
    },
    {
      // Then start web client preview server
      command: 'npm run preview',
      url: 'http://localhost:8080',
      reuseExistingServer: !process.env.CI,
      timeout: 30000,
    },
  ],
});
