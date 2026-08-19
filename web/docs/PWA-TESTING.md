# PWA Testing Guide for MONOTERMINAL

## Overview

This guide covers testing the Progressive Web App (PWA) features of MONOTERMINAL's web client, ensuring compliance with SRS §2.2 and Phase 1 acceptance criteria (§7.1).

**Key Requirement:** "Web client usable, end to end, from an iPhone/Android browser on the same network"

---

## Prerequisites

### Desktop Testing
- **Chrome 90+** or **Edge 90+** (Chromium-based, best PWA support)
- **Firefox 88+** (limited PWA support, no install prompt)
- **Safari 14+** (macOS only, limited PWA support)

### Mobile Testing
- **Android:** Chrome 90+ on Android 8.0+
- **iOS:** Safari on iOS 14.3+ (iPhone or iPad)

### Network Setup
- Desktop running MONOTERMINAL master daemon on `localhost:5000`
- Mobile device on **same Wi-Fi network** as desktop
- Firewall allows inbound connections on port 5000 (Windows Defender Firewall)

---

## Desktop PWA Testing

### 1. Installation Testing

#### Chrome/Edge (Best Support)
1. Navigate to `http://localhost:3000` (dev) or deployed URL
2. After **2 visits** and **5 minutes total engagement**, install prompt should appear
3. Alternative: Click install icon in address bar (⊕ icon)
4. Click "Install" → PWA opens in standalone window

**Expected Behavior:**
- ✅ Window Controls Overlay: Native window chrome with custom title bar
- ✅ File Handling API: Can open `.txt`, `.log` files (Chrome 102+)
- ✅ App appears in Start Menu/Applications (searchable)
- ✅ Launches without browser UI (no address bar, tabs)

#### Manual Install Test (Skip Engagement Tracking)
```javascript
// In DevTools Console:
localStorage.setItem('monoterminal-visit-count', '2');
localStorage.setItem('monoterminal-engagement-time', String(5 * 60 * 1000)); // 5 min
location.reload();
```

### 2. Offline Testing

1. Install PWA
2. Open DevTools → Application → Service Workers → Check "Offline"
3. Refresh the app

**Expected Behavior:**
- ✅ App shell loads (UI visible, "Reconnecting..." status)
- ✅ No blank page or error message
- ✅ Terminal shows disconnected state gracefully

### 3. Update Testing

1. Make a code change (e.g., change app title)
2. Build: `npm run build`
3. Deploy updated build
4. Open installed PWA

**Expected Behavior:**
- ✅ Service worker detects new version
- ✅ Update happens automatically (vite-plugin-pwa `autoUpdate` mode)
- ✅ Refresh to see new content

---

## Mobile PWA Testing

### Android Testing (Chrome)

#### Initial Setup
1. On desktop, find local IP: `ipconfig` (Windows) → IPv4 Address (e.g., `192.168.1.100`)
2. Update `web/.env.local`:
   ```env
   VITE_WS_URL=ws://192.168.1.100:5000
   ```
3. Rebuild: `npm run build`
4. Serve: `npm run preview` or deploy

