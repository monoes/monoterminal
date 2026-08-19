# Claude Code Agents - Rust Build Environment Guide

## Quick Reference for Rust-Working Agents

The Rust toolchain (cargo 1.97.1, rustc 1.97.1) and Protocol Buffers compiler (protoc 35.1) are installed and functional, but require PATH setup in each PowerShell command.

**REQUIRED:** Both `cargo` AND `protoc` must be in PATH for builds to succeed.

### Three Working Patterns

**Pattern 1 - Inline PATH (Recommended - Simplest and Most Reliable)**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo build --release
```

**Pattern 2 - Setup Script Source (Cleaner for Multiple Commands)**
```powershell
. .\scripts\rust-build-env.ps1; cargo build; cargo test; cargo clippy
```

**Pattern 3 - Direct Path (Fallback if PATH fails)**
```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" build --release
```
Note: Direct path may fail if build.rs needs protoc - use Pattern 1 or 2 for full builds.

### Why This Is Needed

Claude Code's PowerShell tool executes each command in a fresh session. Environment variables (including PATH) don't persist between tool calls.

**Two Tools Required:**
1. **cargo/rustc** - Rust compiler toolchain
2. **protoc** - Protocol Buffers compiler (required by monoterminal-protocol crate)

The user's PATH registry is configured correctly for cargo - human engineers can use `cargo` directly after sourcing the setup script once per terminal. However, protoc is in a portable winget package location and needs explicit PATH setup.

### Examples for Common Tasks

**Recommended approach - set PATH once, run multiple commands:**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo build --release
```

**Build the workspace:**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo build --workspace --release
```

**Run tests:**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo test --workspace
```

**Run benchmarks (Criterion.rs for 60 FPS validation):**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo bench
```

**Check code (fast, no full build):**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo check
```

**Clean and rebuild:**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo clean; cargo build --release
```

**Shorter version using the helper script:**
```powershell
. .\scripts\rust-build-env.ps1; cargo build --release; cargo test
```

### Bash Alternative

If using Bash tool instead of PowerShell:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
```

### Verification Before Starting Work

Always verify BOTH tools are accessible:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo --version; rustc --version; protoc --version
```

**Expected output:**
```
cargo 1.97.1 (c980f4866 2026-06-30)
rustc 1.97.1 (8bab26f4f 2026-07-14)
libprotoc 35.1
```

If any tool is missing, the build WILL fail.

### Related Documentation

- **docs/dev-environment-setup.md** - Complete guide for human engineers
- **scripts/setup-rust-env.ps1** - Interactive session setup script
- **Phase 1 Criterion #1** - 60 FPS rendering (requires cargo for shader builds)
