# MONOTERMINAL Distribution Package Design

**Document Version:** 1.0  
**Phase:** 3 Weeks 9-10  
**Status:** DESIGN  
**Owner:** devops-lead

---

## Executive Summary

This document specifies the distribution package strategy for MONOTERMINAL across Linux (.deb, .rpm) and macOS (Homebrew) platforms. Each package format provides automated installation, service configuration, and user setup while maintaining consistency with the project's architecture.

**Package Formats:**
- Debian/Ubuntu (.deb) - systemd integration
- Fedora/RHEL/Rocky (.rpm) - systemd integration
- Homebrew (macOS) - launchd integration
- Standalone tarballs - Manual installation fallback

**Key Integration Points:**
- systemd unit file (task-55): `/templates/systemd/monoterminal.service`
- launchd plist (task-56): `/templates/launchd/com.monoterminal.master.plist`
- File paths (task-53): Linux (/var/lib, /var/log, /etc), macOS (/Library)

---

## 1. Debian/Ubuntu Package (.deb)

### 1.1 Package Metadata

**Package Name:** `monoterminal`  
**Version:** `0.1.0` (from Cargo.toml)  
**Architecture:** `amd64` (x86_64)  
**Section:** `utils`  
**Priority:** `optional`  
**Maintainer:** `MONOTERMINAL Team <team@monoterminal.dev>`  
**Homepage:** `https://github.com/monoterminal/monoterminal`

### 1.2 Dependencies

**Build Dependencies (debian/control BuildDepends):**
```
debhelper-compat (= 13),
cargo,
rustc (>= 1.70),
librust-dev,
protobuf-compiler,
pkg-config,
libssl-dev
```

**Runtime Dependencies (debian/control Depends):**
```
libc6 (>= 2.34),
libssl3 | libssl1.1,
libgcc-s1,
systemd,
adduser
```

### 1.3 Directory Structure

```
packaging/debian/
├── control                 # Package metadata
├── rules                   # Build rules (dh_make template)
├── changelog               # Debian changelog format
├── copyright               # License information
├── compat                  # Debhelper compatibility level (13)
├── install                 # File installation mappings
├── dirs                    # Directories to create
├── monoterminal.service    # systemd unit file (symlink to templates/)
├── postinst                # Post-installation script
├── prerm                   # Pre-removal script
├── postrm                  # Post-removal script
└── lintian-overrides       # Lintian warning suppressions
```

### 1.4 File Installation Mappings (debian/install)

```
target/release/monoterminal-master usr/local/bin/
templates/systemd/monoterminal.service lib/systemd/system/
README.md usr/share/doc/monoterminal/
LICENSE usr/share/doc/monoterminal/
```

### 1.5 Post-Installation Script (debian/postinst)

**Purpose:** Create service user, set up directories, enable systemd service

```bash
#!/bin/bash
set -e

# Create monoterminal system user and group
if ! getent group monoterminal > /dev/null 2>&1; then
    addgroup --system monoterminal
fi

if ! getent passwd monoterminal > /dev/null 2>&1; then
    adduser --system --home /var/lib/monoterminal \
            --no-create-home --ingroup monoterminal \
            --disabled-password --shell /usr/sbin/nologin \
            --gecos "MONOTERMINAL service user" monoterminal
fi

# Create directories (systemd StateDirectory/LogsDirectory handles most)
# ConfigurationDirectory: /etc/monoterminal
# StateDirectory: /var/lib/monoterminal
# LogsDirectory: /var/log/monoterminal

# Set ownership
chown monoterminal:monoterminal /var/lib/monoterminal || true
chown monoterminal:monoterminal /var/log/monoterminal || true
chmod 750 /var/lib/monoterminal || true
chmod 750 /var/log/monoterminal || true

# Reload systemd daemon and enable service
systemctl daemon-reload || true

# Enable (but don't start automatically - user choice)
systemctl enable monoterminal.service || true

echo ""
echo "MONOTERMINAL has been installed successfully."
echo ""
echo "To start the service:"
echo "  sudo systemctl start monoterminal"
echo ""
echo "To check status:"
echo "  sudo systemctl status monoterminal"
echo ""
echo "Configuration: /etc/monoterminal/config.toml"
echo "Logs: /var/log/monoterminal/ and journalctl -u monoterminal"
echo ""

#DEBHELPER#

exit 0
```

### 1.6 Pre-Removal Script (debian/prerm)

**Purpose:** Stop service before package removal

```bash
#!/bin/bash
set -e

# Stop service if running
if systemctl is-active --quiet monoterminal.service; then
    systemctl stop monoterminal.service || true
fi

#DEBHELPER#

exit 0
```

### 1.7 Post-Removal Script (debian/postrm)

**Purpose:** Clean up on purge (not on upgrade)

