# MONOTERMINAL

**Next-generation terminal emulator for the distributed computing era**

[![CI](https://github.com/monoterminal/monoterminal/workflows/Pull%20Request%20Checks/badge.svg)](https://github.com/monoterminal/monoterminal/actions)
[![codecov](https://codecov.io/gh/monoterminal/monoterminal/branch/main/graph/badge.svg)](https://codecov.io/gh/monoterminal/monoterminal)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## What is MONOTERMINAL?

MONOTERMINAL enables you to:

- **Run a single master terminal** on any platform (Windows, Linux, macOS) as a persistent daemon
- **Connect from any device** (desktop, mobile, web) to access and control terminal sessions
- **Share sessions peer-to-peer** without centralized infrastructure
- **Collaborate in real-time** with multiple users attached to the same session
- **Persist sessions across disconnections** with automatic reconnection and state recovery

Unlike traditional terminal multiplexers (tmux, screen) limited to local or SSH-based access, MONOTERMINAL provides a modern, network-transparent terminal architecture suitable for remote work, mobile development, and collaborative debugging.

## Phase 1: Windows + Web (Current)

**Status:** 🚧 In Development - Sprint 0

**Current Phase Goal:** Prove the complete architecture on Windows + Web client before platform expansion.

### Features (Phase 1)

- ✅ Master daemon — **Windows only** (ConPTY, Windows Service)
- ✅ Master's local terminal UI (egui + wgpu, DirectX 12)
- ✅ **Web client (PWA)** — desktop *and* mobile browsers
- ✅ Direct connection: WebSocket + TLS 1.3 + Ed25519/JWT auth
- ✅ Session creation, attach/detach, in-memory scrollback (10k lines)
- ✅ **Monomind integration** — per-session detection, embedded dashboard, health check

### Platform Support

| Platform | Status | Target |
|----------|--------|--------|
| **Windows** | 🚧 Phase 1 | Windows 10 1809+ (ConPTY) |
| **Web (Desktop)** | 🚧 Phase 1 | Chrome 90+, Firefox 88+, Safari 14+ |
| **Web (Mobile)** | 🚧 Phase 1 | Android Chrome, iOS Safari |
| **Linux** | 📅 Phase 3 | Ubuntu 22.04+, Debian 11+, Fedora 38+ |
| **macOS** | 📅 Phase 3 | macOS 12+ (Monterey) |

## Quick Start

### Prerequisites (Windows)

- Windows 10 1809+ or Windows 11
- Rust (stable) via [rustup](https://rustup.rs)
- MSVC Build Tools 2022 (Visual Studio Installer → "Desktop development with C++")
- Protocol Buffers compiler: `winget install protocolbuffers.protoc`
- Node.js LTS: `winget install OpenJS.NodeJS.LTS`

### Build from Source

```powershell
# Clone repository
git clone https://github.com/monoterminal/monoterminal.git
cd monoterminal

# Build protocol types
cargo build -p monoterminal-protocol

# Build master daemon
cargo build --release

# Run master daemon (generates Ed25519 identity on first run)
cargo run --bin monoterminal
# → Ed25519 keypair auto-generated at ~/.monoterminal/identity.key

# Set up web client
cd web
npm install
npm run dev
```

**Note:** On first run, the master daemon automatically generates an Ed25519 keypair for authentication. The private key is stored at `~/.monoterminal/identity.key` with `0600` permissions (owner-only access).

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for detailed setup instructions and [web/docs/AUTH_FLOW.md](web/docs/AUTH_FLOW.md) for authentication details.

## Architecture

```
┌──────────────┐         WebSocket         ┌─────────────────┐
│ Web Client   │────────(TLS 1.3)─────────►│ Master Daemon   │
│ (React+PWA)  │     Protocol Buffers      │ (Rust)          │
│              │◄──────────────────────────┤                 │
│ xterm.js     │                           │ ConPTY Manager  │
│ WebGL render │                           │ wgpu Renderer   │
└──────────────┘                           │ Session Mux     │
                                            │ SQLite Store    │
                                            └─────────────────┘
```

**Key Technologies:**

- **Master:** Rust + wgpu (DirectX 12) + egui + ConPTY (Windows)
- **Client:** React 18 + Vite + xterm.js + WebGL
- **Protocol:** WebSocket + Protocol Buffers + TLS 1.3 + zstd compression
- **Auth:** Ed25519 SSH keys + JWT tokens
- **Storage:** SQLite + zstd compression

## Documentation

- **[Software Requirements Specification](docs/monoterminal-srs.md)** — Complete technical specification
- **[Development Guide](docs/DEVELOPMENT.md)** — Setup and workflow
- **[Architecture Decision Records](docs/decisions/)** — Key technical decisions

## Development

### Running Tests

```powershell
# All tests
cargo test --all-features

# Specific crate
cargo test -p monoterminal-protocol

# With coverage
cargo tarpaulin --out Html --all-features
```

### Code Quality

```powershell
# Format
cargo fmt --all

# Lint
cargo clippy --all-features --all-targets -- -D warnings

# CI checks (locally)
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
```

## Roadmap

### Phase 1: Windows + Web (Months 1-3) — **CURRENT**

- Windows master daemon with ConPTY
- Web client (PWA) for desktop and mobile browsers
- Direct WebSocket connections with TLS 1.3
- Basic session management
- Monomind integration

### Phase 2: Collaboration & Persistence (Months 4-6)

- P2P networking (WebRTC)
- Multi-session management
- SQLite persistence
- Multi-client attach (collaboration)
- Compression (zstd)

### Phase 3: Platform Expansion (Months 7-9)

- Linux master (systemd)
- macOS master (launchd)
- Cross-platform CI matrix
- apt/rpm (Linux), Homebrew (macOS) distribution

### Phase 4: Enterprise Readiness (Months 10+)

- SSO integration
- Audit logging
- RBAC
- Split panes/tabs
- Plugin system

## Contributing

**Current Status:** Not accepting contributions yet (Sprint 0 - foundation phase)

Once Phase 1 MVP is complete, we'll open up for contributions. Stay tuned!

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Support

- **Issues:** [GitHub Issues](https://github.com/monoterminal/monoterminal/issues)
- **Discussions:** [GitHub Discussions](https://github.com/monoterminal/monoterminal/discussions)
- **Documentation:** [monoterminal-srs.md](docs/monoterminal-srs.md)

---

**Built with ❤️ for developers who live in the terminal**
