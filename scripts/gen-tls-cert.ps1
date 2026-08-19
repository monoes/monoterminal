# Generate self-signed TLS certificate for development
# Phase 1: Self-signed cert (TOFU model per SRS §3.2.1)

$CertDir = "$PSScriptRoot\..\certs"

# Create certs directory if it doesn't exist
if (-not (Test-Path $CertDir)) {
    New-Item -ItemType Directory -Path $CertDir -Force | Out-Null
    Write-Host "Created directory: $CertDir"
}

$CertPath = Join-Path $CertDir "server.crt"
$KeyPath = Join-Path $CertDir "server.key"

# Check if OpenSSL is available
if (-not (Get-Command openssl -ErrorAction SilentlyContinue)) {
    Write-Error "OpenSSL not found. Please install OpenSSL or use Git Bash which includes it."
    exit 1
}

Write-Host "Generating self-signed TLS certificate for MONOTERMINAL..."
Write-Host "Certificate: $CertPath"
Write-Host "Private Key: $KeyPath"

# Generate self-signed certificate with 2048-bit RSA key
# Valid for 365 days
# For localhost/127.0.0.1
openssl req -x509 -newkey rsa:2048 -nodes `
    -keyout $KeyPath `
    -out $CertPath `
    -days 365 `
    -subj "/C=US/ST=Local/L=Local/O=MONOTERMINAL/CN=localhost" `
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nTLS certificate generated successfully!" -ForegroundColor Green
    Write-Host "Certificate: $CertPath" -ForegroundColor Cyan
    Write-Host "Private Key: $KeyPath" -ForegroundColor Cyan
    Write-Host "`nNote: This is a self-signed certificate for development only."
    Write-Host "You will need to accept the certificate in your browser on first connection."
} else {
    Write-Error "Failed to generate TLS certificate"
    exit 1
}
