#!/bin/bash
set -e

# MONOTERMINAL Installation Script
# Phase 3 Week 9-10: Distribution Package Planning
# Detects platform and installs appropriate service management

INSTALL_DIR="/usr/local/bin"
SERVICE_USER="monoterminal"
SERVICE_GROUP="monoterminal"
VERSION="0.1.0"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

echo_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

# Check root privileges
if [ "$EUID" -ne 0 ]; then
    echo_error "This script must be run as root (sudo ./install.sh)"
    exit 1
fi

echo_header "MONOTERMINAL Installation v$VERSION"
echo ""

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
echo ""

# Install binary
echo_info "Installing binary to $INSTALL_DIR..."
if [ ! -f "bin/monoterminal-master" ]; then
    echo_error "Binary not found: bin/monoterminal-master"
    echo_error "Please run this script from the extracted tarball directory"
    exit 1
fi

install -m 0755 bin/monoterminal-master "$INSTALL_DIR/"
echo_info "✓ Binary installed: $INSTALL_DIR/monoterminal-master"
echo ""

# Platform-specific installation
case "$PLATFORM" in
    systemd)
        echo_header "Linux systemd Installation"
        echo ""

        # Create service user
        echo_info "Creating service user and group..."
        if ! getent group "$SERVICE_GROUP" > /dev/null 2>&1; then
            groupadd --system "$SERVICE_GROUP"
            echo_info "✓ Created group: $SERVICE_GROUP"
        else
            echo_info "✓ Group already exists: $SERVICE_GROUP"
        fi

        if ! getent passwd "$SERVICE_USER" > /dev/null 2>&1; then
            useradd --system --home /var/lib/monoterminal \
                    --no-create-home --gid "$SERVICE_GROUP" \
                    --shell /sbin/nologin \
                    --comment "MONOTERMINAL service user" "$SERVICE_USER"
            echo_info "✓ Created user: $SERVICE_USER"
        else
            echo_info "✓ User already exists: $SERVICE_USER"
        fi
        echo ""

        # Create directories
        echo_info "Creating application directories..."
        mkdir -p /var/lib/monoterminal
        mkdir -p /var/log/monoterminal
        mkdir -p /etc/monoterminal

        chown "$SERVICE_USER:$SERVICE_GROUP" /var/lib/monoterminal
        chown "$SERVICE_USER:$SERVICE_GROUP" /var/log/monoterminal
        chmod 750 /var/lib/monoterminal
        chmod 750 /var/log/monoterminal

        echo_info "✓ Data directory: /var/lib/monoterminal"
        echo_info "✓ Logs directory: /var/log/monoterminal"
        echo_info "✓ Config directory: /etc/monoterminal"
        echo ""

        # Install systemd unit file
        echo_info "Installing systemd service..."
        if [ ! -f "etc/systemd/monoterminal.service" ]; then
            echo_error "systemd unit file not found: etc/systemd/monoterminal.service"
            exit 1
        fi

        install -m 0644 etc/systemd/monoterminal.service /etc/systemd/system/
        systemctl daemon-reload
        systemctl enable monoterminal.service

        echo_info "✓ systemd service installed and enabled"
        echo ""

        echo_header "Installation Complete"
        echo ""
        echo_info "Start service:    sudo systemctl start monoterminal"
        echo_info "Check status:     sudo systemctl status monoterminal"
        echo_info "View logs:        sudo journalctl -u monoterminal -f"
        echo_info "Configuration:    /etc/monoterminal/config.toml"
        echo ""
        ;;

    macos)
        echo_header "macOS launchd Installation"
        echo ""

        # Create service user (macOS convention: underscore prefix)
        echo_info "Creating service user and group..."
        if ! dscl . -read /Users/_monoterminal > /dev/null 2>&1; then
            # Find available UID/GID (299 is common for custom services)
            UID_GID=299

            # Create group
            dscl . -create /Groups/_monoterminal
            dscl . -create /Groups/_monoterminal PrimaryGroupID $UID_GID
            dscl . -create /Groups/_monoterminal RealName "MONOTERMINAL Service Group"

            # Create user
            dscl . -create /Users/_monoterminal
            dscl . -create /Users/_monoterminal UserShell /usr/bin/false
            dscl . -create /Users/_monoterminal RealName "MONOTERMINAL Service User"
            dscl . -create /Users/_monoterminal UniqueID $UID_GID
            dscl . -create /Users/_monoterminal PrimaryGroupID $UID_GID
            dscl . -create /Users/_monoterminal NFSHomeDirectory /var/empty

            echo_info "✓ Created service user: _monoterminal"
        else
            echo_info "✓ Service user already exists: _monoterminal"
        fi
        echo ""

        # Create directories
        echo_info "Creating application directories..."
        mkdir -p "/Library/Application Support/MONOTERMINAL"
        mkdir -p "/Library/Logs/MONOTERMINAL"

        chown -R _monoterminal:_monoterminal "/Library/Application Support/MONOTERMINAL"
        chown -R _monoterminal:_monoterminal "/Library/Logs/MONOTERMINAL"
        chmod 750 "/Library/Application Support/MONOTERMINAL"
        chmod 750 "/Library/Logs/MONOTERMINAL"

        echo_info "✓ Data directory: /Library/Application Support/MONOTERMINAL"
        echo_info "✓ Logs directory: /Library/Logs/MONOTERMINAL"
        echo ""

        # Install launchd plist
        echo_info "Installing launchd service..."
        if [ ! -f "etc/launchd/com.monoterminal.master.plist" ]; then
            echo_error "launchd plist not found: etc/launchd/com.monoterminal.master.plist"
            exit 1
        fi

        install -m 0644 etc/launchd/com.monoterminal.master.plist /Library/LaunchDaemons/
        chown root:wheel /Library/LaunchDaemons/com.monoterminal.master.plist
        launchctl load /Library/LaunchDaemons/com.monoterminal.master.plist

        echo_info "✓ launchd service installed and loaded"
        echo ""

        echo_header "Installation Complete"
        echo ""
        echo_info "Check status:     sudo launchctl list | grep monoterminal"
        echo_info "View logs:        sudo tail -f /Library/Logs/MONOTERMINAL/stdout.log"
        echo_info "Configuration:    /Library/Application Support/MONOTERMINAL/config.toml"
        echo ""
        echo_info "To unload service: sudo launchctl unload /Library/LaunchDaemons/com.monoterminal.master.plist"
        echo ""
        ;;

    linux-other)
        echo_header "Manual Installation (systemd not detected)"
        echo ""
        echo_warn "systemd not detected - manual service setup required"
        echo ""
        echo_info "Binary installed at: $INSTALL_DIR/monoterminal-master"
        echo_warn "To run manually:"
        echo "  sudo $INSTALL_DIR/monoterminal-master"
        echo ""
        echo_warn "For persistent service, create an init script for your init system"
        echo_warn "(SysV init, Upstart, OpenRC, etc.)"
        echo ""
        ;;

    unknown)
        echo_error "Unknown platform - only binary installed"
        echo ""
        echo_info "Binary installed at: $INSTALL_DIR/monoterminal-master"
        echo_error "Manual configuration required for service management"
        echo ""
        exit 1
        ;;
esac

echo_header "Thank you for installing MONOTERMINAL"
echo ""
echo_info "Documentation: https://github.com/monoterminal/monoterminal"
echo_info "Report issues: https://github.com/monoterminal/monoterminal/issues"
echo ""
