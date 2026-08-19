# MONOTERMINAL - Rust Development Environment Setup
# Purpose: Ensure Rust toolchain AND protoc are in PATH for development sessions
# Usage: . .\scripts\setup-rust-env.ps1  (note the dot-source prefix)

$cargoPath = "$env:USERPROFILE\.cargo\bin"
$protocPath = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin"

# Add both to current session PATH if not already present
$pathUpdated = $false

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:PATH = "$cargoPath;$env:PATH"
    Write-Host "[OK] Rust toolchain added to session PATH" -ForegroundColor Green
    $pathUpdated = $true
} else {
    Write-Host "[OK] Rust toolchain already available" -ForegroundColor Green
}

if (-not (Get-Command protoc -ErrorAction SilentlyContinue)) {
    $env:PATH = "$protocPath;$env:PATH"
    Write-Host "[OK] Protocol Buffers compiler (protoc) added to session PATH" -ForegroundColor Green
    $pathUpdated = $true
} else {
    Write-Host "[OK] Protocol Buffers compiler (protoc) already available" -ForegroundColor Green
}

# Verify toolchain
Write-Host "`nToolchain versions:" -ForegroundColor Cyan
& cargo --version
& rustc --version
& protoc --version
Write-Host "`n[OK] Rust development environment ready (Rust + protoc)" -ForegroundColor Green