```bash
#!/bin/bash
set -e

case "$1" in
    purge)
        # Remove service user and group (only on purge)
        if getent passwd monoterminal > /dev/null 2>&1; then
            deluser --quiet --system monoterminal || true
        fi
        
        if getent group monoterminal > /dev/null 2>&1; then
            delgroup --quiet --system monoterminal || true
        fi
        
        # Remove data directories (only on purge, preserve on upgrade)
        rm -rf /var/lib/monoterminal || true
        rm -rf /var/log/monoterminal || true
        rm -rf /etc/monoterminal || true
        
        # Reload systemd
        systemctl daemon-reload || true
        ;;
    
    remove|upgrade|failed-upgrade|abort-install|abort-upgrade|disappear)
        # On upgrade/remove: preserve data, user, and logs
        # Only reload systemd
        systemctl daemon-reload || true
        ;;
    
    *)
        echo "postrm called with unknown argument \`$1'" >&2
        exit 1
        ;;
esac

#DEBHELPER#

exit 0
```

### 1.8 Build Process

**Build command:**
```bash
# From project root
dpkg-buildpackage -us -uc -b

# Or with debuild (cleaner build environment)
debuild -us -uc -b

# Output: ../monoterminal_0.1.0_amd64.deb
```

**Installation:**
```bash
sudo dpkg -i monoterminal_0.1.0_amd64.deb
sudo apt-get install -f  # Resolve dependencies if needed
```

### 1.9 Lintian Compliance

**Known warnings to override (debian/lintian-overrides):**
```
# Binary built with Rust (no debug symbols in release mode)
monoterminal binary: unstripped-binary-or-object usr/local/bin/monoterminal-master

# Statically linked Rust binary
monoterminal binary: statically-linked-binary usr/local/bin/monoterminal-master
```

---

## 2. Fedora/RHEL Package (.rpm)

### 2.1 Package Metadata

**Package Name:** `monoterminal`  
**Version:** `0.1.0`  
**Release:** `1%{?dist}`  
**Architecture:** `x86_64`  
**Group:** `Applications/System`  
**License:** `MIT` (or Apache-2.0, per project LICENSE)  
**URL:** `https://github.com/monoterminal/monoterminal`  
**Summary:** `Modern terminal session management daemon`

### 2.2 RPM Spec File Structure

**File:** `packaging/rpm/monoterminal.spec`

```spec
Name:           monoterminal
Version:        0.1.0
Release:        1%{?dist}
Summary:        Modern terminal session management daemon
License:        MIT
URL:            https://github.com/monoterminal/monoterminal
Source0:        %{name}-%{version}.tar.gz

# Build requirements
BuildRequires:  cargo
BuildRequires:  rust >= 1.70
BuildRequires:  protobuf-compiler
BuildRequires:  openssl-devel
BuildRequires:  systemd-rpm-macros

# Runtime requirements
Requires:       systemd
Requires:       openssl-libs
Requires(pre):  shadow-utils

%description
MONOTERMINAL is a modern terminal session management daemon that enables
persistent terminal sessions, multi-client collaboration, and P2P networking.
Supports Windows (ConPTY), Linux (pty.rs), and macOS (util.h) backends.

%prep
%setup -q

%build
# Build release binary with Cargo
cargo build --release --workspace

%install
# Create directory structure
install -d %{buildroot}%{_bindir}
install -d %{buildroot}%{_unitdir}
install -d %{buildroot}%{_sysconfdir}/monoterminal
install -d %{buildroot}%{_sharedstatedir}/monoterminal
install -d %{buildroot}%{_localstatedir}/log/monoterminal
install -d %{buildroot}%{_docdir}/%{name}

# Install binary
install -m 0755 target/release/monoterminal-master %{buildroot}%{_bindir}/

# Install systemd unit file
install -m 0644 templates/systemd/monoterminal.service %{buildroot}%{_unitdir}/

# Install documentation
install -m 0644 README.md %{buildroot}%{_docdir}/%{name}/
install -m 0644 LICENSE %{buildroot}%{_docdir}/%{name}/

%pre
# Create monoterminal system user and group before installation
getent group monoterminal >/dev/null || groupadd -r monoterminal
getent passwd monoterminal >/dev/null || \
    useradd -r -g monoterminal -d /var/lib/monoterminal -s /sbin/nologin \
    -c "MONOTERMINAL service user" monoterminal
exit 0

%post
# Enable and start systemd service
%systemd_post monoterminal.service

# Set ownership of directories
chown monoterminal:monoterminal /var/lib/monoterminal || true
chown monoterminal:monoterminal /var/log/monoterminal || true
chmod 750 /var/lib/monoterminal || true
chmod 750 /var/log/monoterminal || true

# Print installation success message
cat <<EOF

MONOTERMINAL has been installed successfully.

To start the service:
  sudo systemctl start monoterminal

To check status:
  sudo systemctl status monoterminal

Configuration: /etc/monoterminal/config.toml
Logs: /var/log/monoterminal/ and journalctl -u monoterminal

EOF

exit 0

%preun
# Stop service before uninstallation
%systemd_preun monoterminal.service

%postun
# Reload systemd after uninstallation
%systemd_postun_with_restart monoterminal.service

# On complete removal (not upgrade), clean up user and directories
if [ $1 -eq 0 ]; then
    # Remove user and group
    userdel monoterminal 2>/dev/null || true
    groupdel monoterminal 2>/dev/null || true
    
    # Remove data directories (only on complete removal)
    rm -rf /var/lib/monoterminal || true
    rm -rf /var/log/monoterminal || true
    rm -rf /etc/monoterminal || true
fi

exit 0

%files
%license LICENSE
%doc README.md

%{_bindir}/monoterminal-master
%{_unitdir}/monoterminal.service

# Configuration directory (owned by package)
%dir %attr(0755,monoterminal,monoterminal) %{_sysconfdir}/monoterminal

# State and log directories (created by systemd, owned by service user)
%dir %attr(0750,monoterminal,monoterminal) %{_sharedstatedir}/monoterminal
%dir %attr(0750,monoterminal,monoterminal) %{_localstatedir}/log/monoterminal

%changelog
* Wed Aug 19 2026 MONOTERMINAL Team <team@monoterminal.dev> - 0.1.0-1
- Initial RPM package
- Phase 3 Linux support (Unix PTY backend)
- systemd service integration
- Basic configuration and logging
```

