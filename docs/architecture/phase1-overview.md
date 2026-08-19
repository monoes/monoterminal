# Phase 1 Architecture Overview — Windows + Web MVP

**Version:** 1.0  
**Date:** 2026-08-14  
**Status:** ACTIVE  
**Owner:** principal-architect  
**SRS Reference:** monoterminal-srs.md v1.2, §7.1

---

## Executive Summary

This document defines the Phase 1 architecture for MONOTERMINAL: Windows master daemon + Web PWA client.

**Phase 1 Scope (per SRS §7.1):**
- ✅ Windows master daemon (ConPTY, Windows Service, DirectX 12)
- ✅ Master's local terminal UI (egui + wgpu)
- ✅ Web PWA client (desktop + mobile browsers)
- ✅ Direct WebSocket connection (TLS 1.3)
- ✅ Ed25519/JWT authentication
- ✅ In-memory scrollback (10k lines)
- ✅ Single-session support
- ✅ Monomind integration (first-class, embedded)

**NOT in Phase 1:**
- ❌ Linux/macOS master (Phase 3)
- ❌ P2P/WebRTC (Phase 2)
- ❌ Multi-session (Phase 2)
- ❌ SQLite persistence (Phase 2)
- ❌ Compression (Phase 2)

**Acceptance Criteria:**
- 60 FPS master rendering (DirectX 12)
- <10ms local latency
- 70% test coverage
- 24-hour soak test passes
- Monomind dashboard accessible from web client

---

## System Architecture

```
┌───────────────────────────────────────────────────┐
│              Phase 1 System                        │
├───────────────────────────────────────────────────┤
│                                                    │
│  ┌─────────────────┐      WebSocket/TLS 1.3      │
│  │  Master Daemon  │◄────────────────────┐       │
│  │  (Windows)      │                     │       │
│  ├─────────────────┤                     │       │
│  │ • ConPTY Mgr    │                     │       │
│  │ • Session Mux   │                     │       │
│  │ • wgpu Render   │                     │       │
│  │ • WebSocket Srv │                     │       │
│  │ • Auth Layer    │                     │       │
│  │ • Monomind Br   │                     │       │
│  └────┬────────────┘                     │       │
│       │ ConPTY                            │       │
│       ▼                                   │       │
│  ┌─────────────┐                         │       │
│  │  Shell      │                         │       │
│  │  Process    │                  ┌──────▼─────┐ │
│  └─────────────┘                  │ Web Client │ │
│                                   │   (PWA)    │ │
│                                   ├────────────┤ │
│                                   │ • xterm.js │ │
│                                   │ • Auth UI  │ │
│                                   │ • Monomind │ │
│                                   └────────────┘ │
└───────────────────────────────────────────────────┘
```

---

## Component Breakdown

### 1. ConPTY Manager

**Location:** `crates/master/src/pty/conpty.rs`

**Responsibilities:**
- CreatePseudoConsole() wrapper
- ResizePseudoConsole() handler
- Process lifecycle (CreateProcess + STARTUPINFOEX)
- Async I/O pipes (overlapped I/O with tokio)

**Key Operations:**
- `create_conpty(rows, cols, shell) -> Result<ConPtySession>`
- `read_output(&mut self, buf) -> Result<usize>` (async)
- `write_input(&mut self, data) -> Result<()>` (async)
- `resize(&mut self, rows, cols) -> Result<()>`

**Dependencies:**
- `windows` crate (Win32_System_Console, Win32_System_Threading)
- `tokio` for async I/O

---

### 2. Session Manager

**Location:** `crates/master/src/session/mod.rs`

**State Machine:**
```
CREATE → RUNNING → TERMINATED
```

**Data Structures:**
```rust
pub struct Session {
    pub id: SessionId,              // UUID v4
    pub pty: ConPtySession,
    pub shell_pid: u32,
    pub shell_type: String,
    pub dimensions: Dimensions,
    pub working_dir: PathBuf,
    pub scrollback: RingBuffer<Line>, // 10k lines
    pub clients: Vec<ClientId>,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub monomind_detected: bool,
}
```

**Key Operations:**
- `Session::create(config)` - Spawn ConPTY process
- `attach_client(client_id)` - Add client, return scrollback
- `handle_pty_output(data)` - Parse VT, update scrollback, fan-out
- `send_input(data)` - Write to PTY
- `resize(dims)` - Resize terminal
- `terminate()` - Kill process, cleanup

**VT Parser:** Use `vte` crate for ANSI escape sequence parsing

---

### 3. WebSocket Server

**Location:** `crates/master/src/network/websocket.rs`

