# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: mobile-browser.spec.ts >> Phase 1 Criterion #2: Mobile Browser Usability >> PWA "Add to Home Screen" metadata present
- Location: e2e\mobile-browser.spec.ts:110:3

# Error details

```
Error: expect(received).toBeTruthy()

Received: false
```

# Page snapshot

```yaml
- generic [ref=e2]:
  - region "Notifications alt+T"
  - generic [ref=e4]:
    - generic:
      - generic:
        - generic:
          - generic:
            - generic:
              - generic:
                - generic:
                  - generic:
                    - generic:
                      - link "llama-ui":
                        - /url: "#/"
                        - heading "llama-ui" [level=1]
                      - button "Close sidebar"
                    - generic:
                      - link "New chat ⌘ O":
                        - /url: "?new_chat=true#/"
                        - generic: New chat
                        - generic:
                          - generic: ⌘
                          - text: O
                      - button "Search ⌘ K":
                        - generic: Search
                        - generic:
                          - generic: ⌘
                          - text: K
                      - link "MCP Servers":
                        - /url: "#/mcp-servers"
                      - link "Settings":
                        - /url: "#/settings"
                  - generic:
                    - generic: Recent conversations
                    - generic:
                      - list:
                        - generic:
                          - paragraph: No conversations yet
    - button "Toggle Sidebar" [ref=e5] [cursor=pointer]
    - main [ref=e7]:
      - generic [ref=e9]:
        - generic [ref=e10]:
          - heading "Connecting to Server" [level=2] [ref=e13]
          - paragraph [ref=e14]: Initializing connection to server...
        - generic [ref=e15]: Connecting...
```

# Test source