### 2.3 Build Process

**Build command:**
```bash
# Create source tarball
tar -czf monoterminal-0.1.0.tar.gz --transform 's,^,monoterminal-0.1.0/,' .

# Build RPM
rpmbuild -ba packaging/rpm/monoterminal.spec

# Output: ~/rpmbuild/RPMS/x86_64/monoterminal-0.1.0-1.fc39.x86_64.rpm
```

**Installation:**
```bash
sudo rpm -ivh monoterminal-0.1.0-1.fc39.x86_64.rpm

# Or with dnf (resolves dependencies)
sudo dnf install monoterminal-0.1.0-1.fc39.x86_64.rpm
```

### 2.4 RPM Scriptlet Phases

**Execution order:**
1. `%pre` - Before installation (create user)
2. Installation (copy files)
3. `%post` - After installation (enable service)
4. `%preun` - Before uninstall (stop service)
5. Uninstallation (remove files)
6. `%postun` - After uninstall (cleanup user if complete removal)

---

## 3. Homebrew Formula (macOS)

### 3.1 Formula Metadata

**Formula Name:** `monoterminal`  
**Tap:** `monoterminal/monoterminal` (custom tap) or submit to `homebrew-core`  
**Class:** `Formula`  
**Homepage:** `https://github.com/monoterminal/monoterminal`  
**License:** `MIT`

### 3.2 Formula File

**File:** `packaging/homebrew/monoterminal.rb`

```ruby
class Monoterminal < Formula
  desc "Modern terminal session management daemon"
  homepage "https://github.com/monoterminal/monoterminal"
  url "https://github.com/monoterminal/monoterminal/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"
  license "MIT"
  head "https://github.com/monoterminal/monoterminal.git", branch: "main"

  # Dependencies
  depends_on "rust" => :build
  depends_on "protobuf"

  # Service management
  service do
    run [opt_bin/"monoterminal-master", "--launchd"]
    working_dir "/Library/Application Support/MONOTERMINAL"
    environment_variables RUST_LOG: "info", TERM: "xterm-256color"
    keep_alive crashed: true, successful_exit: false
    process_type :adaptive
    error_log_path "/Library/Logs/MONOTERMINAL/stderr.log"
    log_path "/Library/Logs/MONOTERMINAL/stdout.log"
  end

  def install
    # Build release binary
    system "cargo", "build", "--release", "--workspace"
    
    # Install binary
    bin.install "target/release/monoterminal-master"
    
    # Install launchd plist (for manual service installation)
    # Homebrew's service block handles automatic installation
    # This is a backup for manual setup
    (prefix/"etc/launchd").install "templates/launchd/com.monoterminal.master.plist"
    
    # Install documentation
    doc.install "README.md", "LICENSE"
  end

  def post_install
    # Create data directory
    system "sudo", "mkdir", "-p", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "mkdir", "-p", "/Library/Logs/MONOTERMINAL"
    
    # Create service user (macOS convention: underscore prefix)
    unless system "dscl", ".", "-read", "/Users/_monoterminal", ">", "/dev/null", "2>&1"
      # Create group first
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal"
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal", "PrimaryGroupID", "299"
      system "sudo", "dscl", ".", "-create", "/Groups/_monoterminal", "RealName", "MONOTERMINAL Service Group"
      
      # Create user
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "UserShell", "/usr/bin/false"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "RealName", "MONOTERMINAL Service User"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "UniqueID", "299"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "PrimaryGroupID", "299"
      system "sudo", "dscl", ".", "-create", "/Users/_monoterminal", "NFSHomeDirectory", "/var/empty"
    end
    
    # Set ownership
    system "sudo", "chown", "-R", "_monoterminal:_monoterminal", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "chown", "-R", "_monoterminal:_monoterminal", "/Library/Logs/MONOTERMINAL"
    system "sudo", "chmod", "750", "/Library/Application Support/MONOTERMINAL"
    system "sudo", "chmod", "750", "/Library/Logs/MONOTERMINAL"
  end

  def caveats
    <<~EOS
      MONOTERMINAL has been installed successfully.
      
      To start the service:
        brew services start monoterminal
      
      Or to run manually:
        monoterminal-master --launchd
      
      Configuration: /Library/Application Support/MONOTERMINAL/config.toml
      Logs: /Library/Logs/MONOTERMINAL/
      
      The service runs as user '_monoterminal' for security isolation.
    EOS
  end

  test do
    # Basic smoke test: verify binary exists and responds to --version
    assert_match "monoterminal", shell_output("#{bin}/monoterminal-master --version")
  end
end
```