**Responsibilities:**
- TLS 1.3 listener (rustls)
- WebSocket accept (tokio-tungstenite)
- Protocol Buffer message framing
- Fan-out broadcast (Arc<Bytes> zero-copy)
- Per-client send queue (bounded mpsc channel)

**Message Flow:**
1. TLS handshake
2. WebSocket upgrade
3. Authentication (challenge-response)
4. AttachRequest → AttachResponse (with scrollback)
5. Bidirectional stream (InputData ← → OutputData)

**Backpressure:**
- Per-client 1MB bounded queue
- Drop oldest output if queue full (lossy, acceptable for terminals)

---

### 4. Auth Layer

**Location:** `crates/master/src/auth/mod.rs`

**Components:**
- Ed25519 keypair management
- Challenge-response protocol
- JWT issuer/verifier (EdDSA signing)
- Rate limiter (tower middleware)

**Flow:**
1. Server generates 256-bit random challenge
2. Client signs with Ed25519 private key
3. Server verifies signature against authorized public key
4. Server issues JWT (access: 15min, refresh: 30 days)
5. Client includes JWT in subsequent requests
6. Server verifies JWT signature on each request

**Rate Limits:**
- 100 connections/min per IP
- 5 auth attempts/hour per IP

**Dependencies:**
- `ed25519-dalek` (signing/verification)
- `jsonwebtoken` (JWT encode/decode with EdDSA)
- `tower` (rate limiting)

---

### 5. Local UI (egui + wgpu)

**Location:** `crates/master/src/ui/renderer.rs`

**Rendering Pipeline:**
```
PTY Output → VT Parser → Cell Grid → Glyph Lookup → GPU Render (DX12)
```

**Frame Budget (60 Hz = 16.67ms):**
- PTY read: 2ms
- Dirty tracking: 0.5ms
- Glyph lookup: 1ms
- GPU render: 8ms
- VSync: 5ms
- **Total: 16.5ms** ✅

**Components:**
- Cell grid (terminal state)
- Glyph cache (4096×4096 atlas, DirectX 12 texture)
- Font rendering (HarfBuzz text shaping)
- Input handling (keyboard, mouse, resize)

**Dependencies:**
- `wgpu` (DirectX 12 backend on Windows)
- `egui` (immediate mode UI)
- `harfbuzz_rs` (text shaping)

---

### 6. Monomind Bridge

**Location:** `crates/monomind-bridge/src/lib.rs`

**Responsibilities:**
- Detect `.monomind/` directory (walk upward from cwd)
- Generate install suggestion banner (if not detected)
- Handle dashboard API requests (WebSocket endpoint)
- Health check integration
- Upgrade functionality

**Detection Logic:**
```rust
pub fn detect(working_dir: &Path) -> bool {
    let mut current = working_dir;
    loop {
        if current.join(".monomind").is_dir() {
            return true;
        }
        match current.parent() {
            Some(p) => current = p,
            None => return false,
        }
    }
}
```

**Dashboard API:**
- GetStatus → OrgStatus (org name, running state)
- RunHealthCheck → HealthReport (CLI ok, control server ok, warnings/errors)
- Upgrade → trigger `npx monomind@latest upgrade`

All commands execute via `tokio::process::Command`, parse JSON output.

---

## Cargo Workspace Structure

```
monoterminal/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── master/                # Main daemon
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── pty/
│   │   │   │   └── conpty.rs
│   │   │   ├── session/
│   │   │   │   ├── mod.rs
│   │   │   │   └── scrollback.rs
│   │   │   ├── network/
│   │   │   │   ├── websocket.rs
│   │   │   │   └── tls.rs
│   │   │   ├── auth/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── challenge.rs
│   │   │   │   └── jwt.rs
│   │   │   ├── ui/
│   │   │   │   ├── renderer.rs
│   │   │   │   ├── glyph_cache.rs
│   │   │   │   └── input.rs
│   │   │   └── service/
│   │   │       └── windows.rs
│   │   └── Cargo.toml
│   ├── protocol/              # Shared Protocol Buffers
│   │   ├── proto/
│   │   │   └── envelope.proto
│   │   ├── src/
│   │   │   └── lib.rs
│   │   ├── build.rs
│   │   └── Cargo.toml
│   └── monomind-bridge/       # Monomind integration
│       ├── src/
│       │   └── lib.rs
│       └── Cargo.toml
├── web/                        # React PWA
│   ├── src/
│   │   ├── components/
│   │   │   ├── Terminal.tsx
│   │   │   ├── AuthFlow.tsx
│   │   │   └── MonomindPanel.tsx
│   │   ├── lib/
│   │   │   ├── websocket.ts
│   │   │   ├── protocol.ts
│   │   │   └── auth.ts
│   │   └── main.tsx
│   ├── proto/
│   │   └── envelope.proto      # Same as server
│   └── package.json
└── docs/
    ├── architecture/
    │   └── phase1-overview.md  # This file
    └── decisions/              # ADRs
```

