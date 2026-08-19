# MONOTERMINAL Web Client

Progressive Web App (PWA) client for MONOTERMINAL terminal emulator.

## Overview

Single React + xterm.js codebase serving **all remote devices** (desktop browsers, Android, iOS) per SRS §2.2. No native apps, no Tauri wrapper.

**Technology Stack:**
- React 18 + TypeScript
- Vite (build tool + dev server)
- xterm.js 6.0 + WebGL addon (55-60 FPS desktop, 30-45 FPS mobile)
- WebSocket + Protocol Buffers (Phase 1)
- PWA: vite-plugin-pwa + Workbox

---

## Quick Start

### Development Server
```bash
npm install
npm run dev
```
Open `http://localhost:3000` - connects to master daemon at `ws://localhost:5000`

### Production Build
```bash
npm run build
npm run preview  # Test production build locally
```

---

## PWA Features (Task-13 / SRS §2.2)

### Installability
- **Desktop:** Window Controls Overlay (native window chrome)
- **Desktop:** File Handling API (open `.txt`, `.log` files)
- **Mobile:** Add to Home Screen (Android + iOS)
- **Install Trigger:** 2 visits + 5 min engagement (auto-tracked)

### Offline Support
- App shell cached (UI loads offline)
- Service worker: auto-update mode (Workbox)
- Connection status: graceful "Reconnecting..." state

### iOS Safari Limitation (Known Trade-off)
Per SRS §2.2, §9.3: iOS Safari suspends WebSocket connections after ~30s when app is backgrounded.

**Mitigation:** Fast reconnect (<10s) + scrollback resync (automatic)

**Escalation:** If user complaints justify it, escalate to Capacitor-style native wrapper (Phase 2+). Do NOT pre-build speculatively.

---

## Scripts

| Script | Description |
|--------|-------------|
| `npm run dev` | Start dev server (HMR, port 3000) |
| `npm run build` | Production build (type-check + bundle) |
| `npm run preview` | Serve production build locally |
| `npm run lint` | Run ESLint |
| `npm run type-check` | TypeScript type checking (no emit) |
| `npm run generate-icons` | Generate placeholder PWA icons (SVG) |
| `npm run pwa:check` | Reminder to check PWA in DevTools |

---

## PWA Icon Setup

### Phase 1 (Development)
```bash
npm run generate-icons
```
Generates placeholder SVG icons:
- `public/pwa-192x192.svg`
- `public/pwa-512x512.svg`
- `public/apple-touch-icon.svg`
- `public/favicon.svg`

### Production (Replace with Branding)
1. Design actual MONOTERMINAL logo/icon
2. Export as PNG at required sizes:
   - `pwa-192x192.png`
   - `pwa-512x512.png`
   - `apple-touch-icon.png` (180x180)
