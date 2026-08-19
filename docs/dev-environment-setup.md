# MONOTERMINAL - Development Environment Setup

## Rust Toolchain Access (REQUIRED)

### For Human Engineers - Interactive PowerShell

If `cargo` command is not found in your PowerShell session:

```powershell
# Run this ONCE per terminal session (dot-source the script):
. .\scripts\setup-rust-env.ps1
```

This adds the Rust toolchain to your session PATH. All subsequent cargo commands will work.

### For Claude Code Agents - Automated Commands

Claude Code's PowerShell tool uses separate sessions per command. Use one of these approaches:

**Option 1 - Chain with setup script:**
```powershell
. .\scripts\setup-rust-env.ps1; cargo build
```

**Option 2 - Inline PATH:**
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build
```

**Option 3 - Direct path (simplest):**
```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" build
```

### Why This Happens

The Rust toolchain is installed correctly at `%USERPROFILE%\.cargo\bin` and is in your User PATH registry, but PowerShell sessions started before the PATH was updated don't automatically inherit it.

### Permanent Solution

**Option 1 - Session Refresh (Recommended):**
- Close ALL PowerShell/terminal windows
- Log out and log back in to Windows
- New sessions will automatically have cargo in PATH

**Option 2 - Per-Session (Fast):**
- Run the setup script in each new terminal: `. .\scripts\setup-rust-env.ps1`

**Option 3 - Profile Automation:**
Add to your PowerShell profile (`$PROFILE`):
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

## Verification

After setup, verify toolchain access:

```powershell
cargo --version  # Should show: cargo 1.97.1
rustc --version  # Should show: rustc 1.97.1
```

## Agent Environment Setup

**For Claude Code agents executing Rust work:**

1. Source the setup script at the start of your session
2. Verify cargo access before running build commands
3. Report toolchain version in your status updates

**Quick verification:**
```powershell
. .\scripts\setup-rust-env.ps1
cargo --version
```

## Troubleshooting

### "cargo: command not found"

Run the setup script:
```powershell
. .\scripts\setup-rust-env.ps1
```

### "Access Denied" or Permission Errors

Ensure you're running PowerShell as your user (not Administrator) since Rust is installed in user directory.

### Script Not Found

Ensure you're in the project root directory:
```powershell
cd C:\Users\nokho\Desktop\projects\monoterminal
. .\scripts\setup-rust-env.ps1
```

## Related

- Phase 1 Criterion #1: 60 FPS rendering (requires cargo for shader compilation)
- CI/CD pipeline: Rust toolchain in GitHub Actions (separate configuration)
- DevOps: Build toolchain setup and maintenance
