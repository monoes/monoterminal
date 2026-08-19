# Web Client Deployment Strategy

**Version:** 1.0  
**Date:** August 15, 2026  
**Phase:** Phase 1 (Windows + Web MVP)  
**Decision:** Master daemon serves web client locally at `http://localhost:8080`

---

## Overview

The MONOTERMINAL web client (React PWA) is served **locally by the master daemon** rather than hosted on a separate domain. This provides the simplest installation and user experience for Phase 1.

---

## Architecture Decision

### Decision: Bundled Local Hosting

**Model:** Master daemon serves static web client files from installation directory

**URL:** `http://localhost:8080` (or configurable port)

**Rationale:**
1. **Simplest UX:** Install MSI → daemon runs → open localhost:8080
2. **No external dependencies:** No separate hosting, domain, or TLS certificate needed
3. **Offline-first:** Works without internet connection (LAN-only Phase 1 scope)
4. **Security:** Web client and WebSocket server on same machine, no CORS issues
5. **Aligns with Phase 1:** Local-only, no P2P/internet discovery yet

**Rejected Alternatives:**
- ❌ Separate hosting (GitHub Pages, Netlify): Adds complexity, requires CORS, separate deployment
- ❌ Tauri/Electron wrapper: Out of scope per SRS §1.2 (PWA-only decision)
- ❌ Remote hosting (monoterminal.app): Phase 2+ (requires P2P/internet discovery)

---

## Implementation

### 1. Web Client Build

**Build Command:**
```bash
cd web
npm run build
```

**Output:** `web/dist/` directory containing:
```
web/dist/
├── index.html
├── assets/
│   ├── index-<hash>.js       (React app bundle)
│   ├── index-<hash>.css      (Styles)
│   └── vendor-<hash>.js      (Dependencies)
├── pwa-192x192.png           (PWA icons)
├── pwa-512x512.png
├── manifest.webmanifest      (PWA manifest)
├── sw.js                     (Service worker)
└── favicon.ico
```

**Size:** ~500-800 KB (gzipped)

### 2. Daemon HTTP Server

**HTTP Server Configuration (`crates/master/src/server/http.rs`):**

```rust
use axum::{
    Router,
    routing::get,
    response::{Html, IntoResponse},
};
use tower_http::services::ServeDir;

pub async fn start_http_server(port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        // Serve static files from installation directory
        .nest_service("/", ServeDir::new(get_web_client_path()))
        // Health check endpoint
        .route("/health", get(health_check))
        // Version endpoint
        .route("/api/version", get(get_version));

    let addr = format!("127.0.0.1:{}", port);
    tracing::info!("HTTP server listening on http://{}", addr);

    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

fn get_web_client_path() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // Development: Serve from web/dist/
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("web/dist")
    }
    
    #[cfg(not(debug_assertions))]
    {
        // Production: Serve from installation directory
        // C:\Program Files\MONOTERMINAL\web\
        std::env::current_exe()
            .unwrap()
            .parent().unwrap()
            .join("web")
    }
}

async fn health_check() -> impl IntoResponse {
    Html("OK")
}

async fn get_version() -> impl IntoResponse {
    Html(env!("CARGO_PKG_VERSION"))
}
```

**Dependencies (`crates/master/Cargo.toml`):**
```toml
[dependencies]
axum = "0.7"
tower-http = { version = "0.5", features = ["fs"] }
```

### 3. Port Configuration

**Default Port:** 8080 (HTTP) + 5000 (WebSocket)

**Configuration File (`~/.monoterminal/config.toml`):**
```toml
[server]
http_port = 8080         # Web client HTTP server
websocket_port = 5000    # WebSocket protocol server

[server.bind]
address = "127.0.0.1"    # Localhost only (Phase 1)
```

**User Override:**
```toml
# Custom ports (if 8080 conflicts)
[server]
http_port = 3000
websocket_port = 5001
```

### 4. Startup Sequence

**Master Daemon Startup:**
1. Load configuration (`~/.monoterminal/config.toml`)
2. Start WebSocket server on port 5000 (protocol server)
3. Start HTTP server on port 8080 (web client files)
4. Log startup message:
   ```
   MONOTERMINAL master daemon started
   WebSocket server: ws://127.0.0.1:5000
   Web client: http://127.0.0.1:8080
   
   Open http://127.0.0.1:8080 in your browser to connect.
   ```

**Web Client Connection:**
1. User opens `http://localhost:8080` in browser
2. Browser loads React PWA (service worker installs)
3. PWA connects to `ws://localhost:5000` (WebSocket)
4. Authentication flow (Ed25519 key + JWT)
5. Session list displayed

---

## MSI Installer Integration

### Installation Layout