---

## Protocol Layer

**Crate:** `monoterminal-protocol`

**Schema:** Protocol Buffers (proto3), fully defined in SRS §3.1.1

**Key Messages:**
- `Envelope` (wrapper with sequence_number)
- `AttachRequest` / `AttachResponse`
- `InputData` / `OutputData`
- `ResizeRequest`
- `ErrorResponse`
- `DashboardRequest` / `DashboardResponse`

**Build:** `prost-build` compiles `.proto` files at build time

**Usage:**
```rust
use monoterminal_protocol::*;

let envelope = Envelope {
    sequence_number: 1,
    message: Some(envelope::Message::AttachRequest(req)),
};
let bytes = envelope.encode_to_vec();
```

---

## Security Architecture

**Module:** `crates/master/src/auth/`

**Components:**

1. **Ed25519 Challenge-Response:**
   - Server generates 256-bit nonce
   - Client signs with private key
   - Server verifies signature against authorized public keys

2. **JWT Issuance:**
   - Access: 15 minutes TTL, EdDSA signing
   - Refresh: 30 days TTL, rotated on use
   - Claims: sub (user ID), iss, exp, iat, scope

3. **TLS 1.3 Configuration:**
   - rustls library (no OpenSSL)
   - Cipher suites: AES-256-GCM, AES-128-GCM, ChaCha20-Poly1305
   - Self-signed cert for localhost (TOFU model)

4. **RBAC Foundation:**
   - Roles: admin, user, read-only
   - Permissions: session:attach, session:create, input:write, etc.
   - Phase 1: basic owner check only

5. **Rate Limiting:**
   - Tower middleware, token bucket algorithm
   - Limits: 100 conn/min, 5 auth/hour, 20 session-creates/min

---

## Web Client Architecture

**Stack:**
- React 18 + TypeScript
- Vite (build tool)
- xterm.js 5.x + WebGL addon
- TanStack Query (server state)
- Zustand (client state)

**Key Components:**

1. **Terminal Component:**
   - xterm.js wrapper
   - WebGL addon for performance
   - Fit addon for responsive sizing
   - Keyboard input forwarding

2. **WebSocket Client:**
   - Protocol Buffer encode/decode (protobufjs)
   - Connection state management
   - Auto-reconnect logic

3. **Auth Flow:**
   - Ed25519 keypair generation/loading (tweetnacl)
   - Challenge signing
   - JWT storage (localStorage)

4. **Monomind Panel:**
   - Dashboard status display
   - Health check trigger
   - Upgrade button
   - Polls via WebSocket every 5s

---

## Testing Strategy

**Target:** 70% coverage (Phase 1)

**Breakdown:**
- Unit tests (60%): Individual components
- Integration tests (30%): Client-server flows
- E2E tests (10%): Full workflow

**Tools:**
- `cargo test` (unit + integration)
- `pytest` (E2E, Python client)
- `cargo-tarpaulin` (coverage)

**Critical Test Cases:**
1. ConPTY creation/resize/termination
2. Ed25519 challenge-response auth
3. WebSocket attach + output streaming
4. VT parsing correctness (snapshot tests)
5. 24-hour soak test (no crashes, no memory leaks)

---

## Deployment

**Windows Service:**
- `sc.exe create` for service registration
- `SERVICE_AUTO_START` or manual start (decision pending)
- Firewall rule for WebSocket port (5000 default)

**Configuration:**
- TOML file: `~/.config/monoterminal/config.toml`
- Ed25519 keys: `~/.ssh/monoterminal_ed25519{,.pub}`
- TLS cert: auto-generated self-signed for localhost

---

## Open Decisions

1. **Windows Service auto-start:** AUTO or MANUAL? (Recommend: MANUAL for Phase 1)
2. **Default shell:** PowerShell 7+, PowerShell 5, or cmd.exe? (Recommend: PowerShell 7+ if installed, else cmd.exe)
3. **Logging:** File, Windows Event Log, or stdout? (Recommend: file for Phase 1)
4. **TLS cert:** Auto-generate self-signed or require user setup? (Recommend: auto-generate)

---

## Timeline

**Sprint 0 (Weeks 1-6):**

Week 1: Workspace + Protocol + ConPTY wrapper  
Week 2: Session manager + WebSocket server (no TLS)  
Week 3: Auth layer + TLS + Monomind bridge  
Week 4: wgpu renderer + egui UI  
Week 5-6: Web client + E2E testing + documentation  

**Estimated Completion:** 6 weeks for Phase 1 MVP

---

**END OF DOCUMENT**