### 3.3 Installation Process

**User installation:**
```bash
# Install from custom tap
brew tap monoterminal/monoterminal
brew install monoterminal

# Or install from local formula
brew install --build-from-source ./packaging/homebrew/monoterminal.rb

# Start service
brew services start monoterminal

# Check status
brew services list | grep monoterminal
```

### 3.4 Homebrew Service Integration

**Service lifecycle:**
```bash
# Start service (creates launchd plist, loads, starts)
brew services start monoterminal

# Stop service
brew services stop monoterminal

# Restart service
brew services restart monoterminal

# Service info
brew services info monoterminal

# Manual launchd control (alternative)
sudo launchctl load /Library/LaunchDaemons/homebrew.mxcl.monoterminal.plist
sudo launchctl unload /Library/LaunchDaemons/homebrew.mxcl.monoterminal.plist
```

---

## 4. Standalone Tarball (Cross-Platform Fallback)

### 4.1 Tarball Contents

**Structure:**
```
monoterminal-0.1.0-linux-x64.tar.gz
├── bin/
│   └── monoterminal-master                 # Compiled binary
├── etc/
│   ├── systemd/
│   │   └── monoterminal.service            # systemd unit file
│   └── launchd/
│       └── com.monoterminal.master.plist   # launchd plist
├── share/
│   └── doc/
│       ├── README.md
│       ├── LICENSE
│       └── INSTALL.txt
└── install.sh                               # Installation script
```

### 4.2 Installation Script

**File:** `packaging/tarball/install.sh`

