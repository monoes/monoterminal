# MONOTERMINAL E2E Tests (Playwright)

**Owner:** test-engineer-e2e  
**Framework:** Playwright + TypeScript  
**Purpose:** Browser-based E2E testing for Phase 1 acceptance criteria #2, #3, #4  

---

## Overview

This directory contains Playwright E2E tests for the MONOTERMINAL web client. These tests verify the **Phase 1 acceptance criteria** that require browser testing:

- **Criterion #2:** Web client usable from iPhone/Android browsers on same LAN
- **Criterion #3:** Monomind detection & dismissal workflow
- **Criterion #4:** Embedded dashboard (no separate service)

See: `docs/phase1-acceptance-verification-plan.md` for detailed verification procedures.

---

## Directory Structure

```
web/e2e/
├── fixtures/
│   └── daemon.ts          # Fixture to launch master daemon before tests
├── smoke.spec.ts          # Basic connectivity & sanity tests
├── mobile-browser.spec.ts # Phase 1 Criterion #2 tests
├── monomind-detection.spec.ts  # Phase 1 Criterion #3 tests
├── dashboard-embedded.spec.ts  # Phase 1 Criterion #4 tests
└── README.md              # This file
```

---

## Prerequisites

1. **Playwright installed:**
   ```bash
   npm install
   npx playwright install  # Install browser binaries
   ```

2. **Master daemon built:**
   ```bash
   cd ..
   cargo build --release -p monoterminal-master
   ```

3. **Web client built:**
   ```bash
   npm run build
   ```

---

## Running Tests

### Quick Start

```bash
# Run all E2E tests
npm run test:e2e

# Run tests with UI (interactive mode)
npm run test:e2e:ui

# Run tests in headed mode (see browser)
npm run test:e2e:headed

# Debug specific test
npm run test:e2e:debug

# Show last test report
npm run test:e2e:report
```

### Run Specific Test Files

```bash
# Smoke tests only
npx playwright test smoke.spec.ts

# Mobile browser tests
npx playwright test mobile-browser.spec.ts

# Monomind detection tests
npx playwright test monomind-detection.spec.ts

# Dashboard tests
npx playwright test dashboard-embedded.spec.ts
```

### Run Specific Browser/Device

```bash
# Desktop Chrome only
npx playwright test --project=chromium

# Mobile Safari viewport
npx playwright test --project=mobile-safari

# All mobile viewports
npx playwright test --project=mobile-chrome --project=mobile-safari
```

---

## Test Status

### ✅ Implemented (task-9)
- Playwright configuration
- Daemon fixture (auto-start/stop master)
- Smoke tests (connectivity, PWA basics)
- Test structure for criteria #2, #3, #4

### ⏳ Pending (task-10)
- Full criterion #2 implementation (session creation/attach)
- Full criterion #3 implementation (monomind bridge)
- Full criterion #4 implementation (dashboard UI)

**Current State:** Framework complete, tests are STUBS with placeholder assertions.  
**Next Step:** task-10 will replace TODOs with real implementations.

---

## Daemon Fixture

The `daemon.ts` fixture automatically:
1. Launches `monoterminal-master.exe` on port 8080
2. Waits for startup (3 seconds)
3. Provides daemon instance to test
4. Gracefully shuts down after test
5. Captures stdout/stderr for debugging

**Usage:**
```typescript
import { test, expect } from './fixtures/daemon';

test('my test', async ({ page, daemon }) => {
  // daemon is running, page is ready
  await page.goto('/');
  // ...
});
```

---

## Browser/Device Matrix