```ts
  23  |     await page.goto('/');
  24  | 
  25  |     // 2. Verify PWA installability
  26  |     // Note: Playwright can't trigger real beforeinstallprompt, but we can check manifest
  27  |     const manifestLink = page.locator('link[rel="manifest"]');
  28  |     await expect(manifestLink).toHaveCount(1);
  29  | 
  30  |     // 3. Verify page renders correctly on mobile viewport
  31  |     const terminal = page.locator('.xterm');
  32  |     await expect(terminal).toBeVisible({ timeout: 10000 });
  33  | 
  34  |     // 4. Connect to session (stub - actual session creation in task-10)
  35  |     // TODO: Implement session creation flow
  36  |     // await page.click('[data-testid="connect-button"]');
  37  |     // await page.fill('[data-testid="session-id-input"]', 'test-session-mobile');
  38  |     // await page.click('[data-testid="attach-button"]');
  39  | 
  40  |     // 5. Verify touch interaction is possible
  41  |     // Click on terminal to focus
  42  |     await terminal.click();
  43  | 
  44  |     // TODO: Type command and verify output (requires session implementation)
  45  |     // await page.keyboard.type('echo "Hello from mobile"\n');
  46  |     // await expect(terminal).toContainText('Hello from mobile', { timeout: 5000 });
  47  | 
  48  |     // 6. Verify touch scrolling works
  49  |     const xtermViewport = page.locator('.xterm-viewport');
  50  |     await expect(xtermViewport).toBeVisible();
  51  | 
  52  |     // Simulate touch scroll
  53  |     await page.touchscreen.tap(200, 300);
  54  | 
  55  |     // 7. Verify no layout breaks on mobile
  56  |     const viewportWidth = await page.evaluate(() => window.innerWidth);
  57  |     expect(viewportWidth).toBeLessThanOrEqual(375);
  58  | 
  59  |     // Terminal should not overflow viewport
  60  |     const terminalBox = await terminal.boundingBox();
  61  |     expect(terminalBox?.width).toBeLessThanOrEqual(viewportWidth);
  62  |   });
  63  | 
  64  |   test('mobile browser full workflow - Android Chrome viewport', async ({ page, browserName }) => {
  65  |     test.skip(browserName !== 'chromium', 'This test is for Chrome only');
  66  | 
  67  |     // 1. Navigate to web client
  68  |     await page.goto('/');
  69  | 
  70  |     // 2. Verify PWA install banner context (Chrome-specific)
  71  |     // Check for required PWA metadata
  72  |     const themeColorMeta = page.locator('meta[name="theme-color"]');
  73  |     await expect(themeColorMeta).toHaveCount(1);
  74  | 
  75  |     // 3. Verify terminal renders on Android viewport
  76  |     const terminal = page.locator('.xterm');
  77  |     await expect(terminal).toBeVisible({ timeout: 10000 });
  78  | 
  79  |     // 4. Touch keyboard should appear when tapping terminal
  80  |     // Note: Playwright can't fully test real mobile keyboard, but we verify focus
  81  |     await terminal.click();
  82  |     const isFocused = await page.evaluate(() => {
  83  |       const activeElement = document.activeElement;
  84  |       return activeElement?.classList.contains('xterm') ||
  85  |              activeElement?.closest('.xterm') !== null;
  86  |     });
  87  |     expect(isFocused).toBeTruthy();
  88  | 
  89  |     // 5. Verify touch scrolling works
  90  |     const xtermViewport = page.locator('.xterm-viewport');
  91  |     await expect(xtermViewport).toBeVisible();
  92  | 
  93  |     // 6. Verify no console errors
  94  |     const consoleErrors: string[] = [];
  95  |     page.on('console', (msg) => {
  96  |       if (msg.type() === 'error') {
  97  |         consoleErrors.push(msg.text());
  98  |       }
  99  |     });
  100 | 
  101 |     await page.waitForTimeout(2000);
  102 | 
  103 |     const criticalErrors = consoleErrors.filter(err =>
  104 |       !err.includes('favicon') &&
  105 |       !err.includes('DevTools')
  106 |     );
  107 |     expect(criticalErrors).toHaveLength(0);
  108 |   });
  109 | 
  110 |   test('PWA "Add to Home Screen" metadata present', async ({ page }) => {
  111 |     await page.goto('/');
  112 | 
  113 |     // Verify all required PWA metadata
  114 |     const checks = await page.evaluate(() => {
  115 |       return {
  116 |         hasManifest: !!document.querySelector('link[rel="manifest"]'),
  117 |         hasThemeColor: !!document.querySelector('meta[name="theme-color"]'),
  118 |         hasViewport: !!document.querySelector('meta[name="viewport"]'),
  119 |         hasAppleTouchIcon: !!document.querySelector('link[rel="apple-touch-icon"]'),
  120 |       };
  121 |     });
  122 | 
> 123 |     expect(checks.hasManifest).toBeTruthy();
      |                                ^ Error: expect(received).toBeTruthy()
  124 |     expect(checks.hasThemeColor).toBeTruthy();
  125 |     expect(checks.hasViewport).toBeTruthy();
  126 |     expect(checks.hasAppleTouchIcon).toBeTruthy();
  127 |   });
  128 | 
  129 |   test('responsive layout adapts to portrait orientation', async ({ page }) => {
  130 |     // Set to mobile portrait (iPhone 12)
  131 |     await page.setViewportSize({ width: 390, height: 844 });
  132 |     await page.goto('/');
  133 | 
  134 |     const terminal = page.locator('.xterm');
  135 |     await expect(terminal).toBeVisible();
  136 | 
  137 |     // Terminal should use full width in portrait
  138 |     const terminalBox = await terminal.boundingBox();
  139 |     expect(terminalBox?.width).toBeGreaterThan(300); // Reasonable mobile width
  140 |   });
  141 | 
  142 |   test('responsive layout adapts to landscape orientation', async ({ page }) => {
  143 |     // Set to mobile landscape
  144 |     await page.setViewportSize({ width: 844, height: 390 });
  145 |     await page.goto('/');
  146 | 
  147 |     const terminal = page.locator('.xterm');
  148 |     await expect(terminal).toBeVisible();
  149 | 
  150 |     // Terminal should adapt to landscape
  151 |     const terminalBox = await terminal.boundingBox();
  152 |     expect(terminalBox?.width).toBeGreaterThan(700);
  153 |   });
  154 | });
  155 | 
  156 | /**
  157 |  * Manual Testing Checklist (to be performed on physical devices)
  158 |  *
  159 |  * iOS Safari (iPhone 12+, iOS 16+):
  160 |  * [ ] Can access http://<master-ip>:8080 on LAN
  161 |  * [ ] Terminal renders correctly (no layout breaks)
  162 |  * [ ] Touch keyboard appears when tapping terminal
  163 |  * [ ] Can type commands and see output
  164 |  * [ ] Touch scrolling works smoothly
  165 |  * [ ] PWA "Add to Home Screen" works
  166 |  * [ ] No console errors
  167 |  *
  168 |  * Android Chrome (Pixel 6+, Android 12+):
  169 |  * [ ] Same checklist as iOS
  170 |  * [ ] PWA install banner appears
  171 |  * [ ] Haptic feedback on key press (optional)
  172 |  *
  173 |  * Evidence: Video recording of full workflow on real devices required
  174 |  */
  175 | 
```