```bash
#!/bin/bash
set -e

# MONOTERMINAL Installation Script
# Detects platform and installs appropriate service management

INSTALL_DIR="/usr/local/bin"
SERVICE_USER="monoterminal"
SERVICE_GROUP="monoterminal"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

echo_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

echo_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check root privileges
if [ "$EUID" -ne 0 ]; then
    echo_error "This script must be run as root (sudo ./install.sh)"
    exit 1
fi

# Detect platform
detect_platform() {
    if [ "$(uname)" = "Linux" ]; then
        if command -v systemctl &> /dev/null; then
            echo "systemd"
        else
            echo "linux-other"
        fi
    elif [ "$(uname)" = "Darwin" ]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

PLATFORM=$(detect_platform)
echo_info "Detected platform: $PLATFORM"

# Install binary
echo_info "Installing binary to $INSTALL_DIR..."
install -m 0755 bin/monoterminal-master "$INSTALL_DIR/"
echo_info "Binary installed: $INSTALL_DIR/monoterminal-master"

# Platform-specific installation
case "$PLATFORM" in
    systemd)
        echo_info "Installing systemd service..."
        
        # Create service user
        if ! getent group "$SERVICE_GROUP" > /dev/null 2>&1; then
            groupadd --system "$SERVICE_GROUP"
            echo_info "Created group: $SERVICE_GROUP"
        fi
        
        if ! getent passwd "$SERVICE_USER" > /dev/null 2>&1; then
            useradd --system --home /var/lib/monoterminal \
                    --no-create-home --gid "$SERVICE_GROUP" \
                    --shell /sbin/nologin \
                    --comment "MONOTERMINAL service user" "$SERVICE_USER"
            echo_info "Created user: $SERVICE_USER"
        fi
        
        # Create directories
        mkdir -p /var/lib/monoterminal
        mkdir -p /var/log/monoterminal
        mkdir -p /etc/monoterminal
        
        chown "$SERVICE_USER:$SERVICE_GROUP" /var/lib/monoterminal
        chown "$SERVICE_USER:$SERVICE_GROUP" /var/log/monoterminal
        chmod 750 /var/lib/monoterminal
        chmod 750 /var/log/monoterminal
        
        # Install systemd unit file
        install -m 0644 etc/systemd/monoterminal.service /etc/systemd/system/
        systemctl daemon-reload
        systemctl enable monoterminal.service
        
        echo_info "systemd service installed and enabled"
        echo_info "Start with: systemctl start monoterminal"
        ;;
    
    macos)
        echo_info "Installing launchd service..."
        
        # Create service user (macOS convention: underscore prefix)
        if ! dscl . -read /Users/_monoterminal > /dev/null 2>&1; then
            # Create group
            dscl . -create /Groups/_monoterminal
            dscl . -create /Groups/_monoterminal PrimaryGroupID 299
            dscl . -create /Groups/_monoterminal RealName "MONOTERMINAL Service Group"
            
            # Create user
            dscl . -create /Users/_monoterminal
            dscl . -create /Users/_monoterminal UserShell /usr/bin/false
            dscl . -create /Users/_monoterminal RealName "MONOTERMINAL Service User"
            dscl . -create /Users/_monoterminal UniqueID 299
            dscl . -create /Users/_monoterminal PrimaryGroupID 299
            dscl . -create /Users/_monoterminal NFSHomeDirectory /var/empty
            
            echo_info "Created service user: _monoterminal"
        fi
        
        # Create directories
        mkdir -p "/Library/Application Support/MONOTERMINAL"
        mkdir -p "/Library/Logs/MONOTERMINAL"
        
        chown -R _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL"
        chown -R _monoterminal:_monoterminal "/Library/Logs/MONOTERMINAL"
        chmod 750 "/Library/Application Support/MONOTERMINAL"
        chmod 750 "/Library/Logs/MONOTERMINAL"
        
        # Install launchd plist
        install -m 0644 etc/launchd/com.monoterminal.master.plist /Library/LaunchDaemons/
        chown root:wheel /Library/LaunchDaemons/com.monoterminal.master.plist
        launchctl load /Library/LaunchDaemons/com.monoterminal.master.plist
        
        echo_info "launchd service installed and loaded"
        echo_info "Check status: launchctl list | grep monoterminal"
        ;;
    
    linux-other)
        echo_warn "systemd not detected - manual service setup required"
        echo_warn "Binary installed at: $INSTALL_DIR/monoterminal-master"
        echo_warn "Run manually or create init script for your system"
        ;;
    
    unknown)
        echo_error "Unknown platform - only binary installed"
        echo_error "Manual configuration required"
        exit 1
        ;;
esac

# Print installation summary
echo ""
echo_info "=========================================="
echo_info "MONOTERMINAL Installation Complete"
echo_info "=========================================="
echo_info "Binary: $INSTALL_DIR/monoterminal-master"

case "$PLATFORM" in
    systemd)
        echo_info "Service: systemctl [start|stop|status] monoterminal"
        echo_info "Config: /etc/monoterminal/config.toml"
        echo_info "Logs: /var/log/monoterminal/ and journalctl -u monoterminal"
        ;;
    macos)
        echo_info "Service: launchctl [load|unload] /Library/LaunchDaemons/com.monoterminal.master.plist"
        echo_info "Config: /Library/Application Support/MONOTERMINAL/config.toml"
        echo_info "Logs: /Library/Logs/MONOTERMINAL/"
        ;;
esac

echo_info "=========================================="
```

### 4.3 Tarball Build Process

```bash
# From project root
VERSION="0.1.0"
PLATFORM="linux-x64"  # or macos-x64, linux-arm64, etc.

# Build release binary
cargo build --release --workspace

# Create tarball structure
mkdir -p monoterminal-$VERSION-$PLATFORM/{bin,etc/{systemd,launchd},share/doc}

# Copy files
cp target/release/monoterminal-master monoterminal-$VERSION-$PLATFORM/bin/
cp templates/systemd/monoterminal.service monoterminal-$VERSION-$PLATFORM/etc/systemd/
cp templates/launchd/com.monoterminal.master.plist monoterminal-$VERSION-$PLATFORM/etc/launchd/
cp README.md LICENSE monoterminal-$VERSION-$PLATFORM/share/doc/
cp packaging/tarball/install.sh monoterminal-$VERSION-$PLATFORM/

# Make install script executable
chmod +x monoterminal-$VERSION-$PLATFORM/install.sh

# Create tarball
tar -czf monoterminal-$VERSION-$PLATFORM.tar.gz monoterminal-$VERSION-$PLATFORM/

# Cleanup
rm -rf monoterminal-$VERSION-$PLATFORM/
```

