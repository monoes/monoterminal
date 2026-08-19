# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: dashboard-embedded.spec.ts >> Phase 1 Criterion #4: Embedded Dashboard (No Separate Service) >> health check executes and displays result
- Location: e2e\dashboard-embedded.spec.ts:34:3

# Error details

```
Test timeout of 30000ms exceeded.
```

```
Error: locator.click: Test timeout of 30000ms exceeded.
Call log:
  - waiting for locator('[data-testid="dashboard-toggle"]')

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
  1   | /**
  2   |  * E2E Test: Embedded Dashboard (Phase 1 Criterion #4)
  3   |  *
  4   |  * Verifies Phase 1 acceptance criterion #4:
  5   |  * "Embedded dashboard reflects live master state with no separate service to start"
  6   |  *
  7   |  * Test procedures from: docs/phase1-acceptance-verification-plan.md §3.4
  8   |  */
  9   | 
  10  | import { test, expect } from './fixtures/daemon';
  11  | 
  12  | test.describe('Phase 1 Criterion #4: Embedded Dashboard (No Separate Service)', () => {
  13  | 
  14  |   test('dashboard embedded in web client - same port/domain', async ({ page }) => {
  15  |     // 1. Navigate to web client
  16  |     await page.goto('/');
  17  |     await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });
  18  | 
  19  |     // 2. Open dashboard panel (should be in same page, not separate service)
  20  |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  21  | 
  22  |     await expect(dashboardToggle).toBeVisible();
  23  |     await dashboardToggle.click();
  24  | 
  25  |     // 3. Verify dashboard shows monomind org status
  26  |     const orgStatus = page.locator('[data-testid="org-status"]');
  27  |     await expect(orgStatus).toBeVisible({ timeout: 3000 });
  28  | 
  29  |     // 4. Verify agent count is displayed
  30  |     const agentCount = page.locator('[data-testid="agent-count"]');
  31  |     await expect(agentCount).toContainText(/\d+ agents?/i);
  32  |   });
  33  | 
  34  |   test('health check executes and displays result', async ({ page }) => {
  35  |     await page.goto('/');
  36  | 
  37  |     // Open dashboard
  38  |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
> 39  |     await dashboardToggle.click();
      |                           ^ Error: locator.click: Test timeout of 30000ms exceeded.
  40  | 
  41  |     // Click health check button
  42  |     const healthCheckButton = page.locator('[data-testid="run-health-check"]');
  43  |     await expect(healthCheckButton).toBeVisible();
  44  |     await healthCheckButton.click();
  45  | 
  46  |     // Verify health check result appears within 10 seconds
  47  |     const healthResult = page.locator('[data-testid="health-result"]');
  48  |     await expect(healthResult).toContainText(/✅|✓|pass/i, { timeout: 10000 });
  49  |   });
  50  | 
  51  |   test('one-click upgrade button present', async ({ page }) => {
  52  |     await page.goto('/');
  53  | 
  54  |     // Open dashboard
  55  |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  56  |     await dashboardToggle.click();
  57  | 
  58  |     // Verify upgrade button exists and is clickable
  59  |     const upgradeButton = page.locator('[data-testid="upgrade-button"]');
  60  |     await expect(upgradeButton).toBeVisible();
  61  |     await expect(upgradeButton).toBeEnabled();
  62  |   });
  63  | 
  64  |   test('dashboard uses same WebSocket connection - no separate port', async ({ page }) => {
  65  |     // Track all network requests
  66  |     const requests: string[] = [];
  67  |     page.on('request', req => requests.push(req.url()));
  68  | 
  69  |     await page.goto('/');
  70  | 
  71  |     // TODO: Open dashboard
  72  |     // const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  73  |     // await dashboardToggle.click();
  74  | 
  75  |     // Wait for dashboard to load
  76  |     await page.waitForTimeout(2000);
  77  | 
  78  |     // Verify NO requests to separate port (e.g., :9000, :9001, etc.)
  79  |     const separatePortRequests = requests.filter(url => {
  80  |       // Dashboard should NOT use a different port than 8080
  81  |       return url.includes(':9000') ||
  82  |              url.includes(':9001') ||
  83  |              url.includes(':3000') ||
  84  |              (url.includes('localhost') && !url.includes(':8080'));
  85  |     });
  86  | 
  87  |     expect(separatePortRequests).toHaveLength(0);
  88  |   });
  89  | 
  90  |   test('no separate authentication required - uses session JWT', async ({ page }) => {
  91  |     const requests: string[] = [];
  92  |     page.on('request', req => requests.push(req.url()));
  93  | 
  94  |     await page.goto('/');
  95  | 
  96  |     // TODO: Open dashboard
  97  |     // const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  98  |     // await dashboardToggle.click();
  99  | 
  100 |     await page.waitForTimeout(2000);
  101 | 
  102 |     // Assert: No separate OAuth flow or token exchange
  103 |     const oauthRequests = requests.filter(url =>
  104 |       url.includes('/oauth') ||
  105 |       url.includes('/token') ||
  106 |       url.includes('/auth/callback')
  107 |     );
  108 | 
  109 |     expect(oauthRequests).toHaveLength(0);
  110 |   });
  111 | 
  112 |   test('dashboard shows live monomind state', async ({ page }) => {
  113 |     await page.goto('/');
  114 | 
  115 |     // Open dashboard
  116 |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  117 |     await dashboardToggle.click();
  118 | 
  119 |     // Verify dashboard displays required monomind information
  120 |     // Expected fields:
  121 |     // - Current org name
  122 |     // - Active agents list
  123 |     // - Run status (running/stopped)
  124 |     // - Health check button
  125 |     // - Upgrade button
  126 | 
  127 |     const orgName = page.locator('[data-testid="org-name"]');
  128 |     await expect(orgName).toBeVisible();
  129 | 
  130 |     const agentsList = page.locator('[data-testid="agents-list"]');
  131 |     await expect(agentsList).toBeVisible();
  132 | 
  133 |     const runStatus = page.locator('[data-testid="run-status"]');
  134 |     await expect(runStatus).toContainText(/running|stopped/i);
  135 |   });
  136 | 
  137 |   test('dashboard accessible from main UI - not separate tab', async ({ page, context }) => {
  138 |     await page.goto('/');
  139 | 
```