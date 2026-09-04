# MONOTERMINAL Installation Guide

**Version:** 0.1.0  
**Phase:** 3 (Linux/macOS Support)  
**Last Updated:** 2026-08-20

This guide covers installation of MONOTERMINAL on all supported platforms:
- Ubuntu/Debian (via .deb package)
- Fedora/RHEL/Rocky (via .rpm package)
- macOS (via Homebrew)
- Standalone tarball (all platforms)

---

## Table of Contents

1. [Ubuntu/Debian Installation](#ubuntudebian-installation)
2. [Fedora/RHEL Installation](#fedorarhel-installation)
3. [macOS Installation](#macos-installation)
4. [Standalone Tarball](#standalone-tarball)
5. [Post-Installation](#post-installation)
6. [Troubleshooting](#troubleshooting)
7. [Uninstallation](#uninstallation)

---

## Ubuntu/Debian Installation

### Prerequisites

- Ubuntu 22.04 LTS or later
- Debian 11 (Bullseye) or later
- sudo privileges

### Method 1: Download and Install .deb Package

```bash
# Download the .deb package
wget https://github.com/monoterminal/monoterminal/releases/download/v0.1.0/monoterminal_0.1.0_amd64.deb

# Install the package
sudo dpkg -i monoterminal_0.1.0_amd64.deb

# Install dependencies (if needed)
sudo apt-get install -f
```

### Method 2: Build from Source

```bash
# Clone repository
git clone https://github.com/monoterminal/monoterminal.git
cd monoterminal

# Install build dependencies
sudo apt-get update
sudo apt-get install -y cargo rustc protobuf-compiler libssl-dev dpkg-dev

# Build .deb package
./packaging/build-deb.sh

# Install
sudo dpkg -i packaging/output/monoterminal_0.1.0_amd64.deb
```

### Verify Installation

```bash
# Check service status
sudo systemctl status monoterminal

# Start service
sudo systemctl start monoterminal

# Enable auto-start on boot
sudo systemctl enable monoterminal

# View logs
sudo journalctl -u monoterminal -f
```

---

## Fedora/RHEL Installation

### Prerequisites

- Fedora 38 or later
- RHEL/Rocky Linux 9 or later
- sudo privileges

### Method 1: Download and Install .rpm Package

```bash
# Download the .rpm package
wget https://github.com/monoterminal/monoterminal/releases/download/v0.1.0/monoterminal-0.1.0-1.fc39.x86_64.rpm

# Install the package
sudo dnf install monoterminal-0.1.0-1.fc39.x86_64.rpm

# Or with rpm directly
sudo rpm -ivh monoterminal-0.1.0-1.fc39.x86_64.rpm
```

### Method 2: Build from Source

```bash
# Clone repository
git clone https://github.com/monoterminal/monoterminal.git
cd monoterminal

# Install build dependencies
sudo dnf install -y cargo rust protobuf-compiler openssl-devel rpm-build rpmdevtools

# Build .rpm package
./packaging/build-rpm.sh

# Install
sudo dnf install ~/rpmbuild/RPMS/x86_64/monoterminal-0.1.0-1.*.rpm
```

### Verify Installation

```bash
# Check service status
sudo systemctl status monoterminal

# Start service
sudo systemctl start monoterminal

# Enable auto-start on boot
sudo systemctl enable monoterminal

# View logs
sudo journalctl -u monoterminal -f
```

---

## macOS Installation

### Prerequisites

- macOS 12 (Monterey) or later
- Homebrew installed
- Admin privileges

### Method 1: Homebrew (Recommended)

```bash
# Add MONOTERMINAL tap
brew tap monoterminal/monoterminal

# Install MONOTERMINAL
brew install monoterminal

# Start service
brew services start monoterminal

# Check status
brew services list | grep monoterminal
```

### Method 2: Build from Source

```bash
# Install dependencies
brew install rust protobuf

# Clone repository
git clone https://github.com/monoterminal/monoterminal.git
cd monoterminal

# Build release binary
cargo build --release --workspace

# Install manually (see standalone tarball method for full instructions)
sudo cp target/release/monoterminal-master /usr/local/bin/
```

### Verify Installation

```bash
# Check service status
sudo launchctl list | grep monoterminal

# View logs
sudo tail -f /Library/Logs/MONOTERMINAL/stdout.log
```

---

## Standalone Tarball

For distributions without .deb/.rpm support or manual installation:

### Download and Install

```bash
# Download tarball (replace PLATFORM with linux-x64 or macos-x64)
wget https://github.com/monoterminal/monoterminal/releases/download/v0.1.0/monoterminal-0.1.0-PLATFORM.tar.gz

# Extract
tar -xzf monoterminal-0.1.0-PLATFORM.tar.gz
cd monoterminal-0.1.0-PLATFORM

# Run installer (requires sudo)
sudo ./install.sh
```

The installer will:
- Detect your platform (systemd Linux, macOS, or other)
- Install binary to `/usr/local/bin`
- Create service user
- Install and enable service (systemd or launchd)
- Set up directories with correct permissions

---

## Post-Installation

### Configuration

Edit the configuration file for your platform:

**Linux:**
```bash
sudo nano /etc/monoterminal/config.toml
```

**macOS:**
```bash
sudo nano "/Library/Application Support/MONOTERMINAL/config.toml"
```

Configuration file will be created on first run. See project documentation for configuration options.

### Restart Service After Configuration Changes

**Linux:**
```bash
sudo systemctl restart monoterminal
```

**macOS:**
```bash
sudo launchctl unload /Library/LaunchDaemons/com.monoterminal.master.plist
sudo launchctl load /Library/LaunchDaemons/com.monoterminal.master.plist
```

### Web Client Access

After installation, access the web client at:
```
https://localhost:5000
```

**Note:** You'll need to accept the self-signed certificate on first access.

---

## Troubleshooting

### Service Won't Start

**Check logs:**

Linux:
```bash
sudo journalctl -u monoterminal -n 50
```

macOS:
```bash
sudo tail -n 50 /Library/Logs/MONOTERMINAL/stderr.log
```

**Common issues:**

1. **Port already in use:**
   - Check if another service is using port 5000
   - Change bind_port in configuration

2. **Permission denied:**
   - Ensure data directories exist and are owned by service user
   - Linux: `sudo chown -R monoterminal:monoterminal /var/lib/monoterminal`
   - macOS: `sudo chown -R _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL"`

3. **Binary not found:**
   - Verify binary location: `which monoterminal-master`
   - Ensure PATH includes `/usr/local/bin`

### Service User Issues

**Linux:**
```bash
# Check if user exists
getent passwd monoterminal

# Recreate if needed
sudo adduser --system --group monoterminal
```

**macOS:**
```bash
# Check if user exists
dscl . -read /Users/_monoterminal
```

### File Permission Issues

**Linux:**
```bash
# Fix ownership and permissions
sudo chown -R monoterminal:monoterminal /var/lib/monoterminal /var/log/monoterminal
sudo chmod 750 /var/lib/monoterminal /var/log/monoterminal
```

**macOS:**
```bash
# Fix ownership and permissions
sudo chown -R _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL" "/Library/Logs/MONOTERMINAL"
sudo chmod 750 "/Library/Application Support/MONOTERMINAL" "/Library/Logs/MONOTERMINAL"
```

---

## Uninstallation

### Ubuntu/Debian

**Remove package (preserve configuration and data):**
```bash
sudo apt-get remove monoterminal
```

**Purge package (remove everything):**
```bash
sudo apt-get purge monoterminal
```

### Fedora/RHEL

**Remove package:**
```bash
sudo dnf remove monoterminal
# or
sudo rpm -e monoterminal
```

**Manual cleanup (if needed):**
```bash
sudo rm -rf /var/lib/monoterminal /var/log/monoterminal /etc/monoterminal
sudo userdel monoterminal
sudo groupdel monoterminal
```

### macOS

**Homebrew:**
```bash
brew services stop monoterminal
brew uninstall monoterminal
brew untap monoterminal/monoterminal
```

**Manual:**
```bash
# Stop and unload service
sudo launchctl unload /Library/LaunchDaemons/com.monoterminal.master.plist

# Remove files
sudo rm /usr/local/bin/monoterminal-master
sudo rm /Library/LaunchDaemons/com.monoterminal.master.plist
sudo rm -rf "/Library/Application Support/MONOTERMINAL"
sudo rm -rf "/Library/Logs/MONOTERMINAL"

# Remove service user
sudo dscl . -delete /Users/_monoterminal
sudo dscl . -delete /Groups/_monoterminal
```

---

## Support

- **Documentation:** https://github.com/monoterminal/monoterminal
- **Issues:** https://github.com/monoterminal/monoterminal/issues
- **Discussions:** https://github.com/monoterminal/monoterminal/discussions

---

**Installation guide complete.** For development setup and building from source, see [CONTRIBUTING.md](../CONTRIBUTING.md).
