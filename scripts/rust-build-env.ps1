# MONOTERMINAL - Inline Rust Build Environment Setup
# Purpose: One-liner to set up PATH for Rust builds in Claude Code agent sessions
# Usage: Copy the $env:PATH line into your PowerShell commands

# For Claude Code agents - use this inline pattern:
# $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"; cargo build

# Or source this file to set up the environment:
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin;$env:PATH"

Write-Host "[OK] Rust build environment configured (cargo + protoc in PATH)" -ForegroundColor Green