---

## 5. Build Automation & CI Integration

### 5.1 GitHub Actions Workflow

**Workflow:** `.github/workflows/release.yml` (extend existing or create new)

```yaml
name: Build Distribution Packages

on:
  release:
    types: [published]
  workflow_dispatch:

env:
  CARGO_TERM_COLOR: always

jobs:
  build-deb:
    name: Build Debian Package
    runs-on: ubuntu-22.04
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install build dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y debhelper devscripts dh-make protobuf-compiler
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Build .deb package
        run: |
          dpkg-buildpackage -us -uc -b
      
      - name: Upload .deb artifact
        uses: actions/upload-artifact@v3
        with:
          name: debian-package
          path: ../monoterminal_*.deb
  
  build-rpm:
    name: Build RPM Package
    runs-on: ubuntu-22.04
    container: fedora:latest
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install build dependencies
        run: |
          dnf install -y rpm-build rpmdevtools cargo rust protobuf-compiler openssl-devel
      
      - name: Set up RPM build tree
        run: |
          rpmdev-setuptree
          tar -czf ~/rpmbuild/SOURCES/monoterminal-0.1.0.tar.gz .
      
      - name: Build .rpm package
        run: |
          rpmbuild -ba packaging/rpm/monoterminal.spec
      
      - name: Upload .rpm artifact
        uses: actions/upload-artifact@v3
        with:
          name: rpm-package
          path: ~/rpmbuild/RPMS/x86_64/monoterminal-*.rpm
  
  build-tarball:
    name: Build Standalone Tarball
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-22.04, macos-13]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install protoc
        uses: arduino/setup-protoc@v3
      
      - name: Build release binary
        run: cargo build --release --workspace
      
      - name: Create tarball
        run: |
          VERSION="0.1.0"
          if [ "$RUNNER_OS" = "Linux" ]; then
            PLATFORM="linux-x64"
          else
            PLATFORM="macos-x64"
          fi
          
          mkdir -p monoterminal-$VERSION-$PLATFORM/{bin,etc/{systemd,launchd},share/doc}
          cp target/release/monoterminal-master monoterminal-$VERSION-$PLATFORM/bin/
          cp templates/systemd/monoterminal.service monoterminal-$VERSION-$PLATFORM/etc/systemd/
          cp templates/launchd/com.monoterminal.master.plist monoterminal-$VERSION-$PLATFORM/etc/launchd/
          cp README.md LICENSE monoterminal-$VERSION-$PLATFORM/share/doc/
          cp packaging/tarball/install.sh monoterminal-$VERSION-$PLATFORM/
          chmod +x monoterminal-$VERSION-$PLATFORM/install.sh
          
          tar -czf monoterminal-$VERSION-$PLATFORM.tar.gz monoterminal-$VERSION-$PLATFORM/
      
      - name: Upload tarball artifact
        uses: actions/upload-artifact@v3
        with:
          name: tarball-${{ matrix.os }}
          path: monoterminal-*.tar.gz
```

### 5.2 Package Signing Strategy

**Debian/Ubuntu (.deb):**
- GPG signing required for apt repository hosting
- Sign with `debsigs` or `dpkg-sig`
- Create apt repository with `reprepro` or `aptly`

```bash
# Sign .deb package
dpkg-sig -k YOUR_GPG_KEY_ID --sign builder monoterminal_0.1.0_amd64.deb

# Verify signature
dpkg-sig --verify monoterminal_0.1.0_amd64.deb
```

**Fedora/RHEL (.rpm):**
- GPG signing required for yum/dnf repository
- Configure RPM macros for automatic signing
- Create yum repository with `createrepo`

```bash
# Sign .rpm package
rpm --addsign monoterminal-0.1.0-1.fc39.x86_64.rpm

# Verify signature
rpm --checksig monoterminal-0.1.0-1.fc39.x86_64.rpm
```

**macOS (Homebrew):**
- No signing required for Homebrew tap
- For notarization (future): Apple Developer account required ($99/year per SRS §6.2)
- Phase 3: Skip notarization (Homebrew tap only)
- Phase 4: Add notarization for enterprise deployment

### 5.3 Repository Hosting

**Option A - GitHub Releases (Recommended for Phase 3):**
- Upload .deb, .rpm, and tarballs as release assets
- Users download and install manually
- No repository hosting costs
- Simple for early adopters

**Option B - Custom APT/YUM Repositories (Phase 4):**
- Host apt repository for Debian/Ubuntu
- Host yum repository for Fedora/RHEL
- Requires web hosting ($5-10/month, within budget)
- Better user experience (apt install monoterminal)

