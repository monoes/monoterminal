# MONOTERMINAL Development Guide

**Phase 1: Windows + Web Client**

This guide covers setting up a development environment for MONOTERMINAL on Windows 10 1809+ or Windows 11.

## Prerequisites

### Required Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| **Rust (stable)** | Master daemon language | `winget install Rustlang.Rustup` or [rustup.rs](https://rustup.rs) <br/>**⚠️ After install: Open a new terminal** to refresh PATH |
| **MSVC Build Tools 2022** | Required for Rust MSVC toolchain and ConPTY/DirectX headers | Visual Studio Installer → "Desktop development with C++" workload |
| **Windows SDK** | ConPTY and Windows APIs | Included with MSVC Build Tools |
| **Protocol Buffers Compiler** | Wire protocol codegen | `winget install protocolbuffers.protoc` or [GitHub releases](https://github.com/protocolbuffers/protobuf/releases) |
| **Node.js LTS** | Web client build | `winget install OpenJS.NodeJS.LTS` |
| **Git** | Version control | `winget install Git.Git` |

### Optional Tools (Recommended)

```powershell
# Code coverage (required for CI)
cargo install cargo-tarpaulin

# Profiling tools
cargo install cargo-flamegraph

# Fuzzing (Phase 2+)
cargo install cargo-fuzz
```

## Post-Installation Verification

After installing all prerequisites, verify your environment in a **new terminal window**:

**Required Checks:**
```powershell
# Rust toolchain
rustc --version    # Should show: 1.97.1 or newer
cargo --version    # Should show: 1.97.1 or newer

# Node.js
node --version     # Should show: v20.x or newer
npm --version      # Should show: 10.x or newer

# Protocol Buffers
protoc --version   # Should show: libprotoc 3.25.x or newer
```

**Optional (Development Tools):**
```powershell
# Coverage measurement
cargo install cargo-tarpaulin
cargo tarpaulin --version  # Should show: 0.37.x or newer

# Git (if not already installed)
git --version      # Should show: 2.x or newer
```

**If any command fails:** Open a new terminal window or manually refresh PATH:
```powershell
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
```

**First Build Verification:**
```powershell
cargo build    # Should complete without errors (may take 5-10 min first time)
```

## Quick Start

### 1. Clone and Setup

```powershell
git clone https://github.com/monoterminal/monoterminal.git
cd monoterminal

# Verify prerequisites
rustc --version
cargo --version
protoc --version
node --version
npm --version
```

### 2. Build Protocol Types

```powershell
# Build the protocol crate (runs prost-build automatically)
cargo build -p monoterminal-protocol
```

### 3. Build Master Daemon

```powershell
# Build all workspace crates
cargo build --all-features

# Run tests
cargo test --all-features

# Run the master daemon (placeholder for now)
cargo run --bin monoterminal
```

### 4. Set Up Web Client

```powershell
cd web

# Install dependencies
npm install

# Start development server
npm run dev

# In another terminal: build for production
npm run build
```

## Development Workflow

### Branch Strategy

**Trunk-based development:**
- `main` branch is protected
- All changes via Pull Request
- Require 1 approving review
- Squash merge for clean history

### Creating a Feature Branch

```powershell
git checkout main
git pull origin main
git checkout -b feature/my-feature

# Make changes...
git add .
git commit -m "feat: add new feature"
git push -u origin feature/my-feature

# Create PR on GitHub
```

### Running Checks Locally

```powershell
# Format check
cargo fmt --all -- --check

# Clippy (linter)
cargo clippy --all-features --all-targets -- -D warnings

# Tests
cargo test --all-features

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html --all-features
# Open tarpaulin-report.html in browser
```

## Definition of Done (Rust Code)

Before submitting a Pull Request for Rust code, verify:

**Required Checks:**
- [ ] `cargo check --all-features` - Compiles without errors
- [ ] `cargo fmt --all -- --check` - Code formatting passes
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` - No linter warnings
- [ ] `cargo test --all-features` - All tests pass
- [ ] Added tests for new functionality or bug fixes
- [ ] Updated relevant documentation

**Coverage (Phase 1 Gate Criteria):**
- [ ] New code maintains ≥70% test coverage (verify with `cargo tarpaulin`)

**PR Checklist:**
- [ ] PR description explains the "why" (not just "what")
- [ ] Commit messages follow conventional commits format
- [ ] No secrets, credentials, or .env files committed
- [ ] All CI checks pass

## Repository Structure

```
monoterminal/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── master/             # Master daemon (ConPTY, wgpu, networking)
│   ├── protocol/           # Protocol Buffers types
│   └── monomind-bridge/    # Monomind integration
├── proto/
│   └── envelope.proto      # Wire protocol schema
├── web/                    # React + Vite + xterm.js PWA
├── .github/workflows/
│   ├── pr.yml             # CI checks
│   └── release.yml        # Release builds
└── docs/
    ├── monoterminal-srs.md # Requirements specification
    ├── DEVELOPMENT.md      # This file
    └── decisions/          # Architecture Decision Records
```

## Debugging

### Rust Debugging (Visual Studio Code)

Install the **rust-analyzer** extension:

```powershell
code --install-extension rust-lang.rust-analyzer
```

**Launch configuration** (`.vscode/launch.json`):

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug monoterminal",
      "cargo": {
        "args": ["build", "--bin=monoterminal"],
        "filter": {
          "name": "monoterminal",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}"
    }
  ]
}
```

### Logging

Set the `RUST_LOG` environment variable:

```powershell
$env:RUST_LOG = "debug"
cargo run --bin monoterminal

# Or specific module
$env:RUST_LOG = "monoterminal_master=trace"
```

## Profiling

### CPU Profiling (cargo-flamegraph)

```powershell
cargo install cargo-flamegraph

# Generate flamegraph
cargo flamegraph --bin monoterminal

# Opens flamegraph.svg
```

### GPU Profiling (RenderDoc)

1. Download [RenderDoc](https://renderdoc.org/)
2. Launch MONOTERMINAL through RenderDoc
3. Capture frames to analyze wgpu rendering

## Testing

### Unit Tests

```powershell
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p monoterminal-protocol

# Run with output
cargo test -- --nocapture
```

### Coverage

```powershell
# Generate coverage report
cargo tarpaulin --out Html --all-features --workspace

# Target: 70% coverage (Phase 1 acceptance criteria)
```

## CI/CD

### GitHub Actions

**PR Checks** (`.github/workflows/pr.yml`):
- Runs on `windows-2022` (Phase 1)
- Checks: format, clippy, build, test, coverage
- Web client: lint, type-check, build, test

**Release Builds** (`.github/workflows/release.yml`):
- Triggered on version tags (`v*`)
- Builds Windows x64 binary
- Creates GitHub release with artifacts

### Release Process

Uses **release-please** for automation:

1. Conventional commits on `main` trigger PR creation
2. PR merging creates tag and GitHub release
3. Release workflow builds and uploads binaries

## Code Signing (Windows EV Certificate)

**Budget:** $200-400/year (approved Phase 1 line item)

**Requirement:** MUST be signed before any external distribution (alpha/beta/production)

**Flexibility:** May defer for first 4-6 weeks of internal development

### Procurement Process

1. **Choose a Certificate Authority:**
   - DigiCert (recommended)
   - Sectigo (formerly Comodo)
   - GlobalSign

2. **Order EV Code Signing Certificate:**
   - Extended Validation (EV) required for SmartScreen reputation
   - Hardware token (USB) for key storage
   - Company verification process (2-5 business days)

3. **Installation:**
   ```powershell
   # Install certificate on Windows machine
   # Certificate comes on USB hardware token
   certutil -csp "eToken Base Cryptographic Provider" -user -p [PIN] -importpfx [certificate.pfx]
   ```

4. **Signing Binaries:**
   ```powershell
   # Using signtool (included with Windows SDK)
   signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a monoterminal.exe
   ```

### CI Integration (Future)

**Placeholder in `.github/workflows/release.yml`:**

```yaml
# Commented out until certificate is procured
# - name: Sign Windows binary
#   if: runner.os == 'Windows'
#   run: |
#     signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a target/release/monoterminal.exe
#   env:
#     CERTIFICATE_PASSWORD: ${{ secrets.CERTIFICATE_PASSWORD }}
```

**Important:** Never commit certificate files or passwords to repository. Use GitHub Secrets for CI.

### Without Signing (Development Only)

If running unsigned binaries during early development:

- Windows SmartScreen will show "Unrecognized publisher" warning
- Users must click "More info" → "Run anyway"
- **This is acceptable ONLY for internal testing**
- **NEVER distribute unsigned binaries externally**

---

## Common Issues

### Protocol Buffer Compilation Fails

**Error:** `protoc` not found

**Solution:**
```powershell
winget install protocolbuffers.protoc
# Restart terminal to refresh PATH
```

### Rust Compilation Errors on Windows

**Error:** Missing Windows SDK

**Solution:**
- Install Visual Studio Build Tools 2022
- Select "Desktop development with C++" workload
- Ensure Windows 10/11 SDK is checked

### Rust Commands Not Recognized After Installation

**Error:** `rustc : The term 'rustc' is not recognized...` (or similar for `cargo`, `rustup`)

**Root Cause:** rustup adds `%USERPROFILE%\.cargo\bin` to PATH, but the current PowerShell session hasn't refreshed its environment variables.

**Solution 1 (Recommended):** Open a new PowerShell window
- The new window will automatically load the updated PATH
- This is the cleanest and most reliable solution

**Solution 2:** Manually refresh PATH in the current session:
```powershell
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
```

**Verify:**
```powershell
rustc --version   # Should show: rustc 1.97.x
cargo --version   # Should show: cargo 1.97.x
```

### cargo-tarpaulin Installation Issues

**Note:** cargo-tarpaulin has limited Windows support. For local coverage, consider:
- Running tests manually and checking coverage in IDE
- Using WSL2 for coverage reports
- Relying on CI coverage reports

## Performance Targets

**Phase 1 Acceptance Criteria:**

- 60 FPS master rendering on Windows 10 1809+
- <10ms local latency (localhost WebSocket)
- 70% test coverage
- Zero crashes in 24-hour soak test

## Support

- **Documentation:** [monoterminal-srs.md](./monoterminal-srs.md)
- **Issues:** [GitHub Issues](https://github.com/monoterminal/monoterminal/issues)
- **Discussions:** [GitHub Discussions](https://github.com/monoterminal/monoterminal/discussions)

## Next Steps

1. **Read the SRS:** [docs/monoterminal-srs.md](./monoterminal-srs.md)
2. **Implement ConPTY Manager:** See §2.1.2 in SRS
3. **Build wgpu Renderer:** See §2.1.1 and §4.2.1 in SRS
4. **Create WebSocket Server:** See §3.1.2 and §3.2.1 in SRS

**Phase 1 Goal:** Windows master + Web client, proving the architecture end-to-end.
