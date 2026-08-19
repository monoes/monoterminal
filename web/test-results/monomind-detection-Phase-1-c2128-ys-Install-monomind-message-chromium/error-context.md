# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: monomind-detection.spec.ts >> Phase 1 Criterion #3: Monomind Detection & Dismissal >> Scenario A: Project without .monomind/ >> suggestion displays "Install monomind?" message
- Location: e2e\monomind-detection.spec.ts:36:5

# Error details

```
Error: expect(locator).toBeVisible() failed

Locator: locator('[data-testid="monomind-suggestion"]')
Expected: visible
Timeout: 5000ms
Error: element(s) not found

Call log:
  - Expect "toBeVisible" with timeout 5000ms
  - waiting for locator('[data-testid="monomind-suggestion"]')

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
  2   |  * E2E Test: Monomind Detection & Dismissal (Phase 1 Criterion #3)
  3   |  *
  4   |  * Verifies Phase 1 acceptance criterion #3:
  5   |  * "Monomind suggestion fires correctly for projects without .monomind/,
  6   |  *  and stays dismissed once declined"
  7   |  *
  8   |  * Test procedures from: docs/phase1-acceptance-verification-plan.md §3.3
  9   |  */
  10  | 
  11  | import { test, expect } from './fixtures/daemon';
  12  | 
  13  | test.describe('Phase 1 Criterion #3: Monomind Detection & Dismissal', () => {
  14  | 
  15  |   test.describe('Scenario A: Project without .monomind/', () => {
  16  |     test('suggestion appears within 5 seconds for directory without monomind', async ({ page }) => {
  17  |       // 1. Navigate to web client
  18  |       await page.goto('/');
  19  |       await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });
  20  | 
  21  |       // 2. Create/attach to session in directory without .monomind/
  22  |       // TODO: Implement session creation in directory without monomind
  23  |       // For now, this is a stub that verifies the UI components exist
  24  | 
  25  |       // 3. Verify suggestion banner/notification appears
  26  |       // Should appear within 5 seconds of session creation
  27  |       const suggestionBanner = page.locator('[data-testid="monomind-suggestion"]');
  28  | 
  29  |       await expect(suggestionBanner).toBeVisible({ timeout: 5000 });
  30  | 
  31  |       // 4. Verify suggestion has correct content
  32  |       await expect(suggestionBanner).toContainText(/install monomind/i);
  33  |       await expect(suggestionBanner).toContainText(/dismiss/i);
  34  |     });
  35  | 
  36  |     test('suggestion displays "Install monomind?" message', async ({ page }) => {
  37  |       await page.goto('/');
  38  | 
  39  |       // TODO: Create session without .monomind/
  40  |       const suggestionBanner = page.locator('[data-testid="monomind-suggestion"]');
> 41  |       await expect(suggestionBanner).toBeVisible({ timeout: 5000 });
      |                                      ^ Error: expect(locator).toBeVisible() failed
  42  |       await expect(suggestionBanner).toContainText(/install monomind/i);
  43  |     });
  44  |   });
  45  | 
  46  |   test.describe('Scenario B: Dismiss suggestion', () => {
  47  |     test('dismissed suggestion does not reappear on reload', async ({ page }) => {
  48  |       // 1. Load page with suggestion
  49  |       await page.goto('/');
  50  |       await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });
  51  | 
  52  |       // 2. Dismiss suggestion
  53  |       const dismissButton = page.locator('[data-testid="monomind-suggestion-dismiss"]');
  54  |       await dismissButton.click();
  55  | 
  56  |       // 3. Verify suggestion disappears
  57  |       const suggestionBanner = page.locator('[data-testid="monomind-suggestion"]');
  58  |       await expect(suggestionBanner).not.toBeVisible();
  59  | 
  60  |       // 4. Reload web client
  61  |       await page.reload();
  62  | 
  63  |       // 5. Verify suggestion does NOT reappear
  64  |       await page.waitForTimeout(6000); // Wait longer than 5s trigger
  65  |       await expect(suggestionBanner).not.toBeVisible();
  66  |     });
  67  | 
  68  |     test('dismissal persists in localStorage or SQLite', async ({ page }) => {
  69  |       await page.goto('/');
  70  | 
  71  |       // TODO: Dismiss suggestion
  72  |       // TODO: Check persistence mechanism
  73  |       // Option 1: localStorage
  74  |       // const dismissed = await page.evaluate(() => {
  75  |       //   return localStorage.getItem('monomind-suggestion-dismissed');
  76  |       // });
  77  |       // expect(dismissed).toBeTruthy();
  78  | 
  79  |       // Option 2: SQLite (verified via API)
  80  |       // const response = await page.request.get('/api/monomind/suggestion-status');
  81  |       // const data = await response.json();
  82  |       // expect(data.dismissed).toBe(true);
  83  | 
  84  |       // Stub placeholder
  85  |       expect(true).toBeTruthy();
  86  |     });
  87  |   });
  88  | 
  89  |   test.describe('Scenario C: Project with .monomind/', () => {
  90  |     test('no suggestion appears for directory with monomind', async ({ page }) => {
  91  |       // 1. Navigate to web client
  92  |       await page.goto('/');
  93  |       await expect(page.locator('.xterm')).toBeVisible({ timeout: 10000 });
  94  | 
  95  |       // 2. Create/attach to session in directory WITH .monomind/
  96  |       // TODO: Implement session creation in directory with monomind
  97  | 
  98  |       // 3. Wait for potential suggestion trigger (5+ seconds)
  99  |       await page.waitForTimeout(6000);
  100 | 
  101 |       // 4. Verify NO suggestion appears
  102 |       const suggestionBanner = page.locator('[data-testid="monomind-suggestion"]');
  103 |       await expect(suggestionBanner).not.toBeVisible();
  104 |     });
  105 |   });
  106 | 
  107 |   test.describe('Multi-session independence', () => {
  108 |     test('dismissing in session A does not affect session B', async ({ page }) => {
  109 |       // Verify each session checks independently
  110 |       // Session A dismiss should NOT affect session B suggestion
  111 | 
  112 |       // TODO: Create session A without monomind
  113 |       // TODO: Dismiss suggestion in session A
  114 |       // TODO: Create session B (different directory, also without monomind)
  115 |       // TODO: Verify session B shows suggestion (not affected by A's dismissal)
  116 | 
  117 |       // Stub: Multi-session test requires session management API
  118 |       expect(true).toBeTruthy(); // Keep stub - needs session creation API
  119 |     });
  120 |   });
  121 | 
  122 |   test.describe('Daemon restart persistence', () => {
  123 |     test('dismissal persists across master daemon restart', async ({ page }) => {
  124 |       // 1. Dismiss suggestion
  125 |       // 2. Stop daemon fixture
  126 |       // 3. Start new daemon
  127 |       // 4. Verify suggestion stays dismissed
  128 | 
  129 |       // TODO: Implement daemon restart test
  130 |       // This may require custom fixture extension
  131 | 
  132 |       // Stub: Requires daemon restart fixture
  133 |       expect(true).toBeTruthy(); // Keep stub - needs daemon restart capability
  134 |     });
  135 |   });
  136 | });
  137 | 
  138 | /**
  139 |  * Implementation Notes:
  140 |  *
  141 |  * These tests are STUBS for framework setup (task-9).
```