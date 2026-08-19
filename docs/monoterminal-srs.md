# MONOTERMINAL Software Requirements Specification

**Version:** 1.2  
**Date:** August 14, 2026  
**Status:** Implementation-Ready  
**Knowledge Matrix Source:** 95% Complete (46/46 nodes, 12/12 domains)

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-08-14 | exhaustive-srs Org | Initial comprehensive SRS from Knowledge Matrix synthesis |
| 1.1 | 2026-08-14 | Product decision | **Native Android/iOS apps removed** — web (PWA) is now the only client, serving desktop and mobile browsers alike. Rollout re-sequenced to **macOS master + Web client first**, then Linux/Windows expansion. Monomind integration (per-session detection, embedded dashboard, health check & upgrade) promoted to a first-class, Phase-1 requirement rather than an ambient feature. |
| 1.2 | 2026-08-14 | Product decision | **Platform order flipped: Windows ships first**, not macOS — this SRS is going to a Windows machine for the initial build. Phase 1 = Windows master + Web client. Phase 3 (platform expansion) now covers Linux + macOS instead of Linux + Windows. Called out explicitly: Windows has no launchd/systemd-style socket activation, so Phase 1 accepts an always-running service instead of activation-on-demand (§7.1). |

**Audience:**
- Engineering team (implementation specifications)
- Product stakeholders (business context and roadmap)
- Security reviewers (threat model and controls)
- QA team (acceptance criteria and test requirements)

