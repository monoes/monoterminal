# PWA Installability Test Plan - MONOTERMINAL
**Date:** 2026-08-15  
**Task:** task-4  
**Status:** Ready to Execute (pending Bash tool approval)  
**Priority:** URGENT - Critical path for Phase 1 gate (3-5 days)

---

## Pre-Execution Configuration Verification ✅

### Manifest Configuration (vite.config.ts)
**Status: ✅ ALL VALID**

- ✅ `name: "MONOTERMINAL"`
- ✅ `short_name: "MonoTerm"`
- ✅ `description: "Next-generation terminal emulator with P2P collaboration"`
- ✅ `theme_color: "#1e1e1e"`
- ✅ `background_color: "#1e1e1e"`
- ✅ `display: "standalone"`
- ✅ `display_override: ["window-controls-overlay"]` (Chrome 90+)
- ✅ `scope: "/"`
- ✅ `start_url: "/"`
- ✅ `orientation: "any"`

### Icons Configuration
- ✅ `pwa-192x192.png` - Standard icon
- ✅ `pwa-512x512.png` - Large icon
- ✅ `pwa-512x512.png` (maskable) - Adaptive icon for Android

### File Handlers
- ✅ `.txt` files
- ✅ `.log` files
- ✅ Action: `/` (opens in app root)

### Service Worker (Workbox)
- ✅ Auto-update registration
- ✅ Asset patterns: `**/*.{js,css,html,ico,png,svg,woff2}`
- ✅ Runtime caching: Google Fonts (CacheFirst, 1 year)
- ✅ Include assets: favicon.ico, robots.txt, apple-touch-icon.png

### Build Scripts
- ✅ `npm run build` - TypeScript compile + Vite build
- ✅ `npm run preview` - Preview server (default port 4173)
- ✅ `npm run pwa:check` - Validation helper

**Conclusion:** PWA configuration is complete and correct per SRS §2.2 requirements.

---

## Execution Steps

### Step 1: Build Production Bundle
```bash
cd web
npm run build
```

**Expected Output:**
- `dist/` directory created
- `dist/manifest.webmanifest` generated
- `dist/sw.js` service worker generated
- No TypeScript errors
- Build completes successfully

### Step 2: Start Preview Server
```bash
npm run preview
```

**Expected Output:**
- Server starts on http://localhost:4173
- Preview mode serves production build
- Console shows "Local: http://localhost:4173"

---

## Test Checklist

### Chrome Desktop (Primary Test Platform)

#### 1. Manifest Validation
- [ ] Navigate to http://localhost:4173
- [ ] Open DevTools (F12)
- [ ] Go to: Application → Manifest
- [ ] Verify all fields:
  - [ ] Name: "MONOTERMINAL"
  - [ ] Short name: "MonoTerm"
  - [ ] Description: "Next-generation terminal emulator with P2P collaboration"
  - [ ] Start URL: "/"
  - [ ] Theme color: #1e1e1e
  - [ ] Background color: #1e1e1e
  - [ ] Display: "standalone"
  - [ ] Display override: "window-controls-overlay"
  - [ ] Orientation: "any"
  - [ ] Icons: 3 entries (192x192, 512x512, 512x512 maskable)
  - [ ] File handlers: text/plain (.txt, .log)

**Screenshot:** manifest-validation.png

#### 2. Install Button (Desktop)
- [ ] Look for "Install" button in Chrome address bar (right side)
- [ ] If not visible, check Application → Manifest → "Installability" section
- [ ] Verify no errors/warnings listed
- [ ] Click "Install" button
- [ ] Confirm installation dialog appears
- [ ] Click "Install" in dialog
- [ ] Verify PWA launches in separate window

**Screenshot:** install-button.png, installed-app.png

#### 3. Window Controls Overlay
- [ ] In installed PWA window, verify custom title bar
- [ ] Chrome's traffic light buttons (close/minimize/maximize) should be integrated into app UI
- [ ] No separate Chrome browser chrome visible
- [ ] App looks native, not like a browser tab

**Screenshot:** window-controls-overlay.png

#### 4. Service Worker Registration
- [ ] DevTools → Application → Service Workers
- [ ] Verify service worker entry: "monoterminal-web" or similar
- [ ] Status: "activated and running"
- [ ] Source: sw.js
- [ ] Scope: /
- [ ] No errors in console

**Screenshot:** service-worker.png

#### 5. Cache Storage
- [ ] DevTools → Application → Cache Storage
- [ ] Verify Workbox caches present:
  - [ ] workbox-precache-v2-... (or similar)
  - [ ] google-fonts-cache (if fonts loaded)
- [ ] Expand precache → Verify assets cached:
  - [ ] index.html
  - [ ] CSS files
  - [ ] JS bundles
  - [ ] Icon files

**Screenshot:** cache-storage.png

#### 6. Offline App Shell
- [ ] With PWA running (browser or installed app)
- [ ] DevTools → Network → Check "Offline" checkbox
- [ ] Refresh page (Ctrl+R)
- [ ] **Expected:** App shell loads (header, layout visible)
- [ ] **Not expected:** "No internet connection" browser error
- [ ] Verify ConnectionStatus component shows "Disconnected" state
- [ ] Re-enable network, verify reconnection

**Screenshot:** offline-app-shell.png

#### 7. File Handling (Desktop PWA)
- [ ] Create test files on desktop:
  - [ ] test.txt (with some text content)
  - [ ] test.log (with log-like content)
