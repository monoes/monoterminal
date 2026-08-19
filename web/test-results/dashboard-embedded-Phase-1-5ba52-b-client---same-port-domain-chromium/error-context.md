# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: dashboard-embedded.spec.ts >> Phase 1 Criterion #4: Embedded Dashboard (No Separate Service) >> dashboard embedded in web client - same port/domain
- Location: e2e\dashboard-embedded.spec.ts:14:3

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: locator('.xterm')
Expected: visible
Timeout: 10000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 10000ms
  - waiting for locator('.xterm')

```

```yaml
- region "Notifications alt+T"
- link "llama-ui":
  - /url: "#/"
  - heading "llama-ui" [level=1]
- link "New chat ⌘ O":
  - /url: "?new_chat=true#/"
  - img
  - text: New chat
  - img
  - text: ⌘ O
- button "Search ⌘ K":
  - img
  - text: Search ⌘ K
- link "MCP Servers":
  - /url: "#/mcp-servers"
  - img
  - text: MCP Servers
- link "Settings":
  - /url: "#/settings"
  - img
  - text: Settings
- text: Recent conversations
- list:
  - paragraph: No conversations yet
- button "Toggle Sidebar":
  - img
  - text: Toggle Sidebar
- complementary:
  - button "New chat":
    - button "New chat":
      - img
  - button "Search":
    - button "Search":
      - img
  - button "MCP Servers":
    - button "MCP Servers":
      - img
  - button "Settings":
    - button "Settings":
      - img
- main:
  - main "Chat interface with file drop zone":
    - heading "Hello there" [level=1]
    - paragraph: Type a message or upload files to get started
    - button "Scroll to bottom":
      - img
    - textbox "Type a message..."
    - button "Add files, prompts, tools or MCP Servers":
      - text: Add files, prompts, tools or MCP Servers
      - img
    - button "qwen3 35B":
      - img
      - text: qwen3 35B
    - button "Send" [disabled]:
      - text: Send
      - img
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
> 17  |     await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });
      |                                          ^ Error: expect(locator).toBeVisible() failed
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
  39  |     await dashboardToggle.click();
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
```