#!/bin/bash
set -e

# MONOTERMINAL Debian Package Build Script
# Phase 3 Week 9: Distribution Package Implementation (task-65)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="0.1.0"
ARCH="amd64"

echo "========================================="
echo "MONOTERMINAL Debian Package Builder"
echo "========================================="
echo "Version: $VERSION"
echo "Architecture: $ARCH"
echo "========================================="
echo ""

# Check prerequisites
echo "[1/6] Checking prerequisites..."

if ! command -v dpkg-deb &> /dev/null; then
    echo "ERROR: dpkg-deb not found. Install with: sudo apt-get install dpkg-dev"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo not found. Install Rust toolchain first."
    exit 1
fi

if ! command -v protoc &> /dev/null; then
    echo "ERROR: protoc not found. Install with: sudo apt-get install protobuf-compiler"
    exit 1
fi

echo "✓ All prerequisites satisfied"
echo ""

# Build release binary
echo "[2/6] Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release --workspace
echo "✓ Binary built: target/release/monoterminal-master"
echo ""

# Create package directory structure
echo "[3/6] Creating package directory structure..."
PKG_DIR="$PROJECT_ROOT/packaging/build/monoterminal_${VERSION}_${ARCH}"
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/local/bin"
mkdir -p "$PKG_DIR/lib/systemd/system"
mkdir -p "$PKG_DIR/usr/share/doc/monoterminal"

echo "✓ Package directory created: $PKG_DIR"
echo ""

# Copy files
echo "[4/6] Copying package files..."

# Binary
cp target/release/monoterminal-master "$PKG_DIR/usr/local/bin/"
chmod 755 "$PKG_DIR/usr/local/bin/monoterminal-master"

# systemd unit file
cp templates/systemd/monoterminal.service "$PKG_DIR/lib/systemd/system/"
chmod 644 "$PKG_DIR/lib/systemd/system/monoterminal.service"

# Documentation
cp README.md "$PKG_DIR/usr/share/doc/monoterminal/"
cp LICENSE "$PKG_DIR/usr/share/doc/monoterminal/copyright"
chmod 644 "$PKG_DIR/usr/share/doc/monoterminal"/*

# DEBIAN control files
cp packaging/debian/control "$PKG_DIR/DEBIAN/"
cp packaging/debian/postinst "$PKG_DIR/DEBIAN/"
cp packaging/debian/prerm "$PKG_DIR/DEBIAN/"
cp packaging/debian/postrm "$PKG_DIR/DEBIAN/"
chmod 644 "$PKG_DIR/DEBIAN/control"
chmod 755 "$PKG_DIR/DEBIAN/postinst"
chmod 755 "$PKG_DIR/DEBIAN/prerm"
chmod 755 "$PKG_DIR/DEBIAN/postrm"

echo "✓ All files copied"
echo ""

# Update control file with calculated size
echo "[5/6] Updating package metadata..."
INSTALLED_SIZE=$(du -sk "$PKG_DIR" | cut -f1)
# Remove existing Installed-Size line and add new one
sed -i '/^Installed-Size:/d' "$PKG_DIR/DEBIAN/control"
echo "Installed-Size: $INSTALLED_SIZE" >> "$PKG_DIR/DEBIAN/control"

echo "✓ Package metadata updated (Installed-Size: ${INSTALLED_SIZE}KB)"
echo ""

# Build package
echo "[6/6] Building .deb package..."
dpkg-deb --build "$PKG_DIR"

# Move to output directory
mkdir -p "$PROJECT_ROOT/packaging/output"
mv "${PKG_DIR}.deb" "$PROJECT_ROOT/packaging/output/"

echo "✓ Package built successfully!"
echo ""
echo "========================================="
echo "Build Complete"
echo "========================================="
echo "Output: packaging/output/monoterminal_${VERSION}_${ARCH}.deb"
echo ""
echo "To install:"
echo "  sudo dpkg -i packaging/output/monoterminal_${VERSION}_${ARCH}.deb"
echo "  sudo apt-get install -f  # Fix dependencies if needed"
echo ""
echo "To test:"
echo "  sudo systemctl status monoterminal"
echo "  sudo systemctl start monoterminal"
echo "========================================="