- [ ] Right-click test.txt → "Open with" → Select MONOTERMINAL
- [ ] Verify file opens in PWA (or browser, depending on OS support)
- [ ] Repeat for test.log
- [ ] **Note:** File handling requires installed PWA and may not work in preview mode

**Screenshot:** file-handling.png (if working)

**Known Limitation:** File Handling API is Chrome 102+ only, may not work on all platforms.

---

### Mobile Testing (If Available)

#### Chrome Android
- [ ] Navigate to http://<your-ip>:4173 from Android device
- [ ] Wait for "Add to Home Screen" banner/prompt
- [ ] If no prompt, Chrome menu → "Add to Home Screen"
- [ ] Install app
- [ ] Open from home screen
- [ ] Verify standalone mode (no browser UI)
- [ ] Test basic terminal functionality
- [ ] Test mobile keyboard accessory row

**Screenshot:** android-install.png, android-standalone.png

#### Safari iOS
- [ ] Navigate to http://<your-ip>:4173 from iOS device
- [ ] Tap Share button → "Add to Home Screen"
- [ ] Name: "MONOTERMINAL"
- [ ] Tap "Add"
- [ ] Open from home screen
- [ ] Verify standalone mode
- [ ] Test basic terminal functionality
- [ ] **Note:** iOS Safari suspends WebRTC on background/lock (documented limitation in SRS §2.2)

**Screenshot:** ios-install.png, ios-standalone.png

**Background Limitation Test:**
- [ ] With app open, lock iPhone screen
- [ ] Wait 10 seconds
- [ ] Unlock screen
- [ ] **Expected:** App reconnects within <10s (SRS §2.2 target)
- [ ] Scrollback resyncs automatically

---

## Expected Results

### All Should PASS ✅
Based on code review, all tests should pass:
- Manifest fully configured
- Service worker properly configured via vite-plugin-pwa
- Icons generated and present
- No known configuration issues

### Potential Issues (Document if Found)

| Issue | Severity | Action |
|-------|----------|--------|
| Icons missing or wrong size | High | Regenerate via `npm run generate-icons` |
| Service worker not registering | High | Check console errors, report to eng-director |
| Install button not appearing | Medium | Check DevTools → Manifest → Installability |
| File handling not working | Low | OS/browser limitation, document and move on |
| Offline mode shows error | High | Service worker cache issue, report immediately |

---

## Documentation Template

Copy this template for final report to qa-lead:

```markdown
# PWA Installability Test Results - MONOTERMINAL
**Date:** 2026-08-15
**Tester:** frontend-engineer
**Build:** web/dist (production)
**Preview URL:** http://localhost:4173

---

## Test Results Summary

| Test Category | Status | Notes |
|---------------|--------|-------|
| Manifest Configuration | ✅/❌ | [details] |
| Desktop Install Button | ✅/❌ | [details] |
| Window Controls Overlay | ✅/❌ | [details] |
| Service Worker Registration | ✅/❌ | [details] |
| Cache Storage | ✅/❌ | [details] |
| Offline App Shell | ✅/❌ | [details] |
| File Handling | ✅/❌/N/A | [details] |
| Chrome Android | ✅/❌/NOT_TESTED | [details] |
| Safari iOS | ✅/❌/NOT_TESTED | [details] |

---

## Detailed Findings

### Chrome Desktop
[Describe results, any issues, screenshots]

### Mobile (if tested)
[Describe results, any issues, screenshots]

---

## Issues Found

**None** / **[Number] issues found:**

1. **[Issue Title]**
   - Severity: High/Medium/Low
   - Description: [what went wrong]
   - Steps to reproduce: [how to see it]
   - Expected behavior: [what should happen]
   - Actual behavior: [what actually happened]
   - Recommendation: [how to fix]

---

## Recommendations

- [ ] [Any fixes needed]
- [ ] [Any improvements]
- [ ] [Any follow-up tests]

---

## Screenshots

Attached:
- manifest-validation.png
- install-button.png
- installed-app.png
- window-controls-overlay.png
- service-worker.png
- cache-storage.png
- offline-app-shell.png
- [mobile screenshots if tested]

---

## SRS Compliance

Per SRS §2.2 requirements:
- ✅/❌ Desktop installability (Chrome 90+)
- ✅/❌ Window Controls Overlay (Chrome 90+)
- ✅/❌ Service Worker registration
- ✅/❌ Offline app shell caching
- ✅/❌ Mobile Add to Home Screen (Android/iOS)
- ✅/❌ File Handling API (Chrome 102+)

---

## Sign-off

**Tested by:** frontend-engineer  
**Reviewed by:** [frontend-lead / qa-lead]  
**Status:** PASS / PASS_WITH_ISSUES / FAIL  
**Gate-ready:** YES / NO

---

## Next Steps

[If issues found, list next actions]
[If all pass, mark as ready for gate]
```

---

## Execution Checklist

Before starting:
- [ ] Bash tool approved
- [ ] Terminal ready
- [ ] Screenshot tool available
- [ ] Mobile devices ready (if testing mobile)

During execution:
- [ ] Take screenshots at each step
- [ ] Document any unexpected behavior immediately
- [ ] Save console errors/warnings
- [ ] Note exact browser versions tested

After completion:
- [ ] Fill out documentation template
- [ ] Attach all screenshots
- [ ] Report to qa-lead
- [ ] CC frontend-lead and eng-director

---

**Estimated Time:** 30-45 minutes
**Blocker:** Bash tool approval
**Status:** Standing by for approval to execute
