# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: dashboard-embedded.spec.ts >> Phase 1 Criterion #4: Embedded Dashboard (No Separate Service) >> dashboard accessible from main UI - not separate tab
- Location: e2e\dashboard-embedded.spec.ts:137:3

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
  140 |     // Track all opened pages/tabs
  141 |     const pagesBefore = context.pages().length;
  142 | 
  143 |     // Open dashboard
  144 |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
> 145 |     await dashboardToggle.click();
      |                           ^ Error: locator.click: Test timeout of 30000ms exceeded.
  146 | 
  147 |     await page.waitForTimeout(1000);
  148 | 
  149 |     // Verify NO new browser tab was opened
  150 |     const pagesAfter = context.pages().length;
  151 |     expect(pagesAfter).toBe(pagesBefore); // Should be same page, not new tab
  152 | 
  153 |     // Dashboard should be visible within same page
  154 |     const dashboard = page.locator('[data-testid="dashboard-panel"]');
  155 |     await expect(dashboard).toBeVisible();
  156 |   });
  157 | 
  158 |   test('dashboard can be toggled open and closed', async ({ page }) => {
  159 |     await page.goto('/');
  160 | 
  161 |     // Toggle dashboard open
  162 |     const dashboardToggle = page.locator('[data-testid="dashboard-toggle"]');
  163 |     await dashboardToggle.click();
  164 | 
  165 |     // Verify dashboard is visible
  166 |     const dashboard = page.locator('[data-testid="dashboard-panel"]');
  167 |     await expect(dashboard).toBeVisible();
  168 | 
  169 |     // Toggle dashboard closed
  170 |     await dashboardToggle.click();
  171 |     await expect(dashboard).not.toBeVisible();
  172 |   });
  173 | });
  174 | 
  175 | /**
  176 |  * Manual Verification Checklist (from verification plan §3.4):
  177 |  *
  178 |  * 1. Open web client at http://<master-ip>:8080
  179 |  * 2. Verify dashboard toggle/tab exists in UI
  180 |  * 3. Click dashboard → should open immediately (no separate login)
  181 |  * 4. Verify shows:
  182 |  *    [ ] Current org name
  183 |  *    [ ] Active agents list
  184 |  *    [ ] Run status (running/stopped)
  185 |  *    [ ] Health check button
  186 |  *    [ ] Upgrade button
  187 |  * 5. Run health check → should complete within 10s
  188 |  * 6. Verify NO separate browser tab or port (NOT http://localhost:9000/dashboard)
  189 |  *
  190 |  * Evidence Required:
  191 |  * - E2E test report (this file)
  192 |  * - Screenshot showing dashboard panel in web client
  193 |  * - Network trace showing single WebSocket connection
  194 |  *
  195 |  * Implementation Notes:
  196 |  *
  197 |  * This is a STUB for task-9 (framework setup).
  198 |  * Actual implementation in task-10 after:
  199 |  * - Dashboard UI component (frontend-lead)
  200 |  * - Monomind bridge API endpoints
  201 |  * - WebSocket message handlers for dashboard data
  202 |  *
  203 |  * Dependencies:
  204 |  * - task-12: Monomind Dashboard Implementation (frontend-lead)
  205 |  */
  206 | 
```