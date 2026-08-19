# ADR-002: PWA-Only Client (No Native Mobile Apps)

**Status:** Accepted  
**Date:** 2026-08-14  
**Deciders:** product-owner, principal-architect  
**SRS Reference:** §8.1.4

---

## Context

MONOTERMINAL needs client applications for:
- Desktop browsers (Chrome, Firefox, Safari)
- Mobile browsers (Android Chrome, iOS Safari)
- Desktop native (Windows/macOS/Linux)

We evaluated native mobile apps (Android/iOS) vs. a single Progressive Web App (PWA).

---

## Decision

**Build ONE client: a React + xterm.js Progressive Web App (PWA)** that serves all devices.

- Desktop browsers install it via Window Controls Overlay (Chrome 90+)
- Mobile browsers install it via "Add to Home Screen" (standard PWA install)
- **NO native Android app**
- **NO native iOS app**
- **NO Tauri desktop wrapper**

---

## Alternatives Considered

### Option A: Native Mobile Apps (Android + iOS)

**Pros:**
- Full OS integration (background services, notifications)
- Native keyboard control
- App Store presence

**Cons:**
- 3 client codebases (Web + Kotlin + Swift)
- Android Play Store risk (Termux rejection precedent)
- iOS App Store audio-mode justification risk
- WebRTC is built into mobile browsers anyway (no native SDK needed)
- 3× the CI/CD + code-signing cost

**Verdict:** ❌ Rejected

---

### Option B: React Native

**Pros:**
- Share code between mobile platforms
- One codebase for Android + iOS

**Cons:**
- Still 2 codebases (Web + React Native)
- 30-40 FPS (JS bridge overhead) vs 58-60 FPS target
- PTY integration requires native modules anyway
- Battery: 250-320 mAh/hour vs 180-220 mAh/hour native

**Verdict:** ❌ Rejected (superseded by PWA-only decision)

---

### Option C: Tauri Desktop Wrapper

**Pros:**
- Native-looking window on desktop
- Smaller binary than Electron

**Cons:**
- Another codebase to maintain (Rust + WebView bridge)
- Code signing costs (Apple Developer $99/yr, Windows EV $200-400/yr)
- Window Controls Overlay (Chrome 90+) already gives native-like windows
- File Handling API (Chrome 102+) already gives file associations

**Verdict:** ❌ Rejected (PWA installability removes need)

---

### Option D: PWA-Only ✅

**Pros:**
- **ONE codebase** (React + xterm.js) serves all devices
- WebRTC `RTCPeerConnection` built into all modern browsers (no SDK to ship)
- No Android Play Store rejection risk (no app to submit)
- No iOS App Store audio-mode justification risk
- No code signing costs for clients (only master daemon)
- Window Controls Overlay + File Handling API = native-like desktop experience
- Offline support via Service Worker

**Cons:**
- iOS Safari background suspension (WebRTC pauses when backgrounded)

**Trade-off Accepted:**
- iOS Safari suspends WebRTC when backgrounded or screen locks
- Web apps CANNOT claim `AVAudioSession` background-audio privilege (native-only)
- **Mitigation:** Fast reconnect (<10s) + late-joiner scrollback resync (already required by architecture)
- Backgrounded mobile session reconnects like a network blip would
- **Fallback (if unacceptable):** Thin Capacitor-style WebView wrapper around SAME web UI to gain background service (not a return to full native rendering)

**Verdict:** ✅ **CHOSEN**

---

## Consequences

### Positive

- 1 client codebase instead of 4 (Web + Android + iOS + Tauri)
- 0 app store submissions to manage
- 0 native client code-signing costs
- WebRTC available in every browser (no SDK integration)
- Faster development (no Kotlin, Swift, or Tauri to learn)

### Negative

- iOS Safari backgrounding limitation (accepted, mitigated)
- No native OS integration (notifications, background services)
- Requires network connection (offline mode limited to app shell)

### Neutral

- Mobile keyboard: On-screen accessory row + native keyboard for external/Bluetooth
- Clipboard: Async Clipboard API (requires user gesture, standard browser behavior)
- No PTY on client (master hosts the process, same as any architecture)

---

## Accepted Limitations

1. **iOS Safari Backgrounding:**
   - Safari suspends WebRTC and Web Audio when app backgrounds or screen locks
   - Web app cannot obtain native `AVAudioSession` background-audio privilege
   - **User impact:** Session disconnects when phone locks, reconnects when unlocked
   - **Acceptance criteria:** Reconnect in <10s, scrollback resync works reliably (Phase 2 validation)

2. **No Native Notifications:**
   - Web Push API available, but requires user permission
   - No guaranteed delivery (OS can defer/drop)
   - **Phase 1:** No notifications. **Phase 2+:** Web Push if needed.

3. **Keyboard Shortcuts:**
   - Cannot intercept reserved browser shortcuts (Ctrl+T/W/N, F11)
   - **Mitigation:** Document limitations, use alternative shortcuts

---

## References

- SRS §8.1.4 (PWA-Only Client — No Native Mobile Apps)
- SRS §2.2 (Client Application — Web (PWA), the only client)
- Apple Developer Forums 2026: iOS WebRTC backgrounding behavior
- Window Controls Overlay: https://developer.chrome.com/docs/web-platform/window-controls-overlay
- File Handling API: https://developer.chrome.com/docs/capabilities/web-apis/file-handling

---

## Follow-up Actions

1. ✅ Remove Android/iOS client from roadmap
2. ✅ Remove Tauri desktop client from roadmap
3. ⏳ Implement PWA manifest.json + Service Worker
4. ⏳ Test iOS Safari backgrounding behavior (Phase 2)
5. ⏳ Document keyboard shortcut limitations