#### Installation
1. On Android, open Chrome
2. Navigate to `http://192.168.1.100:3000` (use desktop's IP)
3. After 2 visits + 5 min engagement, banner appears: "Add MONOTERMINAL to Home screen"
4. Tap "Add" → Icon appears on home screen

**Expected Behavior:**
- ✅ Launches full-screen (no browser UI)
- ✅ Terminal renders correctly (WebGL or Canvas fallback)
- ✅ Mobile keyboard accessory row visible at bottom
- ✅ Touch scrolling works
- ✅ External Bluetooth keyboard works (if connected)

#### Testing Checklist
- [ ] Terminal renders at 30-60 FPS (WebGL addon)
- [ ] Mobile keyboard (Esc, Tab, Ctrl, Alt, arrows) sends correct keys
- [ ] Connection status shows "Connected" when WebSocket is open
- [ ] Orientation change (portrait ↔ landscape) resizes terminal correctly
- [ ] Back button doesn't close app (stays in app context)

### iOS Testing (Safari)

#### Initial Setup
Same as Android: update `VITE_WS_URL` to desktop's local IP.

#### Installation
1. On iPhone/iPad, open Safari
2. Navigate to `http://192.168.1.100:3000`
3. Tap Share button (square with arrow) → "Add to Home Screen"
4. Tap "Add" → Icon appears on home screen

**Expected Behavior:**
- ✅ Launches full-screen
- ✅ Terminal renders (WebGL or Canvas fallback)
- ✅ Mobile keyboard accessory row visible
- ✅ Status bar shows app name ("MONOTERMINAL")

#### **iOS Safari Backgrounding Limitation (KNOWN TRADE-OFF per SRS §2.2, §9.3)**

**Issue:** iOS Safari suspends WebSocket connections after ~30 seconds when the app is backgrounded (home button pressed, switched to another app).

**Mitigation (Automatic):**
- Fast reconnect (<10s per SRS §7.1)
- Scrollback resync from master daemon
- Connection status shows "Reconnecting..." during recovery

**Testing:**
1. Open PWA, connect to master
2. Press Home button (background the app)
3. Wait 30+ seconds
4. Return to app

**Expected Behavior:**
- ✅ Status shows "Reconnecting..." → "Connected" within <10s
- ✅ Terminal scrollback restored
- ✅ Input works immediately after reconnect

**NOT Expected (This is the Accepted Trade-off):**
- ❌ Persistent background connection (iOS doesn't allow it for PWAs)

**Escalation Path:** If users report this is unacceptable, escalate to Capacitor-style native wrapper (Phase 2+). Do NOT pre-build it speculatively.

#### Testing Checklist
- [ ] Install from Safari (Add to Home Screen)
- [ ] Terminal renders correctly
- [ ] Mobile keyboard works
- [ ] Reconnect after backgrounding (<10s)
- [ ] Orientation change works
- [ ] External keyboard works (iPad with Smart Keyboard)

---

## Engagement Tracking Verification

### Test Install Prompt Triggers

1. **First Visit:**
   - Open `http://localhost:3000`
   - Use app for 2 minutes
   - Close tab
   - **Expected:** No install prompt

2. **Second Visit (Insufficient Engagement):**
   - Re-open app
   - Use for 2 minutes (total: 4 min)
   - **Expected:** No install prompt (need 5 min total)

3. **Second Visit (Sufficient Engagement):**
   - Continue using app for 1+ more minute (total: 5+ min)
   - **Expected:** Install prompt appears

### Verify Dismissal Behavior

1. Click "Dismiss" (X button) on install prompt
2. Close and re-open app
3. **Expected:** Prompt does NOT appear again (permanently dismissed)

4. Clear localStorage: `localStorage.clear()`
5. Re-test engagement flow
6. **Expected:** Prompt reappears after meeting thresholds

---

## Performance Validation (Per SRS §7.1)

### Desktop
- **FPS:** 55-60 FPS (WebGL addon)
  - DevTools → Performance → Record terminal interaction
  - Check frame rate stays >55 FPS

- **Latency:** <10ms local (same machine)
  - Type in terminal, observe keystroke → character echo delay
  - Should be imperceptible

### Mobile
- **FPS:** 30-45 FPS (Canvas or WebGL)
  - Acceptable: terminal doesn't need 60 FPS, responsiveness is network-bound
  - Visual smoothness: scrolling, resizing should be fluid

- **Latency:** <30ms LAN (per SRS §5.1.2 p95 target)
  - Type on mobile keyboard
  - Observe delay from tap → character appears in terminal

---

## Troubleshooting

### Install Prompt Doesn't Appear (Desktop)

**Possible Causes:**
- Already installed (check `chrome://apps`)
- Already dismissed permanently (check localStorage)
- Not meeting engagement thresholds (need 2 visits + 5 min)
- Using Firefox (no `beforeinstallprompt` event support)

**Solution:**
- Clear site data: DevTools → Application → Clear storage
- Reload and re-test engagement flow

### iOS: Can't Install from Add to Home Screen

**Possible Causes:**
- Not using Safari (Chrome/Firefox on iOS don't support Add to Home Screen for web apps)
- Site not served over HTTPS (localhost exception only applies to desktop)

**Solution:**
- Use Safari specifically
- If testing over LAN, consider self-signed cert or ngrok tunnel (HTTPS required for non-localhost)

### Android: No Install Banner

**Possible Causes:**
- Chrome version <90
- Not meeting engagement criteria
- manifest.json missing or invalid

**Solution:**
- Check DevTools → Application → Manifest (must show valid manifest)
- Check engagement tracking in localStorage
- Update Chrome to latest version

### Offline Mode Shows Blank Page

**Possible Causes:**
- Service worker not registered
- Workbox cache not populated

**Solution:**
- DevTools → Application → Service Workers (check status)
- Clear cache and reload while online
- Check `vite-plugin-pwa` configuration in `vite.config.ts`

---

## Acceptance Checklist (Per SRS §7.1)

### Phase 1 PWA Acceptance:
- [ ] **Desktop:** PWA installs correctly from Chrome/Edge
- [ ] **Desktop:** Window Controls Overlay displays native window chrome
- [ ] **Desktop:** File Handling API works (open `.txt`, `.log` files)
- [ ] **Desktop:** Offline mode shows app shell (no blank page)
- [ ] **Android:** Installs from Chrome, launches full-screen
- [ ] **Android:** Mobile keyboard accessory row works
- [ ] **iOS:** Installs via Add to Home Screen from Safari
- [ ] **iOS:** Reconnect after backgrounding (<10s)
- [ ] **All Platforms:** 2 visits + 5 min engagement triggers install prompt
- [ ] **All Platforms:** Terminal renders at target FPS (desktop 55-60, mobile 30-45)
- [ ] **Critical:** "Web client usable, end to end, from iPhone/Android browser on same network" ✅

---

## Additional Resources

- [SRS §2.2: Client Application (Web PWA)](../docs/monoterminal-srs.md#22-client-application--web-pwa-the-only-client-d2)
- [SRS §7.1: Phase 1 Acceptance Criteria](../docs/monoterminal-srs.md#71-phase-1--windows--web-months-1-3-d121)
- [SRS §9.3: iOS Safari Backgrounding Risk](../docs/monoterminal-srs.md#93-risk-register)
- [MDN: Progressive Web Apps](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps)
- [Window Controls Overlay API](https://developer.mozilla.org/en-US/docs/Web/API/Window_Controls_Overlay_API)
- [File Handling API](https://developer.chrome.com/articles/file-handling/)

---

## Report Issues

If PWA behavior deviates from this guide, report to `frontend-lead` with:
- Platform (Desktop/Android/iOS)
- Browser + version
- Steps to reproduce
- Expected vs. actual behavior
- Screenshots/video if applicable
