# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: smoke.spec.ts >> Web Client Smoke Tests >> PWA manifest is loaded
- Location: e2e\smoke.spec.ts:62:3

# Error details

```
Error: expect(locator).toHaveCount(expected) failed

Locator:  locator('link[rel="manifest"]')
Expected: 1
Received: 0
Timeout:  5000ms

Call log:
  - Expect "toHaveCount" with timeout 5000ms
  - waiting for locator('link[rel="manifest"]')
    13 × locator resolved to 0 elements
       - unexpected value "0"

```

# Page snapshot

```yaml
- generic [ref=e2]:
  - region "Notifications alt+T"
  - generic [ref=e4]:
    - generic [ref=e10]:
      - generic [ref=e11]:
        - link [ref=e13] [cursor=pointer]:
          - /url: "#/"
          - heading "llama-ui" [level=1] [ref=e14]
        - generic [ref=e15]:
          - link [ref=e16] [cursor=pointer]:
            - /url: "?new_chat=true#/"
            - generic [ref=e17]: New chat
            - generic:
              - generic: ⌘
              - text: O
          - button [ref=e18] [cursor=pointer]:
            - generic [ref=e19]: Search
            - generic:
              - generic: ⌘
              - text: K
          - link "MCP Servers" [ref=e20] [cursor=pointer]:
            - /url: "#/mcp-servers"
          - link "Settings" [ref=e22] [cursor=pointer]:
            - /url: "#/settings"
      - generic [ref=e24]:
        - generic [ref=e25]: Recent conversations
        - list [ref=e27]:
          - paragraph [ref=e29]: No conversations yet
    - button "Toggle Sidebar" [ref=e31] [cursor=pointer]
    - complementary [ref=e34]:
      - generic [ref=e35]:
        - button [ref=e37] [cursor=pointer]:
          - button "New chat" [ref=e38]
        - button [ref=e40] [cursor=pointer]:
          - button "Search" [ref=e41]
        - button [ref=e43] [cursor=pointer]:
          - button "MCP Servers" [ref=e44]
        - button [ref=e46] [cursor=pointer]:
          - button "Settings" [ref=e47]
    - main [ref=e48]:
      - main "Chat interface with file drop zone" [ref=e49]:
        - generic [ref=e50]:
          - generic:
            - generic:
              - heading "Hello there" [level=1]
              - paragraph: Type a message or upload files to get started
            - generic:
              - button "Scroll to bottom"
            - generic [ref=e53]:
              - button:
                - generic: Open prompt picker
              - button:
                - generic: Open resource picker
              - generic [ref=e55]:
                - textbox "Type a message..." [active] [ref=e57]
                - generic [ref=e58]:
                  - button "Add files, prompts, tools or MCP Servers" [ref=e61] [cursor=pointer]
                  - button "qwen3 35B" [ref=e64] [cursor=pointer]:
                    - generic [ref=e66]:
                      - generic [ref=e67]: qwen3
                      - generic [ref=e68]: 35B
                  - button "Send" [disabled]
```

# Test source

```ts
  1  | /**
  2  |  * Smoke Test: Web Client Basic Connectivity
  3  |  *
  4  |  * Verifies that the web client can load and connect to the master daemon.
  5  |  * This is the foundational test that must pass before any other E2E tests.
  6  |  */
  7  | 
  8  | import { test, expect } from './fixtures/daemon';
  9  | 
  10 | test.describe('Web Client Smoke Tests', () => {
  11 |   test('web client loads and connects to daemon', async ({ page }) => {
  12 |     // Navigate to web client
  13 |     await page.goto('/');
  14 | 
  15 |     // Verify page loads with correct title
  16 |     await expect(page).toHaveTitle(/MONOTERMINAL/i);
  17 | 
  18 |     // Verify terminal container is present in DOM
  19 |     const terminal = page.locator('.xterm');
  20 |     await expect(terminal).toBeVisible({ timeout: 10000 });
  21 | 
  22 |     // Verify WebSocket connection is established
  23 |     // The status indicator should show "Connected" within 5 seconds
  24 |     const statusIndicator = page.locator('[data-testid="connection-status"]');
  25 |     await expect(statusIndicator).toContainText(/connected/i, { timeout: 5000 });
  26 | 
  27 |     // Verify no console errors during load
  28 |     const consoleErrors: string[] = [];
  29 |     page.on('console', (msg) => {
  30 |       if (msg.type() === 'error') {
  31 |         consoleErrors.push(msg.text());
  32 |       }
  33 |     });
  34 | 
  35 |     // Allow page to settle
  36 |     await page.waitForTimeout(1000);
  37 | 
  38 |     // Should have no critical errors (allow non-critical warnings)
  39 |     const criticalErrors = consoleErrors.filter(err =>
  40 |       !err.includes('favicon') &&
  41 |       !err.includes('DevTools')
  42 |     );
  43 |     expect(criticalErrors).toHaveLength(0);
  44 |   });
  45 | 
  46 |   test('xterm.js terminal is initialized', async ({ page }) => {
  47 |     await page.goto('/');
  48 | 
  49 |     // Wait for xterm container
  50 |     const xtermScreen = page.locator('.xterm-screen');
  51 |     await expect(xtermScreen).toBeVisible({ timeout: 10000 });
  52 | 
  53 |     // Verify terminal has canvas element (xterm-text-layer)
  54 |     const textLayer = page.locator('.xterm-text-layer');
  55 |     await expect(textLayer).toBeVisible();
  56 | 
  57 |     // Verify terminal is interactive (has cursor)
  58 |     const cursor = page.locator('.xterm-cursor-layer');
  59 |     await expect(cursor).toBeVisible();
  60 |   });
  61 | 
  62 |   test('PWA manifest is loaded', async ({ page }) => {
  63 |     await page.goto('/');
  64 | 
  65 |     // Check for PWA manifest link
  66 |     const manifestLink = page.locator('link[rel="manifest"]');
> 67 |     await expect(manifestLink).toHaveCount(1);
     |                                ^ Error: expect(locator).toHaveCount(expected) failed
  68 | 
  69 |     const manifestHref = await manifestLink.getAttribute('href');
  70 |     expect(manifestHref).toBeTruthy();
  71 | 
  72 |     // Verify manifest file is accessible
  73 |     const manifestResponse = await page.goto(manifestHref!);
  74 |     expect(manifestResponse?.status()).toBe(200);
  75 | 
  76 |     // Verify manifest has required PWA fields
  77 |     const manifestJson = await manifestResponse?.json();
  78 |     expect(manifestJson.name).toBeTruthy();
  79 |     expect(manifestJson.icons).toBeTruthy();
  80 |     expect(manifestJson.start_url).toBeTruthy();
  81 |   });
  82 | 
  83 |   test('service worker registers successfully', async ({ page }) => {
  84 |     await page.goto('/');
  85 | 
  86 |     // Wait for service worker registration
  87 |     await page.waitForFunction(() => {
  88 |       return navigator.serviceWorker.ready;
  89 |     }, { timeout: 10000 });
  90 | 
  91 |     // Verify service worker is active
  92 |     const swState = await page.evaluate(() => {
  93 |       return navigator.serviceWorker.controller?.state;
  94 |     });
  95 | 
  96 |     expect(swState).toBe('activated');
  97 |   });
  98 | });
  99 | 
```