**Target Directory:** `C:\Program Files\MONOTERMINAL\`

```
C:\Program Files\MONOTERMINAL\
├── monoterminal.exe          (Main binary)
├── web\                      (Web client static files - bundled)
│   ├── index.html
│   ├── assets\
│   │   ├── index-<hash>.js
│   │   ├── index-<hash>.css
│   │   └── vendor-<hash>.js
│   ├── pwa-192x192.png
│   ├── pwa-512x512.png
│   ├── manifest.webmanifest
│   ├── sw.js
│   └── favicon.ico
├── LICENSE.txt
└── README.txt
```

**MSI Packaging (`installer/monoterminal.wxs`):**
```xml
<Component Id="WebClientFiles" Guid="*">
  <File Id="WebIndexHtml" Source="$(var.WebDistDir)\index.html" KeyPath="yes" />
  <File Id="WebManifest" Source="$(var.WebDistDir)\manifest.webmanifest" />
  <File Id="WebServiceWorker" Source="$(var.WebDistDir)\sw.js" />
  
  <!-- Assets directory (all files) -->
  <Component Id="WebAssets" Directory="WEBASSETSDIR">
    <File Id="WebAssetIndexJs" Source="$(var.WebDistDir)\assets\index-*.js" />
    <File Id="WebAssetIndexCss" Source="$(var.WebDistDir)\assets\index-*.css" />
    <File Id="WebAssetVendorJs" Source="$(var.WebDistDir)\assets\vendor-*.js" />
  </Component>
</Component>
```

**Build Step (CI):**
```yaml
# .github/workflows/release.yml
- name: Build web client
  run: |
    cd web
    npm ci
    npm run build

- name: Package MSI
  run: |
    # Copy web/dist/ to installer staging
    xcopy /E /I web\dist installer\web
    
    # Build MSI with WiX
    candle installer\monoterminal.wxs -dWebDistDir=installer\web
    light -out monoterminal-x.y.z.msi monoterminal.wixobj
```

---

## User Experience

### Installation

1. **Download MSI:** `monoterminal-1.0.0-x86_64.msi` from GitHub Releases or winget
2. **Run installer:** Double-click MSI (requires admin)
3. **Installation wizard:**
   - Accept license
   - Choose installation directory (default: `C:\Program Files\MONOTERMINAL\`)
   - Install
4. **Service auto-starts:** Windows Service "MonoTerminal" starts automatically
5. **Open web client:** User opens `http://localhost:8080` in browser

### First-Time Setup

**Browser Tab:** `http://localhost:8080`

1. **Landing Page:**
   ```
   Welcome to MONOTERMINAL
   
   [Connect to Local Master]
   
   Connection: ws://localhost:5000
   Status: Connected ✅
   
   No sessions yet. Create your first session below.
   
   [+ New Session]
   ```

2. **Authentication:**
   - Ed25519 key generated on first launch (stored in `~/.monoterminal/identity.key`)
   - JWT issued by master daemon
   - Token stored in browser localStorage

3. **Session Creation:**
   - Click "+ New Session"
   - Default shell: PowerShell (Windows)
   - Session starts, terminal rendered via xterm.js

### Daily Usage

**User Workflow:**
1. Service runs in background (no manual start needed)
2. Open browser → `http://localhost:8080`
3. PWA shows session list (reconnects to existing sessions)
4. Click session to attach
5. Close browser → sessions persist (detached but running)

**Offline Support (PWA):**
- Service worker caches static files
- App loads even if HTTP server temporarily unavailable
- WebSocket reconnection with exponential backoff

---

## Development vs. Production

### Development Mode

**Vite Dev Server:**
```bash
cd web
npm run dev
```

**URLs:**
- Web client: `http://localhost:3000` (Vite dev server)
- WebSocket proxy: `ws://localhost:5000` (proxied via Vite config)

**Hot Module Replacement:** Changes auto-reload

### Production Mode

**Static Build:**
```bash
cd web
npm run build       # → web/dist/
```

**Daemon serves:** `http://localhost:8080` → `web/dist/` files

**No Vite dependency** in production (pure static files)

---

## Security Considerations

### Localhost Binding

**HTTP Server:** Binds to `127.0.0.1` ONLY (not `0.0.0.0`)

**Reason:**
- Prevents external network access to web client
- Phase 1 is local-only (no internet discovery)
- Phase 2+ (P2P) may allow LAN access (configurable)

**Configuration:**
```toml
[server.bind]
address = "127.0.0.1"   # Phase 1: localhost only
# address = "0.0.0.0"   # Phase 2+: LAN access (opt-in)
```

### CORS (Cross-Origin Resource Sharing)

