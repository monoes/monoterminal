# MONOTERMINAL Distribution Packages

Phase 3 Weeks 9-10: Distribution package templates, build scripts, and testing infrastructure.

## Contents

- `debian/` - Debian/Ubuntu package files (.deb)
- `rpm/` - Fedora/RHEL package spec (.rpm)
- `homebrew/` - macOS Homebrew formula
- `tarball/` - Standalone tarball installer
- Build scripts (`.sh`)
- Output directory (created during build)

## Quick Start

### Build Debian Package

```bash
./build-deb.sh
# Output: packaging/output/monoterminal_0.1.0_amd64.deb
```

### Build RPM Package

```bash
./build-rpm.sh
# Output: ~/rpmbuild/RPMS/x86_64/monoterminal-0.1.0-1.*.rpm
```

### Test Packages

```bash
# Requires Docker
./test-packages.sh
```

## Package Formats

### Debian (.deb)

- **Platforms:** Ubuntu 22.04+, Debian 11+
- **Service:** systemd
- **Paths:** `/var/lib/monoterminal`, `/var/log/monoterminal`, `/etc/monoterminal`
- **User:** `monoterminal:monoterminal`

### RPM (.rpm)

- **Platforms:** Fedora 38+, RHEL 9+, Rocky Linux 9+
- **Service:** systemd
- **Paths:** `/var/lib/monoterminal`, `/var/log/monoterminal`, `/etc/monoterminal`
- **User:** `monoterminal:monoterminal`

### Homebrew

- **Platform:** macOS 12+
- **Service:** launchd
- **Paths:** `/Library/Application Support/MONOTERMINAL`, `/Library/Logs/MONOTERMINAL`
- **User:** `_monoterminal:_monoterminal`

## Testing

### Docker-based Testing

The `test-packages.sh` script tests packages in Docker containers:

- Ubuntu 22.04 (.deb)
- Debian 12 (.deb)
- Fedora 39 (.rpm)

Tests verify:
- Package installation succeeds
- Binary installed to correct location
- systemd service file installed
- Service user created
- Directories created with correct ownership

### Manual Testing

```bash
# Ubuntu/Debian
sudo dpkg -i packaging/output/monoterminal_0.1.0_amd64.deb
sudo systemctl start monoterminal
sudo systemctl status monoterminal

# Fedora/RHEL
sudo dnf install ~/rpmbuild/RPMS/x86_64/monoterminal-0.1.0-1.*.rpm
sudo systemctl start monoterminal
sudo systemctl status monoterminal

# macOS
brew install --build-from-source packaging/homebrew/monoterminal.rb
brew services start monoterminal
```

## Build Requirements

### Debian/Ubuntu

```bash
sudo apt-get install -y cargo rustc protobuf-compiler libssl-dev dpkg-dev
```

### Fedora/RHEL

```bash
sudo dnf install -y cargo rust protobuf-compiler openssl-devel rpm-build rpmdevtools
```

### macOS

```bash
brew install rust protobuf
```

## Documentation

- **Design:** `../docs/distribution-package-design.md` (~1,100 LOC)
- **Installation Guide:** `../docs/installation.md`
- **Task Planning:** task-58 (Week 9)
- **Implementation:** task-65 (Week 10)

## File Structure

```
packaging/
├── debian/                      # Debian package files
│   ├── control                  # Package metadata
│   ├── postinst                 # Post-install script
│   ├── prerm                    # Pre-removal script
│   ├── postrm                   # Post-removal script
│   ├── rules                    # Build rules
│   ├── changelog                # Version history
│   ├── compat                   # Debhelper compat level
│   └── copyright                # License
├── rpm/
│   └── monoterminal.spec        # RPM spec file
├── homebrew/
│   └── monoterminal.rb          # Homebrew formula
├── tarball/
│   └── install.sh               # Standalone installer
├── build-deb.sh                 # Debian build script
├── build-rpm.sh                 # RPM build script
├── test-packages.sh             # Docker-based tests
├── build/                       # Temporary build files (gitignored)
└── output/                      # Built packages (gitignored)
```

## CI Integration

Build automation via GitHub Actions (`.github/workflows/release.yml`):

- Builds .deb on Ubuntu 22.04 runner
- Builds .rpm on Fedora container
- Builds tarballs for Linux and macOS
- Uploads packages to GitHub Releases

## Maintenance

### Updating Version

1. Update `Cargo.toml` version
2. Update `packaging/debian/changelog`
3. Update `packaging/rpm/monoterminal.spec` changelog
4. Update version in build scripts
5. Rebuild packages

### Adding Dependencies

1. Update `debian/control` (Depends field)
2. Update `rpm/monoterminal.spec` (Requires field)
3. Update `homebrew/monoterminal.rb` (depends_on)
4. Update installation guide

## Support

- **Package Issues:** https://github.com/monoterminal/monoterminal/issues
- **Design Document:** `../docs/distribution-package-design.md`
- **Installation Guide:** `../docs/installation.md`