| Project | Browser | Viewport | Purpose |
|---------|---------|----------|---------|
| **chromium** | Desktop Chrome | 1280×720 | Primary desktop testing |
| **firefox** | Desktop Firefox | 1280×720 | Cross-browser validation |
| **webkit** | Desktop Safari | 1280×720 | Safari compatibility |
| **mobile-chrome** | Pixel 5 emulation | 393×851 | Android testing (Criterion #2) |
| **mobile-safari** | iPhone 12 emulation | 390×844 | iOS testing (Criterion #2) |

---

## CI Integration

The Playwright config is CI-ready:
- `fullyParallel: false` — sequential execution for daemon isolation
- `forbidOnly: !!process.env.CI` — fail if `test.only` committed
- `retries: process.env.CI ? 2 : 0` — retry flaky tests on CI
- `reporter: process.env.CI ? 'github' : 'html'` — GitHub Actions reporter

**GitHub Actions workflow** (add to `.github/workflows/e2e.yml`):
```yaml
name: E2E Tests
on: [pull_request]
jobs:
  e2e:
    runs-on: windows-2022  # Phase 1 is Windows-only
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p monoterminal-master
      - run: cd web && npm ci
      - run: npx playwright install --with-deps
      - run: cd web && npm run build
      - run: cd web && npm run test:e2e
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: playwright-report
          path: web/playwright-report/
```

---

## Debugging Tests

### View Test in Browser
```bash
npm run test:e2e:headed
```

### Debug Mode (step through)
```bash
npm run test:e2e:debug
```

### Playwright Inspector
```bash
npx playwright test --debug
```

### Show Trace
```bash
npx playwright show-trace trace.zip
```

---

## Evidence Collection (Verification Plan)

For Phase 1 gate approval, collect:

### Criterion #2 Evidence
- [ ] E2E test report (this suite)
- [ ] Manual test checklist (physical iPhone + Android)
- [ ] Video recording of full workflow on real devices

### Criterion #3 Evidence
- [ ] E2E test report
- [ ] Screenshots showing suggestion UI
- [ ] Manual verification on 3 different projects

### Criterion #4 Evidence
- [ ] E2E test report
- [ ] Screenshot showing dashboard panel
- [ ] Network trace showing single WebSocket connection

**QA Lead approval:** All evidence must be submitted before Phase 1 → Phase 2 gate.

---

## Manual Testing Checklist

### iOS Safari (iPhone 12+, iOS 16+)
- [ ] Can access `http://<master-ip>:8080` on LAN
- [ ] Terminal renders correctly (no layout breaks)
- [ ] Touch keyboard appears when tapping terminal
- [ ] Can type commands and see output
- [ ] Touch scrolling works smoothly
- [ ] PWA "Add to Home Screen" works
- [ ] No console errors

### Android Chrome (Pixel 6+, Android 12+)
- [ ] Same checklist as iOS
- [ ] PWA install banner appears
- [ ] Haptic feedback on key press (optional)

---

## Common Issues

### Daemon doesn't start
- **Check:** Is `monoterminal-master.exe` built? (`cargo build --release`)
- **Check:** Is port 8080 already in use? (`netstat -ano | findstr :8080`)
- **Fix:** Kill existing process or change port in fixture

### Web client doesn't load
- **Check:** Is web client built? (`npm run build`)
- **Check:** Does `npm run preview` work standalone?
- **Fix:** Run `npm run build` before tests

### Tests timeout
- **Check:** Increase timeout in test: `await expect(...).toBeVisible({ timeout: 10000 });`
- **Check:** Daemon logs for errors (captured in test output)

### Mobile viewport tests fail
- **Check:** Is viewport set correctly in test?
- **Check:** Is responsive CSS working? (test in browser DevTools)

---

## Links

- **SRS Testing Requirements:** `docs/monoterminal-srs.md` §6.1
- **Phase 1 Verification Plan:** `docs/phase1-acceptance-verification-plan.md`
- **Playwright Docs:** https://playwright.dev
- **Task Tracker:** task-9 (framework setup), task-10 (test implementation)

---

**Status:** ✅ Framework complete (task-9)  
**Next:** ⏳ Implement test TODOs (task-10)  
**Owner:** test-engineer-e2e  
**Last Updated:** 2026-08-15