**Homebrew Tap:**
- Custom tap: `monoterminal/monoterminal`
- Create GitHub repository: `monoterminal/homebrew-monoterminal`
- Users install: `brew tap monoterminal/monoterminal && brew install monoterminal`
- No hosting costs (GitHub Pages)

---

## 6. Testing Strategy

### 6.1 Package Installation Testing

**Test Matrix:**
| Package Type | Platform | Test Environment |
|-------------|----------|------------------|
| .deb | Ubuntu 22.04 | Docker or VM |
| .deb | Debian 12 | Docker or VM |
| .rpm | Fedora 39 | Docker or VM |
| .rpm | Rocky Linux 9 | Docker or VM |
| Homebrew | macOS 13 | GitHub Actions macOS runner |
| Tarball | Ubuntu 22.04 | Docker or VM |
| Tarball | macOS 13 | GitHub Actions macOS runner |

### 6.2 Test Scenarios

**For each package type:**

1. **Fresh Installation:**
   - Install package on clean system
   - Verify binary installed at correct path
   - Verify service user created
   - Verify directories created with correct permissions
   - Verify service registered (systemd/launchd)
   - Verify service starts successfully
   - Verify service can be stopped
   - Verify logs are written

2. **Upgrade:**
   - Install version 0.1.0
   - Upgrade to version 0.2.0
   - Verify service restarts automatically
   - Verify configuration preserved
   - Verify data preserved

3. **Removal:**
   - Install package
   - Remove (not purge)
   - Verify binary removed
   - Verify service stopped
   - Verify configuration preserved
   - Verify data preserved

4. **Purge (Debian/Ubuntu):**
   - Install package
   - Purge (apt purge monoterminal)
   - Verify binary removed
   - Verify service user removed
   - Verify configuration removed
   - Verify data removed

### 6.3 Automated Testing Script

**File:** `scripts/test-packages.sh`

```bash
#!/bin/bash
# Test package installation in Docker containers

# Test .deb on Ubuntu
docker run --rm -it -v $(pwd):/build ubuntu:22.04 bash -c "
    apt-get update &&
    apt-get install -y /build/monoterminal_0.1.0_amd64.deb &&
    systemctl status monoterminal &&
    systemctl start monoterminal &&
    sleep 5 &&
    systemctl status monoterminal
"

# Test .rpm on Fedora
docker run --rm -it -v $(pwd):/build fedora:39 bash -c "
    dnf install -y /build/monoterminal-0.1.0-1.fc39.x86_64.rpm &&
    systemctl status monoterminal &&
    systemctl start monoterminal &&
    sleep 5 &&
    systemctl status monoterminal
"

# Test tarball on Ubuntu (simulates manual installation)
docker run --rm -it -v $(pwd):/build ubuntu:22.04 bash -c "
    cd /tmp &&
    tar -xzf /build/monoterminal-0.1.0-linux-x64.tar.gz &&
    cd monoterminal-0.1.0-linux-x64 &&
    ./install.sh &&
    systemctl status monoterminal &&
    systemctl start monoterminal &&
    sleep 5 &&
    systemctl status monoterminal
"
```

---

## 7. Package Size Estimates

**Binary sizes (after strip):**
- monoterminal-master: ~15-20 MB (Rust release build, statically linked)

**Package sizes:**
- .deb: ~16 MB (binary + metadata + scripts)
- .rpm: ~16 MB (binary + metadata + scriptlets)
- Tarball: ~16 MB (binary + service files + install script)
- Homebrew: Downloads source (~2 MB), builds locally

**Repository bandwidth:**
- Estimated downloads/month (Phase 3): ~100-500
- Bandwidth estimate: 1.6 GB - 8 GB/month
- GitHub Releases: Free (unlimited)
- Custom hosting: $5-10/month covers this easily

---

## 8. Documentation Requirements

### 8.1 User-Facing Documentation

**File:** `docs/installation.md` (create or update)

Sections:
1. **Prerequisites** (per platform)
2. **Debian/Ubuntu Installation**
   - Add repository (Phase 4)
   - Install via apt
   - Manual .deb installation (Phase 3)
3. **Fedora/RHEL Installation**
   - Add repository (Phase 4)
   - Install via dnf/yum
   - Manual .rpm installation (Phase 3)
4. **macOS Installation**
   - Homebrew tap installation
   - Manual tarball installation
5. **Standalone Tarball Installation**
   - Download and extract
   - Run install script
6. **Post-Installation Configuration**
   - Config file location
   - Service management commands
   - Log locations
7. **Troubleshooting**
   - Service won't start
   - Permission issues
   - Log analysis

### 8.2 Developer Documentation

**File:** `docs/packaging.md` (create)