**Traceability:** Every requirement in this document traces back to specific Knowledge Matrix nodes (referenced as `[Dx.y]` where x = domain, y = node).

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [System Architecture](#2-system-architecture)
3. [Protocol & Security](#3-protocol--security)
4. [Data & Platform](#4-data--platform)
5. [Performance & UX](#5-performance--ux)
6. [Development & Testing](#6-development--testing)
7. [Phased Roadmap](#7-phased-roadmap)
8. [Decision Log](#8-decision-log)
9. [Appendices](#9-appendices)

---

## 1. Executive Summary

### 1.1 Project Vision

**MONOTERMINAL** is a next-generation, multi-platform terminal emulator system designed for the distributed computing era. It enables developers to:

- **Run a single master terminal** on any platform (Linux, macOS, Windows) as a persistent daemon
- **Connect from any device** (desktop, mobile, web) to access and control terminal sessions
- **Share sessions peer-to-peer** without centralized infrastructure through WebRTC-based networking
- **Collaborate in real-time** with multiple users attached to the same terminal session
- **Persist sessions across disconnections** with automatic reconnection and state recovery

Unlike traditional terminal multiplexers (tmux, screen) limited to local or SSH-based access, MONOTERMINAL provides a modern, network-transparent terminal architecture suitable for remote work, mobile development, and collaborative debugging.

### 1.2 Scope & Boundaries

**In Scope:**

- **Master Terminal Daemon** (Rust): cross-platform PTY management, session multiplexing, GPU-accelerated rendering (wgpu), daemon mode with socket activation — **Windows first** (the initial build target machine), Linux and macOS follow in Phase 3
- **Client Application: Web (PWA) — the only client.** One React + xterm.js codebase, installable, serving desktop browsers *and* mobile browsers (Android Chrome, iOS Safari) alike. No native Android app, no native iOS app, no Tauri desktop wrapper.
- **P2P Networking**: WebRTC-based direct connections with STUN/TURN traversal, hybrid local (mDNS) and internet (directory) discovery
- **Wire Protocol**: WebSocket + Protocol Buffers, TLS 1.3 encryption, compression (zstd)
- **Security**: Ed25519 SSH keys + JWT authentication, TLS 1.3, RBAC, rate limiting
- **Persistence**: SQLite database for sessions/scrollback/audit logs, hybrid memory + disk storage
- **Collaboration**: Multi-client session attachment, input broadcasting, presence indicators
- **Monomind deep integration — first-class, not bolted on:**
  - **Per-session detection**: every session's working directory is checked for `.monomind/`; if it's missing, the master surfaces an install suggestion immediately, in-session
  - **Embedded dashboard**: org/agent/run status lives inside the same web client the user already has open — no separate port, no separate token to hunt for
  - **Embedded health check & upgrade**: a `monomind doctor`-equivalent self-check runs on a schedule and on demand, with one-click upgrade, surfaced in the same panel

**Permanently Out of Scope (decided against, not deferred):**

- **Native Android app, native iOS app, native/Tauri desktop client** — superseded by the PWA-only decision (§8.1.4). Revisit only if the web client's iOS-backgrounding trade-off (§2.2, §9.3) proves unacceptable in practice.

**Out of Scope (MVP only — later phases):**

- Split panes/tabs (Phase 4+)
- Sixel graphics (Phase 2)
- Plugin system (Phase 4+)
- Built-in SSH client (use existing SSH, attach MONOTERMINAL session)
- Video/screen sharing beyond terminal text
- Custom shells (users specify shell path in config)
- Linux and macOS master support (Phase 3 — Windows ships first)

**Platform Support:**

| Platform | Role | Target Version | Rendering | Rollout |
|----------|------|----------------|-----------|---------|
| **Windows** | Master (+ local UI) | Windows 10 1809+ (ConPTY) | wgpu (DirectX 12) | **Phase 1 — first** |
| **Web** | Client — desktop browsers | Chrome 90+, Firefox 88+, Safari 14+ | WebGL (xterm.js) | **Phase 1 — first** |
| **Web (mobile)** | Client — Android Chrome, iOS Safari | Same PWA, responsive layout | WebGL / Canvas fallback | **Phase 1 — first** |
| **Linux** | Master | Ubuntu 22.04+, Debian 11+, Fedora 38+ | wgpu (Vulkan) | Phase 3 — expansion |
| **macOS** | Master | macOS 12+ (Monterey) | wgpu (Metal) | Phase 3 — expansion |

There is no separate "mobile" platform row: a phone is just another browser hitting the same PWA. See §8.1.4 for why native Android/iOS apps were dropped. **Windows ships first** because the SRS is being handed to a Windows machine for the initial build.

### 1.3 Success Criteria

**Technical Metrics (v1.0 Target):**

- **Performance**: 60 FPS desktop rendering, <30ms LAN p95 latency, 1000 concurrent sessions per master
- **Quality**: 80% test coverage, <5 critical bugs per release, SOC 2 Type 1 compliant (Phase 3+)
- **Reliability**: 99.5% uptime for master daemon, <10s reconnection time, zero data loss on crash
- **Security**: TLS 1.3 only, Ed25519 keys, rate limiting (100 conn/min, 5 auth/hour), optional FIPS mode

**Adoption Metrics (18 months post-launch):**

- **GitHub Stars**: 10,000+ (indicates developer interest)
- **Weekly Active Users**: 1,000+ (terminal session initiations per week)
- **Contributors**: 50+ (community engagement)
- **Client Split**: ~65% desktop browser / 35% mobile browser (single PWA — no native install split to track)

**Business Metrics:**

- **MVP Cost**: $0 infrastructure (self-hosted), $50-100/month CI/CD
- **Phase 3+ Revenue**: Open-core model with enterprise features (SSO, audit logging, SLA support)

### 1.4 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        MONOTERMINAL SYSTEM                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐          ┌──────────────────┐            │
│  │  Master Terminal  │◄────────►│   Web Client     │            │
│  │  (Rust Daemon)    │   P2P    │  (PWA — only one)│            │
│  ├──────────────────┤ WebRTC   ├──────────────────┤            │
│  │ • PTY Manager    │   or      │ • Desktop browser│            │
│  │ • Session Mux    │ WebSocket │ • Mobile browser │            │
│  │ • wgpu Render    │   TLS     │   (Android/iOS)  │            │
│  │ • SQLite Store   │  Proto3   │ • xterm.js+WebGL │            │
│  │ • Daemon Mode    │           │ • Monomind panel │            │
│  │ • Monomind hooks │           │   (dashboard/    │            │
│  │                  │           │    health/upgrade)│            │
│  └──────────────────┘           └──────────────────┘            │
│         │                               │                        │
│         │                               │                        │
│         ▼                               ▼                        │
│  ┌──────────────────┐          ┌──────────────────┐            │
│  │  Local OS (PTY)  │          │  Network Layer   │            │
│  ├──────────────────┤          ├──────────────────┤            │
│  │ • openpty (L/M)  │          │ • mDNS Discovery │            │
│  │ • ConPTY (Win)   │          │ • STUN/TURN      │            │
│  │ • fork+setsid    │          │ • Directory Svc  │            │
│  └──────────────────┘          └──────────────────┘            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Data Flow (Terminal I/O):**

1. User types on mobile → WebSocket → Master daemon → PTY → Shell
2. Shell outputs → PTY → Master reads → Protocol Buffer encode → WebSocket → Fan-out to N clients
3. Each client renders independently (GPU-accelerated on desktop/mobile, Canvas on web)

**Key Differentiators:**

| Feature | tmux/screen | Eternal Terminal | MONOTERMINAL |
|---------|-------------|------------------|--------------|
| P2P Networking | ❌ | ✅ (SSH tunnel) | ✅ (WebRTC) |
| Mobile Access | ❌ | ❌ | ✅ (same PWA, mobile browser — no app install) |
| GPU Rendering | ❌ | ❌ | ✅ (wgpu/Metal, master) |
| Multi-Client Collab | ✅ | ❌ | ✅ |
| Web Client | ❌ | ❌ | ✅ (PWA — the only client) |
| Session Persistence | ✅ | ✅ | ✅ |
| Monomind-aware | ❌ | ❌ | ✅ (per-session detection, embedded dashboard) |

---

## 2. System Architecture

### 2.1 Master Terminal Host Architecture `[D1]`

The master terminal is a cross-platform Rust daemon that manages PTY sessions, multiplexes I/O, and handles client connections.

#### 2.1.1 Technology Stack

**Core Decision: Rust Rewrite** `[D1.1, D8]`

- **Rationale**: Memory safety, wgpu/egui ecosystem, cross-platform, strong async (tokio)
- **Alternatives Rejected**: 
  - Ghostty fork (Zig, MIT license but architecture mismatch - no multiplexer, ~70% net-new code needed)
  - cmux fork (~5k LOC Swift, macOS-only, no networking layer)
- **Development Effort**: 9-12 months, 0.75 FTE maintenance
- **Codebase Estimate**: ~50k LOC (renderer 15k, PTY 8k, networking 12k, protocol 6k, storage 5k, CLI 4k)

**Rendering Engine** `[D1.4]`

- **Primary**: wgpu (cross-platform GPU abstraction over Metal/Vulkan/DX12)
- **Performance**: 58-60 FPS target (16.67ms frame budget)
- **Text Shaping**: HarfBuzz (Linux/Windows), CoreText (macOS) — master platforms only; the web client's text shaping is the browser's own (§2.2)
- **Glyph Caching**: Guillotine bin-packing, 4096×4096 atlas (16MB), LRU eviction
- **Fallback**: Cairo software renderer (CPU-bound, 30-45 FPS, for VMs/SSH)

**Frame Budget (60 Hz):**
- PTY read: 2ms
- Dirty tracking: 0.5ms
- Glyph lookup: 1ms
- GPU render: 8ms
- VSync: 5ms
- **Total**: 16.5ms (within 16.67ms budget)

#### 2.1.2 PTY Management `[D1.2]`

**Linux Implementation** `[D1.2.1]`

```c
// POSIX PTY allocation workflow
int primary_fd = posix_openpt(O_RDWR | O_NOCTTY);
grantpt(primary_fd);  // Set ownership to real UID
unlockpt(primary_fd); // Unlock replica
char replica_name[128];
ptsname_r(primary_fd, replica_name, sizeof(replica_name));

// Child process setup
pid_t pid = fork();
if (pid == 0) {  // Child
    setsid();  // Create new session
    int replica_fd = open(replica_name, O_RDWR);
    dup2(replica_fd, 0);  // stdin
    dup2(replica_fd, 1);  // stdout
    dup2(replica_fd, 2);  // stderr
    if (replica_fd > 2) close(replica_fd);
    execl("/bin/bash", "bash", NULL);
}
```

**macOS Specifics** `[D1.2.2]`

- **BSD Extensions**: `openpty()` combines posix_openpt + grantpt + unlockpt
- **Daemon Integration**: launchd socket activation via `launch_activate_socket()`
- **Permissions**: No sandbox restrictions for PTY access, notarization required (macOS 10.14.5+)

**Windows ConPTY** `[D1.2.3]`

```cpp
// Windows 10 1809+ ConPTY API
HPCON hPC;
HANDLE hInput, hOutput;
CreatePipe(&hInput, ...);  // Named pipes with FILE_FLAG_OVERLAPPED
CreatePipe(&hOutput, ...);
CreatePseudoConsole({80, 24}, hInput, hOutput, 0, &hPC);

// Attach to process via STARTUPINFOEX
STARTUPINFOEX si;
si.StartupInfo.cb = sizeof(STARTUPINFOEX);
UpdateProcThreadAttribute(..., PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, hPC, ...);
CreateProcess(..., EXTENDED_STARTUPINFO_PRESENT, ...);

// Resize on window change
ResizePseudoConsole(hPC, {newCols, newRows});
```

**Key APIs:**

| OS | PTY Creation | Process Spawn | Resize |
|----|--------------|---------------|--------|
| **Linux** | posix_openpt + grantpt | fork + setsid + exec | ioctl(TIOCSWINSZ) |
| **macOS** | openpty (BSD) | fork + setsid + exec | ioctl(TIOCSWINSZ) |
| **Windows** | CreatePseudoConsole | CreateProcess + STARTUPINFOEX | ResizePseudoConsole |

#### 2.1.3 Session Management `[D1.3]`

**Session Lifecycle:**

```rust
struct Session {
    id: SessionId,          // UUID v4
    pty_master_fd: RawFd,   // PTY primary device
    shell_pid: Pid,         // Child process PID
    shell_type: String,     // "bash", "zsh", "fish", etc.
    dimensions: (u16, u16), // (rows, cols)
    working_dir: PathBuf,
    environment: HashMap<String, String>,
    scrollback: RingBuffer<Line>,  // 50k lines capacity
    clients: Vec<ClientId>,         // Attached clients
    created_at: Instant,
    last_activity: Instant,
}
```

**State Transitions:**

```
  CREATE ──> RUNNING ──┬──> DETACHED ──> REATTACHED ──┐
                       │                              │
                       └──> TERMINATED ───────────────┘
```

**Daemon Mode** `[D1.3]`

| Platform | Mechanism | Configuration | Socket Activation |
|----------|-----------|---------------|-------------------|
| **Linux** | systemd Type=notify | /etc/systemd/system/monoterminal.service | ✅ (monoterminal.socket) |
| **macOS** | launchd | ~/Library/LaunchAgents/com.monoterminal.daemon.plist | ✅ (SockServiceName) |
| **Windows** | Windows Service | SERVICE_AUTO_START | ❌ (manual socket mgmt) |

**Graceful Shutdown:**

1. Receive SIGTERM (systemd/launchd) or SERVICE_CONTROL_STOP (Windows)
2. Call `sd_notify(0, "STOPPING=1")` (systemd only)
3. Send SIGHUP to all session process groups
4. Wait up to 10s for clean exit
5. Force kill stragglers with SIGKILL
6. Flush SQLite WAL, close file descriptors
7. Exit with code 0

#### 2.1.4 Networking Layer `[D1.5]`

**IPC Mechanisms:**

| Client Type | Transport | Security | Performance |
|-------------|-----------|----------|-------------|
| **Local (Linux/macOS)** | Unix domain socket | Filesystem permissions + SO_PEERCRED | 300-500 MB/s, 5-20 µs latency |
| **Local (Windows)** | Named pipe | DACL | 200-400 MB/s, 10-30 µs latency |
| **Remote (Direct)** | TCP + TLS 1.3 | Ed25519 + JWT | 50-200 MB/s localhost, 50-200 µs |
| **Remote (P2P)** | WebRTC DataChannel | DTLS-SRTP | 10-50ms LAN, 50-150ms internet |

**Connection Flow:**

```
Client → [Connect] → Master Listener
                          ↓
                    Authentication
                          ↓
                   ┌──────┴──────┐
              ATTACH           CREATE
                ↓                 ↓
         Resume Session    New Session (fork+exec)
                ↓                 ↓
           Fan-out PTY       Start PTY I/O
```

**Session Streaming** `[D1.5]`

- **PTY Read**: Non-blocking I/O (tokio async), 4KB buffer, epoll/kqueue multiplexing
- **Fan-out**: Broadcast to all attached clients (Arc<Bytes> zero-copy)
- **Backpressure**: Per-client bounded queue (1MB), drop oldest output if full
- **Late Joiners**: Send scrollback (10k lines default, ~1MB, 50-200ms sync time)

#### 2.1.5 Monomind Integration `[D1.6]`

**Detection:** Walk upward from `cwd` to find `.monomind/` directory

**Hooks:**

| Hook | Trigger | Budget | Action |
|------|---------|--------|--------|
| **SESSION_START** | Session creation | N/A | Load org state, spawn agents, initialize context |
| **PRE_COMMAND** | Before shell exec | <100ms | Risk assessment, suggest alternatives, allow/deny |
| **OUTPUT_STREAM** | PTY chunks (1-4KB) | <5ms | Async error detection, update UI overlays |
| **POST_COMMAND** | Command exit | N/A | Store outcome in memory, routing feedback |
| **SESSION_END** | Session termination | N/A | Persist state, shutdown agents |

**Privacy:**

- **Opt-in**: User consent during initialization
- **Redaction**: Export API_KEY/PASSWORD → [REDACTED]
- **Storage**: `.monomind/` permissions 0700, files 0600
- **Encryption**: Optional SQLCipher for databases
- **Retention**: Sessions auto-delete after 30 days

---

### 2.2 Client Application — Web (PWA), the only client `[D2]`

**Decision:** one client. No native Android app, no native iOS app, no Tauri desktop wrapper — see §8.1.4 for the full rationale. The same React + xterm.js PWA serves every remote device; a phone is just another browser hitting it.

**Technology Stack:**

- **Terminal**: xterm.js 5.x + WebGL addon (55-60 FPS, 150-200KB gzipped), Canvas fallback
- **Framework**: React 18 + Vite, packaged as an installable PWA (manifest.json + Service Worker)
- **Transport**: WebSocket (Protocol Buffers) direct in Phase 1; WebRTC DataChannel P2P added in Phase 2
- **P2P**: js-libp2p signaling in-browser + native `RTCPeerConnection` — no SDK, no binary to ship

**Performance:**

| Metric | Target |
|--------|--------|
| **FPS** | 30-45 (Canvas), 55-60 (WebGL addon) |
| **Memory** | 100-150 MB |
| **Bundle** | xterm.js 150-200KB + app code 50-100KB |

Raw FPS matters less here than for a game: for a terminal, perceived responsiveness is dominated by network/P2P latency (§3.1.4, §5.1.2), not frame rate — the WebGL path is comfortably enough headroom on both desktop and modern mobile hardware.

**Installability — covers what native/Tauri used to:**

- **Desktop**: Window Controls Overlay (Chrome 90+), File Handling API (Chrome 102+) — an installed PWA looks and feels like a native window without a Tauri/Electron build
- **Mobile**: Add to Home Screen on Android and iOS; manifest.json + Service Worker + HTTPS + basic engagement (2 visits, 5 min) triggers the install prompt
- **Offline**: app shell cached; session data shows "Reconnecting" rather than stale content

**Keyboard:**

- On-screen accessory row for touch (Blink-style Esc / Tab / Ctrl / Alt / arrows)
- Native `KeyboardEvent` capture for external/Bluetooth keyboards
- Accepted limitation: cannot intercept certain reserved browser shortcuts (Ctrl+T/W/N, F11)

**Known limitation — iOS Safari backgrounding (accepted, not a blocker):**

Safari suspends WebRTC and Web Audio the moment the app is backgrounded or the screen locks, and a web app cannot claim the native `AVAudioSession` background-audio privilege that a real iOS app would use to stay alive (confirmed against current Apple Developer Forum threads, 2026). MONOTERMINAL does not try to defeat this. Instead, reconnection is designed to be the *normal* path rather than a rare recovery case: the `<10s reconnection time` target and late-joiner scrollback resync (§2.1.4, `[D1.5]`) already exist for every client, so a phone that locks and unlocks just reconnects and catches up on the last 10k lines — the same flow a flaky WiFi handoff would trigger anyway.

**No PTY, no raw sockets** — same as any browser: the master hosts the process, the client never does. Clipboard access requires a user gesture (standard async Clipboard API).

**Monomind panel**: the embedded dashboard, health check, and upgrade controls (§2.4) live inside this same client — see below.

---

### 2.4 Monomind Deep Integration `[D1.6, D13]`

Monomind is not an optional add-on bolted onto the terminal — it is a first-class part of the master and the web client, present from Phase 1.

#### 2.4.1 Per-Session Detection & Install Suggestion `[D13.1]`

On every new session (and re-checked when the working directory changes — `cd` into a different project), the master resolves the PTY's `cwd` and walks upward looking for a `.monomind/` directory, exactly as the existing detection logic does (§2.1.5). **If none is found, the suggestion happens immediately, in that session** — not buried in a settings page:

- A short, dismissible banner is written into the session output (MOTD-style) and mirrored as a toast in the web client: *"This project doesn't have monomind — install it to unlock org/swarm features for this session."*
- One-line install command included inline; a single click/tap from the web client can trigger it directly via the master (with confirmation, per the tool's own risk category for running install scripts)
- Suppressible **per project**, not globally — a `.monomind-suggest-dismissed` marker (or equivalent local state keyed by project path) means a user who says "not for this repo" isn't nagged every session, while a genuinely new, un-configured project still gets the prompt

#### 2.4.2 Embedded Dashboard `[D13.2]`

This project's own build hit a real, reproducible class of bug: the standalone monomind dashboard is a separate control-server process, discovered via a broker file and a rotating credential, and it silently failed to connect more than once during development (filed upstream as [monoes/monomind#135](https://github.com/monoes/monomind/issues/135) and [#136](https://github.com/monoes/monomind/issues/136)) — dropped auth credentials, dead foreign-server pairing logic, no self-heal, no visible warning when it wasn't working. That's the exact failure mode this feature is designed to make impossible for MONOTERMINAL's own users.

The dashboard is **embedded directly inside the web client**, authenticated through the same session JWT the terminal connection already uses:

- Live org/agent status, run history, and knowledge-graph/memory stats as a panel in the same UI the user already has open
- No separate port to discover, no separate token file to find, no "is the control server even running" question — if the web client is connected, the dashboard is connected
- Reachable per-session, scoped to the project the session belongs to

#### 2.4.3 Health Check & Upgrade `[D13.3]`

A `monomind doctor`-equivalent self-check runs on a schedule (daily) and on demand:

- Verifies CLI version, control-server reachability, and broker-registration integrity (the specific class of thing that broke during this project's own build)
- Surfaces pass/fail state as a status chip in the same embedded panel — not a silent log line
- One-click **Upgrade** action, gated the same way any install/upgrade action is (explicit confirmation, per the tool's action-risk policy)

---

### 2.3 P2P Networking Architecture `[D3]`

#### 2.3.1 P2P Transport: WebRTC-Only Decision `[D3, D8.5]`

**Why WebRTC over libp2p:**

| Factor | libp2p | WebRTC | Decision |
|--------|--------|--------|----------|
| **Mobile Support** | No native client to weigh — MONOTERMINAL ships no mobile binary at all (§8.1.4); browser is the only surface | Built into every mobile browser (`RTCPeerConnection`), zero extra binary | ✅ WebRTC |
| **Browser Interop** | js-libp2p available but immature | Native browser API (RTCPeerConnection) | ✅ WebRTC |
| **NAT Traversal** | ~70% ± 7% measured (real 4.4M-attempt study, TCP≈QUIC) | No comparably rigorous figure exists — measure directly against our own traffic (§7.2) | ✅ WebRTC (qualitative case, §8.2.1 note) |
| **Cellular** | Untested on mobile carriers | Proven (Google Meet, Zoom use it) | ✅ WebRTC |
| **Maintenance** | rust-libp2p mobile updates lag | Google maintains WebRTC across every browser — no SDK for us to track | ✅ WebRTC |

**Architecture:**

```
Master (Rust)          Client (Kotlin/Swift/JS)
     │                          │
     │  ◄─── SDP Offer ─────    │
     │  ───► SDP Answer ────►   │
     │  ◄─── ICE Candidate ──   │
     │  ───► ICE Candidate ──►  │
     │                          │
     └──── DataChannel (DTLS) ───┘
           Protobuf messages
```

#### 2.3.2 NAT Traversal `[D3]`

**STUN Servers** (free, public):
- stun:stun.l.google.com:19302 (primary)
- stun1.l.google.com:19302 (backup)

**TURN Relay** (self-hosted requirement):
- **Software**: coturn (open source, C)
- **Cost**: $5-15/month VPS (100GB bandwidth)
- **Config**: 
  ```
  listening-port=3478
  tls-listening-port=5349
  realm=monoterminal.example.com
  ```

**Success Rates:**

| Network Type | STUN Direct | TURN Fallback |
|--------------|-------------|---------------|
| **WiFi (home)** | 85-95% | 98-99% |
| **Cellular (4G/5G)** | 60-75% (carrier NAT) | 98-99% |
| **Corporate VPN** | 40-55% (double NAT) | 98-99% |

**Relay Fallback Strategy:**

1. Attempt STUN direct connection (timeout: 10s)
2. If fails, use TURN relay
3. If TURN unavailable, fall back to HTTPS relay (master acts as WebSocket relay server)

#### 2.3.3 Discovery `[D3]`

**Hybrid Discovery Model:**

| Method | Scope | Latency | Reliability |
|--------|-------|---------|-------------|
| **mDNS/Bonjour** | LAN only | 1-5s | HIGH (blocked by mobile hotspot) |
| **Directory Service** | Internet | <100ms | MEDIUM (single point of failure) |
| **Kademlia DHT** | Internet | 2-10s | HIGH (decentralized) |

**Recommended Flow:**

```
App Launch
    │
    ├─► mDNS Query (parallel)
    ├─► Directory Query (parallel)
    └─► DHT Query (parallel)
         │
         ▼
    Merge Results → Dedupe by Peer ID → Sort (mDNS first, then Directory, then DHT)
```

**Directory Service API:**

```
POST /api/v1/register
{
  "peer_id": "12D3KooW...",
  "multiaddrs": ["/ip4/192.168.1.10/tcp/5000", "/ip4/203.0.113.5/tcp/5000"],
  "hostname": "laptop.local",
  "session_count": 5
}

GET /api/v1/discover?peer_id=12D3KooW...
Response: { "peer": {...}, "last_seen": 1723641600 }
```

#### 2.3.4 Connection Limits `[D3]`

**Global Limits:**
- **Max Total Connections**: 1000 (cross-platform tested)
- **Max Per Session**: 50 clients (prevents fan-out overload)
- **Connection Rate Limit**: 100 new connections/minute (DDoS protection)

**Resource Quotas:**

| Platform | Mechanism | Configuration |
|----------|-----------|---------------|
| **Linux** | cgroups v2 | memory.max=6GB, cpu.max=800000 (8 cores @ 10% avg) |
| **macOS** | launchd limits | HardResourceLimits (ResidentMemoryMax=6GB) |
| **Windows** | Job Objects | JOB_OBJECT_LIMIT_PROCESS_MEMORY=6GB |

---

## 3. Protocol & Security

### 3.1 Wire Protocol Design `[D4]`

#### 3.1.1 Protocol Buffers Schema `[D4.1]`

**Message Envelope:**

```protobuf
syntax = "proto3";
package monoterminal.v1;

message Envelope {
  uint64 sequence_number = 1;  // Monotonic counter per connection
  oneof message {
    AttachRequest attach_request = 2;
    AttachResponse attach_response = 3;
    InputData input_data = 4;
    OutputData output_data = 5;
    ResizeRequest resize_request = 6;
    DetachRequest detach_request = 7;
    ErrorResponse error_response = 8;
  }
}

message AttachRequest {
  string session_id = 1;         // UUID or empty for new session
  string auth_token = 2;         // JWT
  uint32 rows = 3;               // Terminal dimensions
  uint32 cols = 4;
  uint64 last_seen_sequence = 5; // For late-joiner sync
}

message AttachResponse {
  string session_id = 1;
  SessionMetadata metadata = 2;
  repeated Line scrollback = 3;  // Last 10k lines
}

message InputData {
  bytes data = 1;  // Raw keyboard input (UTF-8)
}

message OutputData {
  bytes data = 1;          // PTY output chunk
  uint64 sequence = 2;     // For ordering/dedup
  CompressionType compression = 3;
}

enum CompressionType {
  NONE = 0;
  ZSTD = 1;
}

message ResizeRequest {
  uint32 rows = 1;
  uint32 cols = 2;
}

message ErrorResponse {
  ErrorCode code = 1;
  string message = 2;
}

enum ErrorCode {
  UNKNOWN = 0;
  SESSION_NOT_FOUND = 1;
  AUTH_FAILED = 2;
  PERMISSION_DENIED = 3;
  RATE_LIMIT_EXCEEDED = 4;
}
```

#### 3.1.2 Transport: WebSocket Framing `[D4.2]`

**Why WebSocket over Raw TCP:**

- Browser compatibility (xterm.js web client requires WebSocket)
- Built-in message framing (no manual length-prefix framing)
- Automatic fragmentation/reassembly
- Compatible with P2P (WebRTC DataChannel shares API surface)

**Frame Format:**

```
WebSocket Binary Frame
    │
    ├─► Protocol Buffer Envelope (variable length, self-delimiting)
    └─► Compression (optional, per-message zstd if enabled)
```

**No additional framing needed:** WebSocket provides message boundaries, Protocol Buffers are self-delimiting.

#### 3.1.3 Compression `[D4.3]`

**Algorithm**: zstd (Zstandard)

**Rationale:**
- **Ratio**: 50-60% compression on terminal text
- **Speed**: 300-500 MB/s compress, 500-900 MB/s decompress (single core)
- **Latency**: <1ms for 4KB chunk
- **Memory**: ~2 MB per stream

**Triggering:**

- **Threshold**: Enable compression if output chunk >4KB
- **Per-Client**: Enable if client advertises support (AttachRequest.supports_compression=true)
- **Adaptive**: Enable if client write buffer >50% full (detect slow client)

**Example:**

```
Uncompressed: 10 KB terminal output (cat large.log)
Compressed:   4-5 KB (zstd level 3)
Overhead:     0.5ms compression + 0.2ms decompression
Savings:      ~50% bandwidth, acceptable for >4KB chunks
```

#### 3.1.4 Output Buffering & Flow Control `[D4.4]`

**Master PTY Read:**

- **Buffer Size**: 4KB per read
- **Trigger**: PTY fd readable (epoll/kqueue edge-triggered)
- **Rate**: Read as fast as PTY produces (no throttling at source)

**Flush Triggers:**

1. **Size**: Buffer ≥4KB → flush immediately
2. **Time**: 100ms elapsed since last flush → flush partial buffer
3. **Newline**: Detect '\n' → flush (interactive responsiveness)

**Fan-Out Strategy:**

```rust
// 1→N broadcast (single read, N writes)
let chunk = pty_read(4096)?;  // Read once from PTY
let bytes = Arc::new(chunk);  // Zero-copy Arc

for client in session.clients.iter() {
    client.send(bytes.clone())?;  // Reference-counted, no copy
}
```

**CPU Savings**: 40-60% vs N×1 (per-client PTY reads)

**Backpressure:**

- **Per-Client Buffer**: 1 MB bounded queue
- **Slow Client Detection**: Buffer >80% full for >5s
- **Action**: 
  1. Enable compression (if not already)
  2. Send LAGGING warning to client
  3. If buffer full, drop oldest output (lossy, acceptable for terminals)
  4. Disconnect client if lagging >30s

**Latency Targets:**

| Scenario | Latency (p50) | Latency (p95) |
|----------|---------------|---------------|
| **LAN** | <10ms | <30ms |
| **Internet (Direct)** | <50ms | <150ms |
| **TURN Relay** | <100ms | <300ms |

---

### 3.2 Security Architecture `[D5]`

#### 3.2.1 Transport Security `[D5.1]`

**TLS 1.3 Only** (reject TLS 1.2 and earlier)

**Rationale:**
- Forward secrecy (ephemeral keys)
- Faster handshake (1-RTT vs 2-RTT)
- Removes insecure cipher suites (RC4, DES, 3DES)
- Encrypted SNI (privacy)

**Implementation:**

| Component | Library | Configuration |
|-----------|---------|---------------|
| **Master (Rust)** | rustls 0.21+ | TLS 1.3 only, no session tickets |
| **Web Client** (the only client, §2.2) | Browser native (no config) | TLS 1.3 automatic (Chrome 70+, Firefox 63+, Safari 12.1+) |

**Cipher Suites** (preference order):
1. TLS_AES_256_GCM_SHA384 (strongest)
2. TLS_AES_128_GCM_SHA256 (default)
3. TLS_CHACHA20_POLY1305_SHA256 (mobile optimization, NEON/SIMD acceleration)

**Certificate Management:**

- **Development**: Self-signed cert (user accepts on first connection, TOFU model)
- **Production**: Let's Encrypt cert (master daemon can run certbot for HTTPS endpoint)
- **P2P**: DTLS-SRTP for WebRTC (automatic, browser/SDK managed)

#### 3.2.2 Authentication `[D5.2]`

**Multi-Factor Authentication Flow:**

```
Client                          Master
  │                               │
  │  ──── TLS Handshake ────►     │  (Certificate validation)
  │  ◄─── Server Hello ──────     │
  │                               │
  │  ──── Challenge Request ──►   │
  │  ◄─── Challenge (nonce) ──    │  (256-bit random)
  │                               │
  │  (Sign challenge with        │
  │   Ed25519 private key)       │
  │                               │
  │  ──── Signed Challenge ───►   │
  │                               │
  │                              │  (Verify signature with stored public key)
  │                              │  (Generate JWT)
  │                               │
  │  ◄─── JWT (access+refresh)─   │
  │                               │
  │  ──── AttachRequest+JWT ──►   │
  │  ◄─── AttachResponse ─────    │
```

**Ed25519 SSH Keys:**

- **Why Ed25519**: Fast (50µs sign, 100µs verify), small (32-byte keys, 64-byte signatures), modern (vs RSA 2048-bit)
- **Storage**: `~/.ssh/monoterminal_ed25519` (private key), `~/.ssh/monoterminal_ed25519.pub` (public key)
- **Key Generation**: `ssh-keygen -t ed25519 -f ~/.ssh/monoterminal_ed25519`

**JWT Tokens:**

```json
{
  "typ": "JWT",
  "alg": "EdDSA"
}
{
  "sub": "user@example.com",
  "iss": "monoterminal-master",
  "exp": 1723642500,  // 15 minutes from issue
  "iat": 1723641600,
  "scope": "session:attach session:create session:kill"
}
```

- **Access Token**: 15 minutes TTL (short-lived, included in each request)
- **Refresh Token**: 30 days TTL (stored securely, used to obtain new access token)
- **Rotation**: Refresh token rotated on each use (refresh token reuse detection)

#### 3.2.3 Authorization (RBAC) `[D5.3]`

**Roles:**

| Role | Permissions | Use Case |
|------|-------------|----------|
| **admin** | session:*, client:*, config:write | Master owner |
| **user** | session:attach, session:create, session:resize, input:write | Regular user |
| **read-only** | session:attach, input:none | Pair programming observer |

**Session-Level Permissions:**

```rust
struct SessionPermissions {
    owner_uid: Uid,
    allowed_users: HashMap<Uid, Permission>,
}

enum Permission {
    Owner,      // Full control
    ReadWrite,  // Attach, input, resize
    ReadOnly,   // Attach only, no input
}
```

**Enforcement:**

```rust
fn check_permission(session: &Session, user: &User, action: Action) -> Result<(), Error> {
    if session.owner_uid == user.uid {
        return Ok(()); // Owner always allowed
    }
    
    let perm = session.allowed_users.get(&user.uid)
        .ok_or(Error::PermissionDenied)?;
    
    match action {
        Action::Attach if *perm >= Permission::ReadOnly => Ok(()),
        Action::Input | Action::Resize if *perm >= Permission::ReadWrite => Ok(()),
        Action::Kill if *perm == Permission::Owner => Ok(()),
        _ => Err(Error::PermissionDenied),
    }
}
```

#### 3.2.4 Rate Limiting `[D5.4]`

**Limits:**

| Resource | Limit | Window | Action on Exceed |
|----------|-------|--------|------------------|
| **New Connections** | 100 | 1 minute | Reject with 429 Too Many Requests |
| **Auth Attempts** | 5 | 1 hour | Temporary ban (15 min) |
| **Session Creates** | 20 | 1 minute | Reject with 429 |
| **Input Rate** | 10 KB/s | Per session | Drop excess input |
| **Output Rate** | No limit | - | Backpressure per client |

**Implementation**: Token bucket algorithm (tokio-tower rate limiter)

#### 3.2.5 Optional FIPS Mode `[D5.5]`

**Enterprise Requirement**: FIPS 140-2 compliance for government/regulated sectors

**Changes:**

- **TLS**: Use FIPS-validated OpenSSL (not rustls)
- **Hashing**: SHA-256/384/512 only (no BLAKE2, SHA-1)
- **Random**: Use `/dev/random` (blocking) instead of `/dev/urandom` on Linux
- **Ciphers**: AES-GCM only (no ChaCha20-Poly1305)

**Enable**: `monoterminal-master --fips-mode` (compile-time feature flag)

---

## 4. Data & Platform

### 4.1 Database & State Management `[D6]`

#### 4.1.1 SQLite Schema `[D6.1]`

**Why SQLite over PostgreSQL:**

- Embedded (no separate server process)
- Performance sufficient (100k INSERT/s, <1ms SELECT)
- File-based (easy backup/restore)
- Zero configuration
- Cross-platform

**Schema:**

```sql
-- sessions table
CREATE TABLE sessions (
    session_id TEXT PRIMARY KEY,
    owner_uid INTEGER NOT NULL,
    shell_path TEXT NOT NULL,
    shell_pid INTEGER,
    working_dir TEXT,
    rows INTEGER NOT NULL,
    cols INTEGER NOT NULL,
    created_at INTEGER NOT NULL,   -- Unix timestamp
    last_activity INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active', 'detached', 'terminated'))
);
CREATE INDEX idx_sessions_owner ON sessions(owner_uid);
CREATE INDEX idx_sessions_status ON sessions(status);

-- clients table
CREATE TABLE clients (
    client_id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(session_id) ON DELETE CASCADE,
    peer_addr TEXT NOT NULL,
    user_id TEXT,
    connected_at INTEGER NOT NULL,
    last_seen INTEGER NOT NULL
);
CREATE INDEX idx_clients_session ON clients(session_id);

-- scrollback table (overflow from memory)
CREATE TABLE scrollback (
    session_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    line_data BLOB NOT NULL,  -- zstd compressed
    PRIMARY KEY (session_id, line_number),
    FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
);

-- session_permissions table
CREATE TABLE session_permissions (
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    permission TEXT NOT NULL CHECK(permission IN ('owner', 'read_write', 'read_only')),
    granted_at INTEGER NOT NULL,
    PRIMARY KEY (session_id, user_id),
    FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
);

-- audit_log table
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    session_id TEXT,
    details TEXT,  -- JSON
    result TEXT NOT NULL CHECK(result IN ('success', 'failure'))
);
CREATE INDEX idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_user ON audit_log(user_id);
```

#### 4.1.2 WAL Mode & Performance `[D6.2]`

**Write-Ahead Logging (WAL):**

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;  -- Balanced durability/performance
PRAGMA cache_size = -64000;   -- 64 MB cache
PRAGMA mmap_size = 268435456; -- 256 MB mmap
PRAGMA temp_store = MEMORY;
```

**Benefits:**

- Concurrent readers (multiple SELECT while one INSERT)
- ~30% faster writes vs DELETE journal
- No journal delete overhead

**Performance:**

| Operation | Throughput | Latency |
|-----------|------------|---------|
| **INSERT** (single) | 10k/s | ~0.1ms |
| **INSERT** (batched 1000) | 100k/s | ~10ms per batch |
| **SELECT** (indexed) | 1M/s | <1ms |
| **UPDATE** (indexed) | 50k/s | ~0.02ms |

#### 4.1.3 Hybrid Scrollback Storage `[D6.3]`

**Strategy:** Memory (hot) + SQLite (cold)

```rust
struct HybridScrollback {
    memory: RingBuffer<Line>,  // 10k lines (or 10 MB, whichever first)
    overflow_db: SqliteConnection,
}

impl HybridScrollback {
    fn append(&mut self, line: Line) {
        if self.memory.is_full() {
            // Evict oldest 1000 lines to SQLite
            let batch = self.memory.drain_oldest(1000);
            self.overflow_db.batch_insert_compressed(batch)?;
        }
        self.memory.push(line);
    }
    
    fn get_range(&self, start: usize, end: usize) -> Vec<Line> {
        let memory_start = self.total_lines - self.memory.len();
        
        if start >= memory_start {
            // All in memory
            self.memory.slice(start - memory_start, end - memory_start)
        } else if end <= memory_start {
            // All in SQLite
            self.overflow_db.fetch_lines(start, end)
        } else {
            // Split: SQLite + memory
            let db_lines = self.overflow_db.fetch_lines(start, memory_start);
            let mem_lines = self.memory.slice(0, end - memory_start);
            [db_lines, mem_lines].concat()
        }
    }
}
```

**Compression:** zstd level 3 (50-60% ratio, <1ms per 1000 lines)

**Batch Insert:**

```rust
// Batch 1000 lines every 5 seconds or when buffer full
let mut batch = Vec::with_capacity(1000);
loop {
    tokio::select! {
        line = line_rx.recv() => {
            batch.push(compress_line(line));
            if batch.len() >= 1000 {
                db.execute_batch_insert(&batch)?;
                batch.clear();
            }
        }
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            if !batch.is_empty() {
                db.execute_batch_insert(&batch)?;
                batch.clear();
            }
        }
    }
}
```

**Target Capacity:**

- **1000 sessions** × 10 MB avg scrollback = **10 GB total**
- **Memory**: 1000 × 1 MB (10k lines in RAM) = **1 GB**
- **SQLite**: ~9 GB (compressed)

---

### 4.2 Cross-Platform Desktop `[D7]`

#### 4.2.1 Master Terminal Desktop App `[D7.1]`

**Stack:** egui (immediate mode GUI) + wgpu (rendering)

**Rationale:**
- **Performance**: 60 FPS achievable with wgpu GPU rendering
- **Cross-Platform**: Single Rust codebase (Linux/macOS/Windows)
- **Binary Size**: 5-15 MB (smaller than Electron)
- **Memory**: 40-80 MB (vs 150-300 MB Electron)
- **Startup**: 150-300ms (vs 500-1500ms Electron)

**UI Structure:**

```
┌────────────────────────────────────────────┐
│  Menu Bar (File, Session, View, Help)     │
├────────────────────────────────────────────┤
│  Session List (sidebar, collapsible)      │
│  ┌──────────────────────────────────┐     │
│  │  Terminal Canvas (wgpu)          │     │
│  │  ┌────────────────────────────┐  │     │
│  │  │                            │  │     │
│  │  │  PTY Output (GPU rendered) │  │     │
│  │  │                            │  │     │
│  │  └────────────────────────────┘  │     │
│  └──────────────────────────────────┘     │
├────────────────────────────────────────────┤
│  Status Bar (connection, latency, FPS)    │
└────────────────────────────────────────────┘
```

**Threading:**

- **Main Thread**: egui UI + event loop
- **PTY Thread**: tokio async runtime for I/O
- **Render Thread**: wgpu command submission

#### 4.2.2 Desktop "Client" — Removed `[superseded, D7.2]`

v1.0 paired the master's egui+wgpu UI with a separate Tauri-wrapped desktop client for *remote* access from another desktop. v1.1 removes this entirely: a remote desktop user reaches a master through the same installable web PWA (§2.2) as everyone else — Window Controls Overlay and File Handling API give it a native-feeling window without a second Rust/Tauri codebase, code-signing budget, or release pipeline to maintain. The distribution costs (Apple Developer, EV code signing) that used to apply to this client no longer apply to anything except the master binary itself (§7.3).

---

## 5. Performance & UX

### 5.1 Performance & Scalability `[D9]`

#### 5.1.1 Master Capacity `[D9.1]`

**Target**: 1000 concurrent sessions

**Resource Requirements:**

| Metric | Per Session | 1000 Sessions |
|--------|-------------|---------------|
| **Memory** | 7 MB (1 MB scrollback + 2 MB PTY buffers + 4 MB overhead) | **7 GB** |
| **CPU (Idle)** | 0.01% | **10%** (100 cores × 0.01%) |
| **CPU (Active 10%)** | 0.75% | **7.5 cores** |
| **File Descriptors** | 2 (PTY master + client socket) | **2000** (ulimit -n 65536 required) |

**Optimization Strategies:**

- **Connection Pooling**: Reuse tokio async tasks, avoid per-connection thread
- **Zero-Copy**: Arc<Bytes> for broadcast, avoid memcpy
- **Lazy Scrollback**: Load from SQLite only on client request
- **Adaptive Compression**: Enable only for slow clients (>50% buffer full)

#### 5.1.2 Network Performance `[D9.2]`

**Bandwidth:**

| Scenario | Bandwidth | Notes |
|----------|-----------|-------|
| **Idle SSH** | <1 Kbps | Keepalive pings (30s interval) |
| **Interactive Typing** | 1-5 Kbps | User input + echo |
| **Tail -f Log** | 10-100 Kbps | Continuous output |
| **Cat Large File** | 1-10 Mbps | Burst, limited by terminal render speed |

**Latency Targets:**

| Scenario | p50 | p95 | p99 |
|----------|-----|-----|-----|
| **LAN** | <10ms | <30ms | <50ms |
| **Internet (Direct)** | <50ms | <150ms | <300ms |
| **TURN Relay** | <100ms | <300ms | <500ms |

**Adaptive Batching:**

- **Low Latency** (<50ms): Flush every 10ms or 1KB (whichever first)
- **High Latency** (>100ms): Batch up to 100ms or 8KB (amortize RTT)

#### 5.1.3 Mobile Battery `[D9.3]` — revised for a browser client

The v1.0 targets below assumed a native app with its own power profile via Battery Historian / Xcode Energy Log. As of v1.1 there is no native binary to instrument that way (§8.1.4) — mobile is the same web PWA as desktop, running inside the OS's own browser process, so MONOTERMINAL doesn't control battery accounting directly. What it *does* control:

- **Adaptive FPS**: 0 FPS when the tab/PWA is backgrounded (moot on iOS Safari once WebRTC itself suspends — §2.2), 30 FPS active, 60 FPS only for smooth scrolling
- **Coalescing**: batch output updates (50ms delay, reduce wakeups from 60 Hz to 20 Hz) — same technique, now implemented in the render loop of the single web client instead of duplicated across three native codebases
- **Compression**: zstd enabled on cellular connections to cut radio-on time

**Acceptance criterion, revised:** rather than an absolute mAh/hour target (unmeasurable without a native app), Phase 2 validates that a moderate-use mobile browser session doesn't visibly outpace the battery drain of an equivalent native chat/video app on the same device — a relative, browser-realistic bar instead of a fabricated absolute number.

---

### 5.2 User Experience & Features `[D10]`

#### 5.2.1 Configuration `[D10.1]`

**Format**: TOML (`~/.config/monoterminal/config.toml`)

```toml
[terminal]
shell = "/bin/bash"
scrollback_lines = 50000
font_family = "JetBrains Mono"
font_size = 14
line_height = 1.2

[theme]
# One of: monokai, solarized-dark, dracula, nord, gruvbox, ...
# or path to custom theme file
name = "monokai"

[network]
listen_address = "127.0.0.1:5000"
enable_p2p = true
stun_servers = ["stun:stun.l.google.com:19302"]
turn_server = "turn:turn.example.com:3478"
turn_username = "user"
turn_password = "secret"

[security]
tls_cert = "~/.config/monoterminal/cert.pem"
tls_key = "~/.config/monoterminal/key.pem"
authorized_keys = "~/.ssh/monoterminal_authorized_keys"
fips_mode = false

[storage]
database_path = "~/.local/share/monoterminal/sessions.db"
scrollback_compression = true

[features]
clipboard_osc52 = true
hyperlinks = true
sixel_graphics = false  # Phase 2
```

**Hot Reload**: Watch config file (inotify/FSEvents/ReadDirectoryChangesW), reload on change (except TLS certs - requires daemon restart)

#### 5.2.2 Advanced Features `[D10.2]`

**True Color:**

- 24-bit RGB via SGR sequences (`\e[38;2;R;G;Bm`)
- Supported on all platforms (wgpu RGBA8 textures)

**Hyperlinks:**

- OSC 8 sequences: `\e]8;;http://example.com\e\\clickable text\e]8;;\e\\`
- Ctrl+Click or Cmd+Click to open in browser

**Sixel Graphics** (Phase 2):

- Inline images via Sixel escape sequences
- libsixel integration (Rust bindings)
- Render to separate texture layer, composite over text

**Clipboard:**

- OSC 52 (remote → local): `\e]52;c;<base64>\e\\` (server sets client clipboard)
- Bracketed Paste: `\e[?2004h` enables, pastes wrapped in `\e[200~...\e[201~`
- Smart Paste: Detect >10 lines or special chars (;|&), prompt user

**Tabs/Splits** (Phase 4):

- Not in MVP (use tmux inside MONOTERMINAL for now)
- Future: Master-side window management (similar to WezTerm multiplexer)

#### 5.2.3 Collaboration Features `[D10.3]`

**Multi-Attach:**

- N clients attach to same session
- Each client sees same PTY output (broadcast)
- All clients can send input simultaneously (race conditions possible, similar to tmux)

**Presence Indicators:**

- Master tracks `client_id`, `user_id`, `connected_at`, `last_seen`
- Broadcast to all clients: `ClientJoined{user: "alice@laptop"}`, `ClientLeft{user: "bob@phone"}`
- UI shows avatars/badges: "Alice (laptop), Bob (phone)"

**Input Broadcasting** (optional mode):

- **All-Send Mode** (default): All clients can type, potential conflicts
- **Moderator Mode**: One client has write permission, others read-only, can request control
- **Input Lock**: Temporary exclusive input (30s timeout)

**Cursor Sharing** (Google Docs style):

- Each client broadcasts cursor position: `{client_id, row, col}` (debounced 100ms)
- Master fans out to all clients
- Render colored rectangles with labels: "Alice at row 10, col 5"
- Limitation: UI cursor only (not PTY cursor position)

**Scrollback Sync:**

- Each client tracks viewport: `{top_line, bottom_line}`
- Broadcast debounced (500ms): `ScrollViewport{client_id, top_line}`
- Show indicator: "Alice viewing line 1500" (optional follow mode)

---

## 6. Development & Testing

### 6.1 Testing Strategy `[D11.1]`

**Target**: 80% code coverage

**Testing Pyramid:**

```
        /\
       /  \        E2E (5%): Full system tests (slow, comprehensive)
      /────\
     /      \      Integration (25%): Multi-component tests (moderate speed)
    /────────\
   /          \    Unit (70%): Isolated component tests (fast, focused)
  /────────────\
```

**Tools:**

| Type | Tool | Platform | Purpose |
|------|------|----------|---------|
| **Unit** | cargo test | Rust | PTY logic, protocol parsing, state management |
| **Integration** | cargo test --test | Rust | Client-server handshake, session lifecycle |
| **E2E** | Python + pytest | Cross-platform | Full workflow (attach, type, receive output, detach) |
| **Property** | proptest | Rust | Fuzz protocol parser, state machine transitions |
| **Snapshot** | insta | Rust | VT sequence rendering (golden files) |
| **Benchmark** | criterion.rs | Rust | Latency, throughput, memory |

**Fuzzing:**

```bash
cargo install cargo-fuzz
cargo fuzz run protobuf_parser -- -max_total_time=600  # 10 min
cargo fuzz run pty_output -- -max_total_time=600
```

**CI Test Matrix:**

| OS | Arch | Rust Version | PTY | Status |
|----|------|--------------|-----|--------|
| **Ubuntu 22.04** | x86_64 | stable, beta | openpty | ✅ Required |
| **Ubuntu 22.04** | aarch64 | stable | openpty | ✅ Required (ARM) |
| **macOS 13** | x86_64 | stable | openpty | ✅ Required |
| **macOS 13** | aarch64 | stable | openpty | ✅ Required (Apple Silicon) |
| **Windows 11** | x86_64 | stable | ConPTY | ✅ Required (Phase 2) |
| **Windows 11** | aarch64 | stable | ConPTY | ⚠️ Optional (ARM64 Windows) |

---

### 6.2 CI/CD Pipeline `[D11.2]`

**Platform**: GitHub Actions

**Cost**: $50-100/month (Linux free, macOS/Windows runner minutes)

**Workflows:**

**1. Pull Request Checks** (`.github/workflows/pr.yml`):

```yaml
name: Pull Request
on: [pull_request]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-22.04, macos-13, windows-2022]
        rust: [stable, beta]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@${{ matrix.rust }}
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check
  
  coverage:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --out Xml --all-features
      - uses: codecov/codecov-action@v3
```

**2. Release Build** (`.github/workflows/release.yml`):

```yaml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - os: macos-13
            target: x86_64-apple-darwin
          - os: macos-13
            target: aarch64-apple-darwin
          - os: windows-2022
            target: x86_64-pc-windows-msvc
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - run: tar czf monoterminal-${{ matrix.target }}.tar.gz -C target/${{ matrix.target }}/release monoterminal
      - uses: softprops/action-gh-release@v1
        with:
          files: monoterminal-${{ matrix.target }}.tar.gz
```

**3. Automation**: `release-please` (auto-generate CHANGELOG, bump version, tag)

---

### 6.3 Development Workflow `[D11.3]`

**Branching**: Trunk-based development

```
main (protected)
  │
  ├── feature/websocket-compression
  ├── fix/pty-resize-race
  └── refactor/client-auth
```

**Rules:**

- All changes via PR (no direct pushes to main)
- Require 1 approving review
- CI must pass (tests + clippy + fmt)
- Squash merge (clean history)

**Tools:**

| Tool | Purpose | Configuration |
|------|---------|---------------|
| **rust-analyzer** | LSP (autocomplete, goto-def, inline errors) | VS Code/Neovim |
| **lldb** / **gdb** | Debugging (breakpoints, inspect state) | CLI or VS Code |
| **cargo-flamegraph** | CPU profiling (identify hotspots) | `cargo flamegraph --bin monoterminal` |
| **RenderDoc** | GPU profiling (Metal/Vulkan frame capture) | Standalone app |
| **Heaptrack** / **Valgrind** | Memory profiling (leaks, allocations) | Linux |

**Documentation:**

- **API Docs**: `cargo doc --no-deps --open` (inline rustdoc comments)
- **User Guide**: mdBook (`docs/` directory)
- **ADRs**: Architecture Decision Records (`docs/decisions/`)

---

## 7. Phased Roadmap

**Sequencing principle (v1.2):** prove the whole system on one platform pair — **Windows master + Web client** — before spending a single day on Linux or macOS. Windows first because the SRS ships to a Windows machine for the initial build. Native mobile is not in this roadmap at all; the web client *is* the mobile client (§2.2, §8.1.4).

### 7.1 Phase 1 — Windows + Web (Months 1-3) `[D12.1]`

**Goal**: A complete, monomind-aware system on exactly one platform pair, proving the architecture end to end before expanding.

**Features:**

- ✅ Master daemon — **Windows only** (ConPTY, Windows Service)
- ✅ Master's local terminal UI (egui + wgpu, DirectX 12)
- ✅ **Web client (PWA)** — desktop *and* mobile browsers, from day one (§2.2)
- ✅ Direct connection: WebSocket + TLS 1.3 + Ed25519/JWT auth (no P2P yet — that's Phase 2)
- ✅ Session creation, attach/detach, in-memory scrollback (10k lines)
- ✅ Basic config (TOML)
- ✅ **Monomind integration — first-class, not deferred:** per-session `.monomind/` detection & install suggestion (§2.4.1), embedded dashboard (§2.4.2), embedded health check & upgrade (§2.4.3)

**Explicitly not in Phase 1:**

- ❌ Linux / macOS master
- ❌ P2P (WebRTC) — direct WebSocket only for now
- ❌ Multi-session, compression, SQLite persistence, collaboration
- ❌ Native mobile apps — permanently out of scope (§8.1.4)

**Known trade-off, called out explicitly:** Windows has no equivalent to launchd/systemd socket activation (§2.1.3's table already flags this — Windows Service is `SERVICE_AUTO_START` with manual socket management, not activation-on-connect). Phase 1 accepts a normally-running background service instead of activation-on-demand; this is revisited if it matters once Linux/macOS (which do have socket activation) land in Phase 3.

**Acceptance Criteria:**

- 60 FPS master rendering on Windows 10 1809+
- Web client usable, end to end, from an iPhone/Android browser on the same network
- Monomind suggestion fires correctly for a project without `.monomind/`, and stays dismissed once declined
- Embedded dashboard reflects live master state with no separate service to start
- <10ms local latency, 70% test coverage, zero crashes in a 24-hour soak test

**Effort**: 3 months, 1 engineer

---

### 7.2 Phase 2 — Networking & Persistence (Months 4-6) `[D12.2]`

**Goal**: Turn the Windows+Web pair into a real product — P2P, persistence, multi-session, collaboration — still Windows-only for the master.

**Features:**

- ✅ WebRTC P2P (rust-webrtc), STUN/TURN (coturn), hybrid mDNS + directory discovery
- ✅ Multi-session management (create/list/kill)
- ✅ SQLite persistence (sessions + scrollback), zstd compression
- ✅ Multi-client attach (collaboration), presence indicators
- ✅ winget/MSI distribution (Windows)

**Acceptance Criteria:**

- 100 concurrent sessions (tested)
- 65-80% NAT traversal success in real network conditions (verify against §3.2's cited fabricated-precision risk — measure it directly, don't assume the literature figure)
- Reconnect-after-background works reliably on iOS Safari (the accepted trade-off from §2.2 — validate the <10s target holds in practice)
- 75% test coverage

**Effort**: 3 months, 1.5 engineers

---

### 7.3 Phase 3 — Platform Expansion: Linux + macOS (Months 7-9) `[D12.3]`

**Goal**: Take the now-proven architecture to the other two master platforms. No new client work — the same web PWA already covers every device.

**Features:**

- ✅ Linux master (systemd Type=notify, socket activation)
- ✅ macOS master (launchd socket activation)
- ✅ Cross-platform CI matrix (Windows, Ubuntu, macOS × stable/beta)
- ✅ apt/rpm (Linux), Homebrew (macOS) distribution

**Acceptance Criteria:**

- Ubuntu 22.04+ / Debian 11+ / Fedora 38+ support, macOS 12+ (Monterey) support
- Feature parity with the Windows master (same protocol, same web client works unmodified against any of the three)
- 80% test coverage

**Effort**: 3 months, 1.5 engineers

---

### 7.4 Phase 4+ — Enterprise & Advanced (Months 10-14) `[D12.4]`

**Goal**: Advanced features, enterprise readiness.

**Features:**

- ✅ Splits/tabs (master-side window management)
- ✅ Plugin system (WASM-based, sandboxed)
- ✅ Advanced clipboard (bidirectional OSC 52)
- ✅ Search (regex, case-sensitive/insensitive)
- ✅ Session recording/playback
- ✅ Sixel graphics support — not tied to any specific master platform, so it wasn't gated on any earlier phase; lands here alongside the rest of the advanced-feature set
- ✅ SOC 2 Type 1 compliance
- ✅ Enterprise features (SSO, LDAP, audit logging)
- ✅ Deeper monomind: org-run scheduling from the embedded dashboard, not just status viewing

**Monetization** (Open-Core Model):

- **Free (Open Source)**: All core features (MIT/Apache-2.0 license)
- **Enterprise** ($500-2000/year per org):
  - SSO integration (SAML 2.0, OIDC)
  - Advanced audit logging (Splunk/ELK export)
  - SLA support (8×5, 24×7 options)
  - On-premise deployment consulting

**v1.0 Target**: Month 14 — four months earlier than the original native-mobile roadmap, entirely from not building and maintaining two extra client codebases.

---

### 7.5 Success Metrics Timeline `[D12.5]`

| Milestone | Target Month | GitHub Stars | Weekly Users | Revenue |
|-----------|--------------|--------------|--------------|---------|
| **Phase 1 Release** (Windows + Web) | Month 3 | 500 | 50 | $0 |
| **Phase 2 Complete** (P2P + persistence) | Month 6 | 2,000 | 200 | $0 |
| **Phase 3 Complete** (Linux + macOS) | Month 9 | 5,000 | 500 | $0 |
| **v1.0 Release** | Month 14 | 10,000 | 1,000 | $10k-50k MRR |

---

## 8. Decision Log

### 8.1 Build vs Fork Decisions `[D8]`

#### 8.1.1 Ghostty: Reference, Not Fork `[D8.2]`

**Decision**: Do NOT fork Ghostty, use as reference for rendering patterns only

**Rationale:**

- **License**: MIT (permissive, could fork legally)
- **Language**: Zig (not production-ready for mobile as of Jan 2025, no Android NDK support)
- **Architecture Mismatch**: Ghostty is single-instance local terminal, NOT a multiplexer
- **Net-New Code**: ~70% of MONOTERMINAL is networking/P2P/mobile (greenfield)
- **Maintenance**: Zig ecosystem smaller than Rust, fewer mobile developers

**What We Learned:**

- Metal/Vulkan rendering patterns (MTLDevice/VkInstance setup)
- PTY handling (posix_openpt workflow)
- VT parser architecture (state machine)

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.1.2 cmux: Not a Foundation `[D8.3]`

**Decision**: Do NOT use cmux as a foundation

**Rationale:**

- **Codebase Size**: ~5k LOC Swift/SwiftUI (too small, minimal terminal logic)
- **Architecture**: GUI wrapper around Ghostty instances, NO networking layer
- **Platform**: macOS-only (AppKit/Metal dependencies)
- **Networking**: "No formal protocol" (per findings), would need complete rewrite

**What We Learned:**

- Unix domain socket IPC patterns (AF_UNIX)
- Session persistence patterns (NSUserDefaults)
- macOS GUI integration (SwiftUI + Metal interop)

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.1.3 Rust Rewrite: Hybrid Approach `[D1.1, D8]`

**Decision**: Rust from scratch, Ghostty as reference, WezTerm as architecture study

**Alternatives Considered:**

| Option | Effort | Pros | Cons | Verdict |
|--------|--------|------|------|---------|
| **Fork Ghostty** | 6-9 months | Proven VT parser, Metal rendering | Zig mobile immaturity, 70% rewrite anyway | ❌ Rejected |
| **Fork WezTerm** | 12-18 months | Mature Rust multiplexer, wgpu rendering | Local-first (mux server is SSH-based), 60% rewrite | ❌ Rejected |
| **Fork Alacritty** | 9-12 months | Excellent GPU renderer | NO multiplexing at all, 80% rewrite | ❌ Rejected |
| **Rust from Scratch** | 9-12 months | Full control, P2P-native architecture | Slower initial MVP | ✅ **CHOSEN** |

**Rationale:**

- **Mobile First**: Rust has cargo-mobile2, Zig lacks Android NDK bindings
- **wgpu Ecosystem**: Cross-platform GPU (Metal/Vulkan/DX12) in one codebase
- **Async Networking**: tokio mature for P2P (libp2p/webrtc-rs integrate seamlessly)
- **70% Net-New Anyway**: Networking, P2P, protocol, mobile are all greenfield regardless of fork

**Trade-offs Accepted:**

- ✅ Slower MVP (3 months vs 1-2 if forked)
- ✅ Re-implement VT parser (~2k LOC, use vte crate as reference)
- ✅ Re-implement PTY abstraction (~1k LOC, use portable-pty crate)

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.1.4 PWA-Only Client — No Native Mobile Apps `[D2, D8.6]`

**Decision**: The web client (React + xterm.js PWA, §2.2) is the *only* client. No native Android app, no native iOS app, no Tauri-wrapped desktop client. A phone is just another browser.

**Rationale:**

- WebRTC's `RTCPeerConnection` and DataChannel are built into every modern mobile browser (Chrome/Android, Safari/iOS) — nothing native is required to reach the P2P transport chosen in §2.3
- Collapses three client codebases (Kotlin, Swift, Tauri/Rust) into one (React + xterm.js), already required anyway for desktop web access
- Removes the Android Play Store rejection risk entirely (the Termux precedent, §9.3 of v1.0) — there's no app to submit
- Removes the iOS App Store audio-mode-justification risk entirely — there's no native binary requesting a background-audio entitlement

**Trade-off accepted — iOS Safari backgrounding:** Safari suspends WebRTC/Web Audio when the app backgrounds or the screen locks, and a web app cannot obtain the native `AVAudioSession` background-audio privilege the (now-cancelled) native iOS client would have used. This is treated as expected behavior, not a defect: reconnect-in-under-10-seconds plus late-joiner scrollback resync (`[D1.5]`) were already required for every client, so a backgrounded-then-resumed phone session just reconnects like a network blip would. If this trade-off proves unacceptable once real users hit it, the fallback is a **thin native wrapper** (Capacitor-style WebView shell) around the *same* web UI to gain a real background service — not a return to fully separate native rendering stacks.

**Supersedes:** §8.3.1 (React Native rejection) and §8.3.2 (custom iOS keyboard extension rejection) are now moot — there is no native client to weigh them against. Left in the log, marked superseded, for historical record.

**Decision Date**: 2026-08-14  
**Confidence**: HIGH

---

#### 8.1.5 Monomind Dashboard: Embedded, Not a Separate Service `[D13]`

**Decision**: The monomind dashboard, health check, and upgrade controls (§2.4) are embedded inside the web client itself, authenticated via the same session JWT — not a standalone service on its own port.

**Rationale:** during this very project's build process, the standalone monomind dashboard failed to connect on separate, distinct occasions — a dropped auth credential at server registration, then dead foreign-server pairing logic when a port collision occurred — both filed upstream ([monoes/monomind#135](https://github.com/monoes/monomind/issues/135), [#136](https://github.com/monoes/monomind/issues/136)). Neither failure produced a visible warning; both required manually reverse-engineering two source files to diagnose. A side-channel dashboard with its own discovery, its own port, and its own credential file is a whole extra failure surface a user of MONOTERMINAL should never have to debug. Embedding it in the client that's already open and already authenticated removes that failure class by construction.

**Decision Date**: 2026-08-14  
**Confidence**: HIGH

---

### 8.2 Technology Choices

#### 8.2.1 WebRTC over libp2p `[D3, D8.5]`

**Decision**: Use WebRTC exclusively for P2P transport

**Alternatives:**

| Library | Mobile Support | Browser | Maintenance | Verdict |
|---------|----------------|---------|-------------|---------|
| **rust-libp2p** | No client to weigh — no native binary ships at all (§8.1.4) | js-libp2p (immature) | Active but mobile lags | ❌ |
| **WebRTC** | Built into every mobile browser, no SDK to ship | Native browser API | Google maintains | ✅ |

*Note (added v1.1): the original NAT-traversal percentages cited here ("65-80% WebRTC vs 60-70% libp2p") were checked against real sources and found to be fabricated precision — a real 4.4M-attempt libp2p study measured ~70% ± 7% with no statistically significant TCP-vs-QUIC difference, and no comparably rigorous WebRTC figure exists to set against it. The qualitative case below still holds; the removed column did not.*

**Rationale:**

- **No client to maintain**: with native mobile apps dropped entirely (§8.1.4), WebRTC's browser-native support means zero SDK integration work on any platform
- **Browser Native**: RTCPeerConnection built into all modern browsers
- **Proven at Scale**: Google Meet, Zoom, Discord use WebRTC
- **Cellular Tested**: real-world NAT traversal rates should be measured directly against MONOTERMINAL's own traffic in Phase 2 (§7.2 acceptance criteria) rather than assumed from literature

**Trade-offs:**

- ✅ Simpler integration (fewer bindings, smaller binaries)
- ❌ Less flexible than libp2p (no Kad DHT, must build own directory service)
- ❌ STUN/TURN infrastructure required (self-host coturn)

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.2.2 Protocol Buffers over JSON `[D4]`

**Decision**: Use Protocol Buffers (proto3) for wire protocol

**Alternatives:**

| Format | Overhead | Schema | Performance | Verdict |
|--------|----------|--------|-------------|---------|
| **JSON** | 30-50% larger | No enforcement | 100-200 MB/s | ❌ |
| **MessagePack** | 10-20% smaller than JSON | No schema | 200-400 MB/s | ❌ |
| **Protocol Buffers** | 10-20B fixed overhead | Enforced + evolution | 500-1000 MB/s | ✅ |
| **Cap'n Proto** | Zero-copy | Enforced | 800-1200 MB/s | ❌ (immature Rust) |

**Rationale:**

- **Schema Evolution**: Add fields without breaking old clients (backward/forward compat)
- **Type Safety**: Compile-time schema validation (prost codegen)
- **Performance**: 5-10x faster than JSON for terminal chunks (binary, no parsing overhead)
- **Ecosystem**: Mature Rust (prost), Kotlin (protobuf-kotlin), Swift (swift-protobuf), JS (protobufjs)

**Trade-offs:**

- ✅ 10-20 byte overhead acceptable for 4KB+ terminal chunks (<1%)
- ❌ Slightly harder to debug (binary, need `protoc --decode`)

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.2.3 SQLite over PostgreSQL `[D6]`

**Decision**: Use SQLite for all persistent storage

**Rationale:**

- **Embedded**: No separate database server process
- **Performance**: 100k INSERT/s sufficient for terminal session logging
- **Backup**: Simple file copy (`.db` file + WAL)
- **Cross-Platform**: Works identically on all OSes
- **Zero Configuration**: No connection pooling, authentication, or network config

**When PostgreSQL Would Be Better:**

- 10,000+ concurrent sessions (requires horizontal scaling)
- Multi-master replication (HA/failover)
- Complex analytics queries (OLAP workload)

**MONOTERMINAL Scope**: 1000 sessions target, single master node → SQLite sufficient

**Decision Date**: 2026-08-13  
**Confidence**: HIGH

---

#### 8.2.4 egui+wgpu for the Master's Local UI `[D1, D7]` — updated in v1.1

**Decision**: egui+wgpu remains the master's own local terminal rendering. There is no separate client-side desktop stack to choose anymore — v1.0 paired this with a Tauri client wrapper; v1.1 removes that wrapper entirely in favor of the web-only PWA client (§8.1.4), so this decision now covers the master alone.

**Rationale:**

| Component | Choice | Why |
|-----------|--------|-----|
| **Master's local terminal** | egui + wgpu | 60 FPS requirement, performance-critical, GPU rendering mandatory |
| **Every remote client (desktop or mobile)** | Web PWA (§2.2) | One codebase, no install friction, network latency dominates over render perf anyway |

**Master requires 60 FPS** (per D1.4 specs) → egui+wgpu remains the only option for the machine actually running the shell.

**Decision Date**: 2026-08-13 (updated 2026-08-14)  
**Confidence**: HIGH

---

### 8.3 Rejected Alternatives

#### 8.3.1 React Native for Mobile `[D2.1, D2.2]` — SUPERSEDED by §8.1.4

**Decision**: REJECTED *(historical — as of v1.1 there is no native mobile client at all, so this comparison no longer applies. Kept for the record.)*

**Reason** (at the time, when native was still planned):

- **Performance**: 30-40 FPS (JS bridge overhead) vs 58-60 FPS target
- **Battery**: 250-320 mAh/hour vs 180-220 mAh/hour native
- **Binary Size**: 20-28 MB (minified) vs 6-12 MB native
- **PTY Integration**: Requires native modules anyway (defeats RN purpose)

**Chosen Instead (v1.0)**: Native Kotlin (Android) + Swift (iOS) → **replaced in v1.1 by the PWA-only decision, §8.1.4**

---

#### 8.3.2 Custom Keyboard Extension (iOS) `[D2.2]` — SUPERSEDED by §8.1.4

**Decision**: REJECTED *(historical — no native iOS client exists as of v1.1. Kept for the record.)*

**Reason** (at the time):

- **Sandboxing**: Extension runs in separate process, NO network access
- **Memory**: 48 MB limit (iOS 13+), insufficient for session state
- **No PTY Access**: Cannot communicate with main app's P2P connection

**Chosen Instead (v1.0)**: Custom inputAccessoryView (Blink BKKeyboard pattern) → **replaced in v1.1 by the web client's on-screen accessory row, §2.2**

---

#### 8.3.3 Request Battery Optimization Bypass (Android) `[D2.1]` — SUPERSEDED by §8.1.4

**Decision**: REJECTED *(historical — moot as of v1.1: there is no native Android app, foreground service, or manifest to request this permission from. Kept for the record.)*

**Reason** (at the time):

- **Play Store Risk**: REQUEST_IGNORE_BATTERY_OPTIMIZATIONS flagged, high rejection rate unless justified
- **User Friction**: Manual Settings navigation required
- **Unnecessary**: Foreground Service already exempt from Doze/App Standby

**Chosen Instead (v1.0)**: Rely on Foreground Service exemptions (dataSync type) → **entire Android app removed in v1.1, §8.1.4**

---

## 9. Appendices

### 9.1 Glossary

| Term | Definition |
|------|------------|
| **PTY** | Pseudo-Terminal: Kernel device emulating a physical terminal, used for shell I/O |
| **VT100** | DEC VT100 terminal emulation standard (ANSI escape sequences for cursor control, colors, etc.) |
| **SGR** | Select Graphic Rendition: ANSI escape sequence for text styling (colors, bold, underline) |
| **OSC** | Operating System Command: ANSI sequence for terminal-specific commands (clipboard, hyperlinks, titles) |
| **WebRTC** | Web Real-Time Communication: P2P protocol for audio/video/data channels (IETF standard) |
| **STUN** | Session Traversal Utilities for NAT: Protocol for discovering public IP/port (UDP-based) |
| **TURN** | Traversal Using Relays around NAT: Relay server for P2P when direct connection fails |
| **ICE** | Interactive Connectivity Establishment: Framework combining STUN/TURN for optimal connection |
| **DTLS** | Datagram TLS: TLS for UDP (used by WebRTC DataChannel encryption) |
| **WAL** | Write-Ahead Logging: SQLite journaling mode for better concurrency |
| **zstd** | Zstandard: Facebook's compression algorithm (fast, high ratio) |
| **mDNS** | Multicast DNS: Zero-config local network discovery (Bonjour on Apple platforms) |
| **ConPTY** | Windows Console Pseudo-console: Windows 10 1809+ replacement for legacy console API |

---

### 9.2 References

**Knowledge Matrix Nodes (Primary Source):**

- D1: Master Terminal Host Architecture (6 nodes)
- D2: Client Applications (5 nodes)
- D3: P2P Networking Architecture (4 nodes)
- D4: Wire Protocol Design (4 nodes)
- D5: Security Architecture (4 nodes)
- D6: Database & State Management (3 nodes)
- D7: Cross-Platform Desktop Development (3 nodes)
- D8: Existing Project Evaluation (5 nodes)
- D9: Performance & Scalability (3 nodes)
- D10: User Experience & Features (3 nodes)
- D11: Development & Testing (3 nodes)
- D12: Phased Roadmap (3 nodes)
- D13: Monomind Deep Integration (3 nodes — added v1.1: per-session detection, embedded dashboard, health check & upgrade)

**External References:**

- POSIX PTY API: `posix_openpt(3)`, `grantpt(3)`, `ptsname(3)` man pages
- Windows ConPTY: `CreatePseudoConsole` (MSDN docs)
- WebRTC: RFC 8825 (ICE), RFC 8445 (STUN), RFC 5766 (TURN)
- Protocol Buffers: proto3 language guide (Google)
- TLS 1.3: RFC 8446
- Ed25519: RFC 8032
- SQLite: WAL mode documentation (sqlite.org)
- wgpu: wgpu.rs documentation
- Rust async: tokio.rs documentation

---

### 9.3 Risk Register

| Risk | Probability | Impact | Mitigation | Owner |
|------|-------------|--------|------------|-------|
| **Rust learning curve delays MVP** | MEDIUM (downgraded from HIGH 2026-08-15) | HIGH | Hire experienced Rust dev OR accept 2-month ramp-up in timeline. Evidence: Build success (18min), auth wiring complete. Further downgrade to LOW pending heap corruption fix. | Tech Lead |
| **wgpu rendering issues on old GPUs** | LOW | MEDIUM | Implement Cairo CPU fallback renderer (30-45 FPS acceptable) | Rendering Team |
| **Heap corruption in test suite** | HIGH (active P0 blocker as of 2026-08-16) | HIGH | rust-backend-lead P0 investigation (Mon-Thu 2026-08-16 to 2026-08-18). Shared root cause hypothesis with memory leak. Blocks Criterion #6 (70% coverage) verification. Escalate if >4 days or systemic issue. | Backend Team / rust-backend-lead |
| **Memory leak 52.1% in long-running sessions** | HIGH (active P0 blocker as of 2026-08-16) | HIGH | rust-backend-lead P0 investigation (Mon-Thu 2026-08-16 to 2026-08-18). Shared root cause hypothesis with heap corruption. Blocks Criterion #7 (24h soak test) verification. Escalate if >4 days or systemic issue. | Backend Team / rust-backend-lead |
| **Latency benchmark tooling execution risk** | MEDIUM (deferred Phase 1 carryover as of 2026-08-16) | LOW | Criterion #5 (<10ms local latency) deferred to early Phase 2 per ADR-011. qa-lead assessment: 70-80% success probability, 4-6h work. Must complete before Phase 2 P2P features begin. Fallback: manual measurement if automated benchmark fails. | Performance Team / performance-engineer |
| **P2P NAT traversal <80% success** | MEDIUM | MEDIUM | Always offer HTTPS relay fallback (master acts as WebSocket relay) | Network Team |
| **iOS Safari suspends WebRTC/Web Audio in background** | HIGH (expected, confirmed via Apple Developer Forums 2026 — not hypothetical) | MEDIUM | Accepted trade-off (§2.2, §8.1.4): fast reconnect + scrollback resync already required by the architecture. Escalate to a Capacitor-style native wrapper only if user complaints justify it. | Web Team |
| **Code signing cost exceeds budget** | LOW | LOW | Windows EV signing ($200-400/yr) is now a Phase 1 cost, not deferrable — budget it from month 1, or accept SmartScreen "unrecognized publisher" warnings for an early unsigned build. macOS notarization ($99/yr) moves to Phase 3 alongside the macOS master. | DevOps |
| **SQLite performance insufficient** | LOW | MEDIUM | Horizontal scale (multiple masters) OR migrate to PostgreSQL (Phase 4+) | Backend Team |
| **Security vulnerability (0-day)** | LOW | HIGH | Bug bounty program, regular pentesting ($5k-15k Phase 2), public disclosure policy | Security Team |

---

### 9.4 Changelog (Future SRS Updates)

| Version | Date | Changes | Author |
|---------|------|---------|--------|
| 1.0 | 2026-08-14 | Initial comprehensive SRS from Knowledge Matrix synthesis | exhaustive-srs Org |
| 1.1 | 2026-08-14 | Native Android/iOS apps removed — PWA-only client (§2.2, §8.1.4). Rollout re-sequenced: macOS + Web first (§7.1), Linux/Windows expansion moved to Phase 3 (§7.3). Monomind per-session detection, embedded dashboard, and health/upgrade promoted to first-class, Phase-1 scope (§2.4). v1.0 target moved from Month 18 to Month 14. | Product decision |
| 1.2 | 2026-08-14 | Platform order flipped: Windows ships first (§7.1), not macOS — Phase 3 now covers Linux + macOS (§7.3) instead of Linux + Windows. Windows' lack of socket activation called out explicitly. Code-signing risk mitigation reversed accordingly (§9.3). | Product decision |
| 1.3 | 2026-08-16 | Document Control issue correction: ADR-006 (Phase 1 Gate Passage) contained incorrect acceptance criteria values. Criterion #5 corrected from "<30ms LAN p95" to "<10ms local latency" (§7.1 line 1463). Criterion #6 corrected from "80%" to "70% test coverage" (§7.1 line 1463). ADR-006 mistakenly used v1.0 overall targets (§1.3 line 102) instead of Phase 1 acceptance criteria (§7.1). SRS §7.1 remains authoritative; corrective ADR filing required. | product-owner (Document Control Authority §8) |
| 1.4 | 2026-08-16 | **Phase 1 Strategy Decision**: Criterion #5 (<10ms local latency) deferred to early Phase 2 as Phase 1 carryover per product-owner approval. Rationale: P1 bugs (heap corruption + memory leak 52.1%) prioritized for quality-first path targeting 6/7 gate passage by Thursday 2026-08-18. Risk Register §9.3 updated with memory leak (NEW), heap corruption (escalated to P0/HIGH), and latency benchmark execution risk (deferred, MEDIUM/LOW). Requires ADR-011 filing. | product-owner (Document Control Authority §8) |
| 1.5 | 2026-08-16 | **ADR-010 Filed**: Phase 1 Acceptance Criteria Corrections (docs/decisions/010-phase1-criteria-corrections.md). Formalizes the corrections identified in v1.3. ADR-006 preserved as-written for audit trail; ADR-010 documents both errors (Criterion #5 latency target, Criterion #6 coverage target) with full error analysis, source identification, and corrective actions. Coordination work (task-5, task-6) updated to use corrected SRS §7.1 values. No operational impact on gate passage threshold (5/7 minimum per ADR-006 remains valid). | product-owner (Document Control Authority §8) |

---

### 9.5 Implementation Quick-Start (Windows, Phase 1)

This section exists so an engineer can start Sprint 0 from this document alone, on a fresh Windows machine, with no other context. It doesn't introduce new requirements — it just collects what §2.1, §6, and §7.1 already specify into one setup checklist.

**Prerequisites (Windows 10 1809+ or Windows 11, per §1.2's platform table):**

| Tool | Why | Get it |
|------|-----|--------|
| Rust (stable, via rustup) | Master daemon language (§2.1.1, `[D1.1]`) | `winget install Rustlang.Rustup` or rustup.rs |
| MSVC Build Tools 2022 + Windows 10/11 SDK | Required by the `rustc` MSVC toolchain and by ConPTY/DirectX headers | Visual Studio Installer → "Desktop development with C++" workload |
| `protoc` (Protocol Buffers compiler) | Codegen for the wire protocol (§3.1.1, `[D4.1]`) | `winget install protocolbuffers.protoc` or from github.com/protocolbuffers/protobuf/releases |
| Node.js LTS + npm/pnpm | Web client build (§2.2 — React 18 + Vite) | `winget install OpenJS.NodeJS.LTS` |
| Git | Version control (§6.3 — trunk-based, PR-only) | `winget install Git.Git` |
| `cargo-tarpaulin`, `cargo-fuzz`, `cargo-flamegraph` (optional, Phase 1 CI needs tarpaulin) | Coverage, fuzzing, profiling (§6.1, §6.3) | `cargo install <name>` |

**Suggested repo layout** (Cargo workspace + separate web app, matching the crate split implied by §2.1.1's LOC estimate and §2.2's separate client stack):

```
monoterminal/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── master/                # the daemon — §2.1 (PTY, session mux, wgpu render, monomind hooks)
│   ├── protocol/               # shared .proto schema + prost-generated types — §3.1.1
│   └── monomind-bridge/        # §2.4 — detection, embedded dashboard API, health/upgrade
├── proto/
│   └── envelope.proto           # copy from §3.1.1 verbatim — it's already complete
├── web/                         # React + Vite + xterm.js PWA — §2.2
│   ├── package.json
│   └── src/
├── .github/workflows/
│   ├── pr.yml                   # §6.2 — windows-2022 only in Phase 1
│   └── release.yml
└── docs/
    ├── monoterminal-srs.md      # this document
    └── decisions/                # ADRs per §6.3 — start by copying §8's entries as the first ADRs
```

**Starter `crates/master/Cargo.toml` dependencies** (versions are illustrative — pin to whatever's current when Sprint 0 starts; every crate here is already named somewhere in §2–§5, this just consolidates them):

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }        # §2.1.4 — async PTY I/O, networking
wgpu = "0.20"                                           # §2.1.1 — DirectX 12 on Windows
egui = "0.28"                                           # §4.2.1 — master's local UI
rustls = "0.21"                                         # §3.2.1 — TLS 1.3 only
prost = "0.13"                                          # §3.1.1 — Protocol Buffers codegen
ed25519-dalek = "2"                                     # §3.2.2 — auth keypair
jsonwebtoken = "9"                                      # §3.2.2 — JWT issuance/verification
rusqlite = { version = "0.31", features = ["bundled"] } # §4.1 — SQLite persistence (Phase 2)
zstd = "0.13"                                           # §3.1.3 — compression
windows = { version = "0.58", features = [              # §2.1.2 — ConPTY bindings
    "Win32_System_Console", "Win32_System_Threading", "Win32_Foundation"
] }
tower = "0.4"                                            # §3.2.4 — rate limiting (token bucket)
tracing = "0.1"                                           # logging — implied by §6.3's debugging tooling

[build-dependencies]
prost-build = "0.13"                                     # compiles proto/envelope.proto at build time
```

**First-day commands:**

```powershell
# 1. Workspace + master crate
cargo new --lib crates/protocol
cargo new crates/master
cargo new --lib crates/monomind-bridge

# 2. Web client (§2.2)
npm create vite@latest web -- --template react-ts
cd web && npm install xterm xterm-addon-webgl && cd ..

# 3. Wire protocol codegen — copy the schema from §3.1.1 into proto/envelope.proto first
cargo build -p protocol   # runs prost-build via build.rs

# 4. First implementation target per §7.1 / this document's closing note:
#    ConPTY session creation (D1.2.3) + basic session struct (D1.3)
```

**Where the supporting research lives:** this SRS was synthesized from a Knowledge Matrix at 95% completeness (46/46 nodes, §Document Control). If deeper sourcing is needed for any `[Dx.y]` claim than what's inline here, the original per-domain research files (`research-d*.json`, `research-d*-summary.md`, and the full `knowledge-matrix-monoterminal.json`) were generated alongside this document — ask whoever handed you this file for them if citations beyond what's already quoted in §8's Decision Log are needed. This document is self-sufficient for implementation; those files are only useful for auditing *why* a decision was made beyond what's already written in §8.

---

**END OF DOCUMENT**

_This Software Requirements Specification is implementation-ready and self-contained — everything needed to start Sprint 0 is in §9.5 above, with no dependency on the conversation or tooling that produced it. Every requirement traces to specific Knowledge Matrix nodes (referenced as `[Dx.y]`)._

**Next Steps:**

1. **Engineering Review**: Tech Lead reviews SRS for technical feasibility
2. **Stakeholder Approval**: Product Owner approves scope and roadmap
3. **MVP Kickoff**: Sprint 0 — environment setup per §9.5, repository structure, CI skeleton (windows-2022 runner)
4. **First Sprint**: Implement the Windows ConPTY manager (`[D1.2.3]`) + basic session creation (`[D1.3]`) — Phase 1's actual starting point per §7.1

**Document Version**: 1.2 (2026-08-14) — see Document Control at the top of this file for the full revision history

**Knowledge Matrix Source**: `knowledge-matrix-monoterminal.json` (95% complete, 46/46 nodes) — request this file separately if deep citation auditing is needed; it is not required to begin implementation
