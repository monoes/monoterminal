<p align="center">
  <img src="assets/banner.png" alt="MONOTERMINAL" width="600" />
</p>

<div align="center">

<img src="assets/logo.png" width="160" alt="MONOTERMINAL monkey mascot"/>

# monoterminal

**One terminal daemon. Every device. No port-forwarding.**

Run a single persistent shell on any machine and reach it from a browser,
anywhere — over a direct connection on your LAN, or peer-to-peer through a
signaling relay with automatic TURN fallback when NAT gets in the way.

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-33ff99?style=flat-square)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-333?style=flat-square)](#platform-support)
[![Protocol](https://img.shields.io/badge/wire-Protobuf%20%2B%20TLS%201.3-33ff99?style=flat-square)](proto/monoterminal)

</div>

---

## What is MONOTERMINAL?

MONOTERMINAL is a **master daemon + web client** pair: the daemon owns a real
PTY session on whatever machine it runs on, and any browser — desktop or
mobile — can attach to it, type into it, split it, and watch it keep running
after the tab closes.

```
you, from any browser ──── direct WebSocket (LAN) ────► daemon on your Mac/PC/box
                     └──── WebRTC P2P via relay ────►    (same daemon, reached
                           (TURN fallback if needed)      from anywhere)
```

- 🖥️ **One daemon, many attachers** — the same named session is shared across
  every client that attaches to it; splitting a pane in one browser tab shows
  up in another
- 🌐 **Two ways in** — a direct WebSocket for same-network access, or a
  WebRTC `DataChannel` negotiated through a signaling relay for anywhere
  access, with STUN/TURN handling restrictive NATs automatically
- 🔐 **TLS 1.3 + Ed25519** — every connection is encrypted and every daemon
  has a persistent Ed25519 identity; production runs self-provision their own
  certificate on first start
- 📦 **A real system service** — install as a macOS `launchd` daemon or Linux
  `systemd` unit with one command; runs under a dedicated service account,
  survives reboots, restarts on crash
- 🧩 **Splits, panes, and scrollback** — a server-owned layout tree (not just
  one terminal per connection), with scrollback persisted to SQLite so a
  reattach doesn't start blank
- 🐒 **Monomind-aware** — per-session project health checks surface straight
  in the terminal UI

## Platform support

| Platform | PTY backend | Service install |
|---|---|---|
| **macOS** | Unix PTY (`portable-pty`) | `launchd` (`sudo monoterminal install-service`) |
| **Linux** | Unix PTY (`portable-pty`) | `systemd` (`Type=notify` readiness) |
| **Windows** | ConPTY | Windows Service |

## Quick start

### Prerequisites

- Rust (stable) via [rustup](https://rustup.rs)
- Node.js 18+ (for the web client)
- `protoc` (Protocol Buffers compiler)

### Run the daemon

```bash
git clone https://github.com/monoes/monoterminal.git
cd monoterminal

# Dev mode: skips TLS cert bootstrap and Ed25519 challenge-response,
# binds 127.0.0.1:54321. Never use this outside local testing.
cargo run -p monoterminal-master -- --dev-mode
```

On a real (non-dev) run, the daemon self-provisions a TLS certificate and an
Ed25519 identity key on first start — no manual setup required. Port 5000
was the original default; it's now **54321**, since macOS's AirPlay Receiver
silently claims 5000 on every stock Mac.

### Install as a system service (macOS/Linux)

```bash
cargo build --release -p monoterminal-master
sudo ./target/release/monoterminal install-service

# check on it
sudo launchctl list | grep monoterminal      # macOS
systemctl status monoterminal                # Linux

# remove it
sudo ./target/release/monoterminal uninstall-service
```

This copies the binary to `/usr/local/bin`, creates a dedicated `_monoterminal`
service account (macOS) with no usable home directory, and runs the daemon
under it — identity key, TLS cert, and data all live under the platform's
system data directory instead of a user's home.

### Run the web client

```bash
cd web
npm install
npm run dev          # http://127.0.0.1:3000, proxies /ws to the daemon
```

### Reach it from anywhere (WebRTC P2P)

Point the daemon at a signaling relay and it'll register itself so a browser
elsewhere can find and connect to it without any port forwarding:

```bash
cargo run -p monoterminal-master -- --relay-url wss://relay.example.com
```

Run your own relay with `crates/signaling-relay` — see its
[README](crates/signaling-relay/README.md) for OAuth login setup, Docker, and
systemd deployment, and TURN credential minting for restrictive NATs.

## Architecture

```mermaid
flowchart LR
    subgraph Browser["Web Client (React + PWA)"]
        UI["xterm.js panes"]
    end

    subgraph Daemon["Master Daemon (Rust)"]
        WS["WebSocket server<br/>TLS 1.3"]
        RTC["WebRTC DataChannel<br/>peer"]
        AUTH["Ed25519 + JWT auth"]
        SESS["Session Manager<br/>PTY · Layout · Scrollback (SQLite)"]
    end

    subgraph Relay["Signaling Relay (axum)"]
        SIG["Offer / Answer / ICE (JSON)"]
        TURN["TURN credential minting"]
    end

    UI -- "direct WebSocket, same LAN<br/>Protobuf Envelope" --> WS
    UI -- "WebRTC negotiation" --> SIG
    SIG -- "pairs peers" --> RTC
    UI == "DataChannel, once paired<br/>(STUN, TURN fallback)" ==> RTC
    WS --> AUTH
    RTC --> AUTH
    AUTH --> SESS
```

Both transports speak the exact same Protobuf `Envelope` protocol and the
exact same auth handshake — the relay only ever sees SDP/ICE JSON to pair two
peers, never terminal traffic or credentials.

**Key crates** (`crates/`):

- `master` — the daemon: PTY backends, session/layout management, TLS + auth,
  WebSocket server, WebRTC signaling client
- `protocol` — the Protobuf `Envelope` wire format shared by every transport
- `signaling-relay` — the P2P pairing/TURN-credential relay (axum), deployable
  standalone via Docker or systemd
- `monomind-bridge` — project health checks surfaced in the terminal UI

### Authentication

Every connection — direct WebSocket or P2P DataChannel — completes the same
Ed25519 challenge-response handshake before it can attach to a session. There
is no shared secret and no password: the daemon proves a client controls a
specific private key, and issues a short-lived JWT for the rest of that
connection's lifetime.

```mermaid
sequenceDiagram
    participant C as Client (browser)
    participant D as Daemon

    C->>D: ChallengeRequest
    D-->>C: ChallengeResponse (32-byte nonce, expiry)
    C->>C: sign(nonce) with Ed25519 private key
    C->>D: AuthRequest (signature, public key, nonce)
    D->>D: verify signature · derive user_id from pubkey
    D-->>C: AuthResponse (access token, refresh token)
    C->>D: AttachRequest (access token)
    D-->>C: AttachResponse (scrollback, session metadata)
    Note over C,D: Access token ~15 min · refreshed proactively.<br/>Refresh token single-use, rotates on every use.
```

The client's Ed25519 keypair is generated once and persisted locally
(IndexedDB in the browser); the daemon never sees the private key, only a
signature it can verify against the public key presented alongside it. The
derived `user_id` (`ed25519:<sha256(pubkey)[..16]>`) is a stable *continuity*
identity, not an allowlist — any client that completes the challenge
correctly gets a token, matching the trust model of "can open a socket to
this daemon" already implied by direct/local access. `--dev-mode` skips this
entirely (auto-issues tokens with no signature check) and is for local
testing only — never run it on anything reachable outside your own machine.

## Documentation

- [Architecture Decision Records](docs/decisions/) — the reasoning behind the
  transport, auth, and NAT-traversal design
- [Development Guide](docs/DEVELOPMENT.md) — full setup and workflow
- [`crates/signaling-relay/README.md`](crates/signaling-relay/README.md) —
  running your own P2P relay

## Development

```bash
# All tests
cargo test --workspace

# One crate
cargo test -p monoterminal-master

# Format + lint
cargo fmt --all
cargo clippy --all-features --all-targets -- -D warnings

# Web client
cd web && npm run type-check && npm test
```

## License

Dual-licensed under [MIT](https://opensource.org/licenses/MIT) or
[Apache 2.0](http://www.apache.org/licenses/LICENSE-2.0), at your option.

## Support

- **Issues:** [GitHub Issues](https://github.com/monoes/monoterminal/issues)

---

<p align="center"><sub>Built for developers who live in the terminal — and everywhere else they carry a browser.</sub></p>