**Not Required:** Same-origin (http://localhost:8080 → ws://localhost:5000)

**WebSocket Upgrade:** HTTP → WS on same origin, no CORS headers needed

### TLS (HTTPS)

**Phase 1:** HTTP only (`http://localhost:8080`)

**Reason:**
- Localhost traffic not exposed to network
- Self-signed TLS adds complexity (browser warnings)
- WebSocket uses separate TLS 1.3 connection

**Phase 2+:** Optional HTTPS with self-signed cert (if user enables LAN access)

---

## PWA Features

### Offline Support

**Service Worker (`web/public/sw.js`):**
- Caches static assets (HTML, JS, CSS, icons)
- Network-first for WebSocket (live connection)
- Cache-first for static files (offline viewing)

**Manifest (`web/public/manifest.webmanifest`):**
- Installable on desktop (Chrome/Edge)
- Standalone window mode (no browser chrome)
- Custom icons and theme color

**App Installation:**
1. Open `http://localhost:8080` in Chrome/Edge
2. Address bar shows install icon (+)
3. Click "Install MONOTERMINAL"
4. PWA opens in standalone window (no URL bar)

### Mobile Browser Support

**iOS Safari:**
- Add to Home Screen
- Standalone mode (full screen)
- Service worker support (iOS 11.3+)

**Android Chrome:**
- Add to Home Screen
- WebAPK installation (native-like app)
- Full PWA support

**URL:** Same `http://localhost:8080` (if master runs on same device) OR `http://<LAN-IP>:8080` (Phase 2+)

---

## Monitoring & Debugging

### HTTP Server Logs

**Tracing:**
```rust
tracing::info!(port = %port, "HTTP server started");
tracing::debug!(path = %req.uri().path(), "Serving static file");
tracing::error!(error = %e, "HTTP server failed");
```

**Log Output:**
```
2026-08-15T10:00:00Z INFO monoterminal::server::http: HTTP server started port=8080
2026-08-15T10:00:05Z DEBUG monoterminal::server::http: Serving static file path=/index.html
2026-08-15T10:00:05Z DEBUG monoterminal::server::http: Serving static file path=/assets/index-a3f2b.js
```

### Health Check Endpoint

**URL:** `http://localhost:8080/health`

**Response:**
```
HTTP/1.1 200 OK
Content-Type: text/html

OK
```

**Use Case:** Monitoring, CI tests, automated checks

### Version Endpoint

**URL:** `http://localhost:8080/api/version`

**Response:**
```
HTTP/1.1 200 OK
Content-Type: text/html

1.0.0
```

---

## Troubleshooting

### Web Client Not Loading

**Symptom:** Browser shows "Connection refused" at `http://localhost:8080`

**Diagnostics:**
1. Check daemon running:
   ```cmd
   sc query MonoTerminal
   ```
   Expected: `STATE: 4 RUNNING`

2. Check port 8080 listening:
   ```cmd
   netstat -ano | findstr :8080
   ```
   Expected: `TCP    127.0.0.1:8080    0.0.0.0:0    LISTENING    <PID>`

3. Check logs:
   ```cmd
   Get-Content "$env:USERPROFILE\.monoterminal\logs\master.log" -Tail 50
   ```
   Look for: `HTTP server started port=8080`

**Solutions:**
- Restart service: `sc stop MonoTerminal && sc start MonoTerminal`
- Check port conflict: Change `http_port` in config.toml
- Verify web files exist: `dir "C:\Program Files\MONOTERMINAL\web\"`

### PWA Not Installing

**Symptom:** No install icon (+) in browser address bar

**Causes:**
- Browser not Chrome/Edge (Firefox doesn't support desktop PWA install)
- Manifest.json errors (check DevTools → Application → Manifest)
- Service worker registration failed (check DevTools → Application → Service Workers)

**Solutions:**
- Use Chrome or Edge (Chromium-based)
- Check manifest validity: DevTools → Application → Manifest
- Check service worker: DevTools → Application → Service Workers → "Update on reload"

### WebSocket Connection Failed

**Symptom:** Web client loads but shows "Disconnected" status

**Diagnostics:**
1. Check WebSocket server running:
   ```cmd
   netstat -ano | findstr :5000
   ```
   Expected: `TCP    127.0.0.1:5000    0.0.0.0:0    LISTENING`

2. Check browser DevTools → Console:
   ```
   WebSocket connection to 'ws://localhost:5000' failed: Connection refused
   ```

**Solutions:**
- Verify WebSocket port in config.toml matches web client config
- Restart daemon
- Check firewall rules (Windows Firewall may block local WebSocket)

---

## Future Enhancements (Phase 2+)

- [ ] **LAN Access:** Allow binding to `0.0.0.0` for LAN clients (opt-in)
- [ ] **HTTPS Support:** Self-signed TLS cert for web client (browser trust required)
- [ ] **mDNS Discovery:** `monoterminal.local` instead of IP address
- [ ] **Remote Hosting:** Optional separate hosting for web client (P2P era)
- [ ] **Custom Domain:** User can host web client on own domain (advanced)

---

## Summary

**Phase 1 Deployment Model:**
- ✅ Daemon serves web client locally at `http://localhost:8080`
- ✅ Web client bundled in MSI installer (`C:\Program Files\MONOTERMINAL\web\`)
- ✅ PWA installable as standalone app
- ✅ Offline support via service worker
- ✅ No external hosting or domain needed

**User Experience:**
1. Install MSI
2. Service auto-starts
3. Open `http://localhost:8080` in browser
4. Install PWA (optional)
5. Create session and start using

**Status:** ✅ Strategy documented, ready for implementation in MSI packaging (task-4)

---

**Questions or clarifications? Ping devops-lead.**