3. Generate `favicon.ico` (16x16, 32x32, 48x48) using [favicon.io](https://favicon.io)
4. Replace SVG placeholders in `public/`

---

## Testing

### Desktop Testing (Chrome/Edge)
1. `npm run dev`
2. Open DevTools → Application tab
3. Check:
   - Manifest (valid, icons present)
   - Service Workers (registered, activated)
   - Install button in address bar (⊕ icon)

### Mobile Testing (iOS/Android)
See **[docs/PWA-TESTING.md](./docs/PWA-TESTING.md)** for comprehensive mobile testing guide.

**Quick Test:**
1. Find desktop IP: `ipconfig` (Windows) → IPv4 (e.g., `192.168.1.100`)
2. Create `.env.local`:
   ```env
   VITE_WS_URL=ws://192.168.1.100:5000
   ```
3. `npm run build && npm run preview`
4. On mobile (same Wi-Fi): Open `http://192.168.1.100:4173` (preview port)
5. **Android:** Install from Chrome banner
6. **iOS:** Safari → Share → "Add to Home Screen"

---

## Project Structure

```
web/
├── public/               # Static assets
│   ├── favicon.svg       # Generated placeholder favicon
│   ├── pwa-*.svg         # Generated placeholder PWA icons
│   └── icons.svg         # App icons/assets
├── src/
│   ├── components/
│   │   ├── Terminal.tsx          # xterm.js integration (task-10)
│   │   ├── MobileKeyboard.tsx    # Touch keyboard accessory (task-11)
│   │   ├── MonomindPanel.tsx     # Embedded dashboard (task-12)
│   │   ├── ConnectionStatus.tsx  # WebSocket state display (task-11)
│   │   └── InstallPrompt.tsx     # PWA install prompt (task-13)
│   ├── lib/
│   │   └── websocket-client.ts   # WebSocket + Protocol Buffers (task-9)
│   ├── App.tsx           # Main app component
│   ├── main.tsx          # React entry point
│   └── *.css             # Component styles
├── scripts/
│   └── generate-pwa-icons.js  # PWA icon generator (task-13)
├── docs/
│   └── PWA-TESTING.md    # Comprehensive PWA testing guide (task-13)
├── vite.config.ts        # Vite + PWA plugin config
└── package.json
```

---

## Phase 1 Acceptance (SRS §7.1)

**Critical:** "Web client usable, end to end, from iPhone/Android browser on same network"

### Checklist
- [x] React + TypeScript + Vite setup
- [x] xterm.js 6.0 + WebGL addon (task-10)
- [x] WebSocket client with Protocol Buffers (task-9)
- [x] Mobile keyboard accessory row (task-11)
- [x] Connection status display (task-11)
- [x] Monomind panel stub (task-12 placeholder)
- [x] PWA manifest + service worker (task-13)
- [x] Install prompt (2 visits + 5 min) (task-13)
- [x] Offline app shell (task-13)
- [x] Window Controls Overlay (task-13)
- [x] File Handling API (task-13)
- [ ] End-to-end testing (task-14 - test-engineer-e2e)

---

## Dependencies

### Runtime
- `react`, `react-dom` - UI framework
- `@xterm/xterm` - Terminal emulator core
- `@xterm/addon-webgl` - GPU-accelerated rendering
- `@xterm/addon-fit` - Responsive terminal sizing
- `@xterm/addon-web-links` - Clickable URLs
- `protobufjs` - Protocol Buffer encoding/decoding

### Build/Dev
- `vite` - Build tool + dev server
- `@vitejs/plugin-react` - React Fast Refresh
- `vite-plugin-pwa` - PWA manifest + service worker generation
- `workbox-window` - Service worker runtime
- `typescript` - Type system
- `eslint` - Linter

---

## Environment Variables

Create `.env.local` for local overrides:

```env
# WebSocket URL (master daemon)
VITE_WS_URL=ws://localhost:5000

# For mobile testing over LAN:
# VITE_WS_URL=ws://192.168.1.100:5000
```

---

## Browser Support

| Browser | Version | Install | Offline | Notes |
|---------|---------|---------|---------|-------|
| Chrome | 90+ | ✅ | ✅ | Best PWA support |
| Edge | 90+ | ✅ | ✅ | Chromium-based |
| Firefox | 88+ | ❌ | ✅ | No install prompt |
| Safari | 14+ | ⚠️ | ✅ | iOS only; limited PWA features |
| Chrome Android | 90+ | ✅ | ✅ | Full support |
| Safari iOS | 14.3+ | ⚠️ | ✅ | Add to Home Screen only; backgrounding limitation |

---

## Known Limitations (Accepted Trade-offs)

1. **Browser Shortcuts:** Cannot intercept Ctrl+T/W/N, F11 (browser-reserved)
2. **iOS Backgrounding:** WebSocket disconnects after ~30s when app is backgrounded
   - Mitigation: Fast reconnect + scrollback resync (automatic)
   - Escalation: Capacitor wrapper (if user complaints justify it)
3. **Clipboard:** Requires user gesture (standard async Clipboard API)
4. **No PTY, No Raw Sockets:** Master hosts the process; client is display-only

---

## Troubleshooting

### Install Prompt Doesn't Appear
- Check localStorage: `monoterminal-visit-count` (need 2+)
- Check localStorage: `monoterminal-engagement-time` (need 300000+ ms = 5 min)
- Check localStorage: `monoterminal-install-dismissed` (should be null)
- Clear site data: DevTools → Application → Clear storage

### Service Worker Not Registering
- Must use HTTPS (or localhost exception)
- Check DevTools → Application → Service Workers
- Check console for registration errors
- Run `npm run build` (service worker is production-only with current config)

### Mobile: WebSocket Connection Fails
- Firewall: Allow inbound on port 5000 (Windows Defender Firewall)
- Network: Mobile and desktop must be on same Wi-Fi
- URL: Use desktop's local IP (not `localhost`)
- CORS: Master daemon must allow WebSocket origin (check Rust backend config)

---

## Contributing

See main project [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## License

See main project [LICENSE](../LICENSE)

---

## Related Documentation

- [SRS §2.2: Client Application](../docs/monoterminal-srs.md)
- [PWA Testing Guide](./docs/PWA-TESTING.md)
- [Phase 1 Acceptance Criteria](../docs/monoterminal-srs.md#71-phase-1)
