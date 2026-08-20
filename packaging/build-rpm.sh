#!/bin/bash
set -e

# MONOTERMINAL RPM Package Build Script
# Phase 3 Week 10: Distribution Package Implementation (task-65)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="0.1.0"
RELEASE="1"

echo "========================================="
echo "MONOTERMINAL RPM Package Builder"
echo "========================================="
echo "Version: $VERSION-$RELEASE"
echo "========================================="
echo ""

# Check prerequisites
echo "[1/5] Checking prerequisites..."

if ! command -v rpmbuild &> /dev/null; then
    echo "ERROR: rpmbuild not found. Install with: sudo dnf install rpm-build rpmdevtools"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo not found. Install Rust toolchain first."
    exit 1
fi

if ! command -v protoc &> /dev/null; then
    echo "ERROR: protoc not found. Install with: sudo dnf install protobuf-compiler"
    exit 1
fi

echo "✓ All prerequisites satisfied"
echo ""

# Set up RPM build tree
echo "[2/5] Setting up RPM build tree..."
rpmdev-setuptree 2>/dev/null || mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Create source tarball
echo "[3/5] Creating source tarball..."
cd "$PROJECT_ROOT"
TAR_NAME="monoterminal-${VERSION}.tar.gz"
tar --transform "s,^,monoterminal-${VERSION}/," \
    --exclude='.git' \
    --exclude='target' \
    --exclude='node_modules' \
    --exclude='packaging/build' \
    --exclude='packaging/output' \
    -czf ~/rpmbuild/SOURCES/"$TAR_NAME" .

echo "✓ Source tarball created: ~/rpmbuild/SOURCES/$TAR_NAME"
echo ""

# Copy spec file
echo "[4/5] Copying spec file..."
cp packaging/rpm/monoterminal.spec ~/rpmbuild/SPECS/
echo "✓ Spec file copied to ~/rpmbuild/SPECS/"
echo ""

# Build RPM
echo "[5/5] Building RPM package..."
rpmbuild -ba ~/rpmbuild/SPECS/monoterminal.spec

# Copy to output directory
mkdir -p "$PROJECT_ROOT/packaging/output"
cp ~/rpmbuild/RPMS/x86_64/monoterminal-${VERSION}-${RELEASE}.*.rpm "$PROJECT_ROOT/packaging/output/" 2>/dev/null || \
   echo "Note: RPM package location may vary by distribution"

echo ""
echo "========================================="
echo "Build Complete"
echo "========================================="
echo "Output: ~/rpmbuild/RPMS/x86_64/monoterminal-${VERSION}-${RELEASE}.*.rpm"
echo "Copied to: packaging/output/ (if available)"
echo ""
echo "To install:"
echo "  sudo rpm -ivh ~/rpmbuild/RPMS/x86_64/monoterminal-${VERSION}-${RELEASE}.*.rpm"
echo "  # or"
echo "  sudo dnf install ~/rpmbuild/RPMS/x86_64/monoterminal-${VERSION}-${RELEASE}.*.rpm"
echo ""
echo "To test:"
echo "  sudo systemctl status monoterminal"
echo "  sudo systemctl start monoterminal"
echo "========================================="