Sections:
1. **Building Packages Locally**
2. **Package Structure Details**
3. **Modifying Maintainer Scripts**
4. **Testing Package Installations**
5. **Release Process**
6. **Signing Packages**

---

## 9. Budget Impact (SRS §6.2: $50-100/month)

**One-time costs:**
- None (using free tools and GitHub infrastructure)

**Monthly recurring costs:**
- GitHub Actions: $0 (within free tier for public repos)
- Homebrew tap hosting: $0 (GitHub Pages)
- macOS notarization: $99/year = $8.25/month (deferred to Phase 4)
- Package repository hosting (Phase 4): $5-10/month
- **Total Phase 3: $0/month**
- **Total Phase 4: $13-18/month** (within $50-100 budget)

---

## 10. Timeline

**Week 9 (Planning):**
- Day 1-2: Create package templates (debian/, rpm/, homebrew/)
- Day 3: Create tarball install script
- Day 4: Update CI workflows for package building
- Day 5: Documentation (installation.md, packaging.md)

**Week 10 (Testing & Refinement):**
- Day 1-2: Test .deb installation on Ubuntu/Debian
- Day 2-3: Test .rpm installation on Fedora/Rocky
- Day 3-4: Test Homebrew formula on macOS
- Day 4: Test tarball installation on all platforms
- Day 5: Fix issues, update documentation, final review

**Post-Week 10 (Implementation):**
- Merge packaging templates to main branch
- Enable CI package building on releases
- Create first tagged release (v0.1.0)
- Publish packages to GitHub Releases
- Update documentation with download links

---

## 11. Acceptance Criteria

✅ **Design Document:** This document (~1000 LOC) specifies all package formats  
⏳ **Package Templates:**
   - debian/ directory with all maintainer scripts
   - monoterminal.spec for RPM
   - monoterminal.rb for Homebrew
   - install.sh for tarballs

⏳ **Build Automation:**
   - CI workflow builds all package types on release
   - Packages uploaded as GitHub Release assets

⏳ **Testing Strategy:**
   - Test matrix defined (7 platform/package combinations)
   - Automated test script for Docker-based testing

⏳ **Documentation:**
   - Installation guide for end users
   - Packaging guide for developers

---

## 12. Future Enhancements (Post-Phase 3)

**Phase 4 Improvements:**
1. **Repository Hosting:**
   - Create apt repository (reprepro or aptly)
   - Create yum repository (createrepo)
   - Host on GitHub Pages or S3
   - Update installation docs with `apt install` commands

2. **macOS Notarization:**
   - Purchase Apple Developer account ($99/year)
   - Sign binaries with Developer ID
   - Notarize via Apple notary service
   - Users get "verified developer" status (no Gatekeeper warnings)

3. **Windows Distribution:**
   - winget manifest (already planned in SRS §6.2)
   - MSI installer (task-XX in backlog)
   - Code signing certificate ($200-400/year, budgeted in SRS)

4. **Multi-Architecture Builds:**
   - ARM64 packages for Linux (Raspberry Pi, ARM servers)
   - Apple Silicon (arm64) for macOS (already supported via Homebrew)
   - Cross-compilation in CI

5. **Auto-Update Mechanism:**
   - Built-in update checker (monomind health/upgrade hook)
   - Package manager integration (apt/dnf/brew handles updates)

---

## Appendix A: File Paths Summary (task-53 Integration)

| Platform | Binary | Config | Data | Logs | Service File |
|----------|--------|--------|------|------|--------------|
| Linux | /usr/local/bin/monoterminal-master | /etc/monoterminal/ | /var/lib/monoterminal | /var/log/monoterminal | /etc/systemd/system/monoterminal.service |
| macOS | /usr/local/bin/monoterminal-master | /Library/Application Support/MONOTERMINAL/ | /Library/Application Support/MONOTERMINAL | /Library/Logs/MONOTERMINAL | /Library/LaunchDaemons/com.monoterminal.master.plist |

**Service Users:**
- Linux: `monoterminal:monoterminal` (system user/group)
- macOS: `_monoterminal:_monoterminal` (underscore prefix convention)

---

## Appendix B: Dependencies Reference

**Build Dependencies:**
- Rust toolchain (>= 1.70)
- Cargo
- protobuf-compiler
- pkg-config
- libssl-dev (Linux) or openssl-devel (Fedora)

**Runtime Dependencies:**
- libc6 >= 2.34 (Debian/Ubuntu)
- glibc >= 2.34 (Fedora/RHEL)
- libssl3 or libssl1.1
- systemd (Linux)
- launchd (macOS, built-in)

---

**End of Document**

This design document provides comprehensive specifications for all distribution package formats. Implementation follows in Weeks 9-10 with package templates, CI automation, and testing validation.
