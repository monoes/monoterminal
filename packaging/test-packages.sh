#!/bin/bash
set -e

# MONOTERMINAL Package Testing Script
# Phase 3 Week 10: Distribution Package Implementation (task-65)
# Tests .deb and .rpm packages in Docker containers

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="0.1.0"

echo "========================================="
echo "MONOTERMINAL Package Testing Suite"
echo "========================================="
echo "Testing .deb and .rpm packages in Docker"
echo "========================================="
echo ""

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    echo "ERROR: Docker not found. Please install Docker to run package tests."
    exit 1
fi

# Ensure packages are built
if [ ! -f "$PROJECT_ROOT/packaging/output/monoterminal_${VERSION}_amd64.deb" ]; then
    echo "ERROR: .deb package not found. Build it first with ./build-deb.sh"
    exit 1
fi

echo "✓ Prerequisites satisfied"
echo ""

# Test Debian package on Ubuntu 22.04
echo "========================================="
echo "Test 1: Debian Package on Ubuntu 22.04"
echo "========================================="
echo ""

docker run --rm -v "$PROJECT_ROOT:/monoterminal" ubuntu:22.04 bash -c "
    set -e
    echo '[1/5] Updating package lists...'
    apt-get update -qq

    echo '[2/5] Installing .deb package...'
    apt-get install -y /monoterminal/packaging/output/monoterminal_${VERSION}_amd64.deb || {
        echo 'Installation failed (expected: dependency issues)'
        echo '[3/5] Installing dependencies...'
        apt-get install -f -y
    }

    echo '[4/5] Verifying installation...'
    if [ -f /usr/local/bin/monoterminal-master ]; then
        echo '✓ Binary installed: /usr/local/bin/monoterminal-master'
    else
        echo '✗ Binary NOT found'
        exit 1
    fi

    if [ -f /lib/systemd/system/monoterminal.service ]; then
        echo '✓ systemd service installed'
    else
        echo '✗ systemd service NOT found'
        exit 1
    fi

    if getent passwd monoterminal > /dev/null 2>&1; then
        echo '✓ Service user created'
    else
        echo '✗ Service user NOT created'
        exit 1
    fi

    echo '[5/5] Checking directories...'
    ls -ld /var/lib/monoterminal /var/log/monoterminal /etc/monoterminal 2>/dev/null || echo 'Note: Some directories may be created on first start'

    echo ''
    echo '✓ Ubuntu 22.04 test PASSED'
"

echo ""
echo "========================================="
echo "Test 2: Debian Package on Debian 12"
echo "========================================="
echo ""

docker run --rm -v "$PROJECT_ROOT:/monoterminal" debian:12 bash -c "
    set -e
    echo '[1/5] Updating package lists...'
    apt-get update -qq

    echo '[2/5] Installing .deb package...'
    apt-get install -y /monoterminal/packaging/output/monoterminal_${VERSION}_amd64.deb || {
        echo '[3/5] Installing dependencies...'
        apt-get install -f -y
    }

    echo '[4/5] Verifying installation...'
    if [ -f /usr/local/bin/monoterminal-master ]; then
        echo '✓ Binary installed'
    else
        echo '✗ Binary NOT found'
        exit 1
    fi

    if [ -f /lib/systemd/system/monoterminal.service ]; then
        echo '✓ systemd service installed'
    else
        echo '✗ systemd service NOT found'
        exit 1
    fi

    echo ''
    echo '✓ Debian 12 test PASSED'
"

# Test RPM package if it exists
if [ -f ~/rpmbuild/RPMS/x86_64/monoterminal-${VERSION}-*.rpm ]; then
    echo ""
    echo "========================================="
    echo "Test 3: RPM Package on Fedora 39"
    echo "========================================="
    echo ""

    docker run --rm -v ~/rpmbuild/RPMS/x86_64:/packages fedora:39 bash -c "
        set -e
        echo '[1/4] Installing .rpm package...'
        dnf install -y /packages/monoterminal-${VERSION}-*.rpm

        echo '[2/4] Verifying installation...'
        if [ -f /usr/bin/monoterminal-master ]; then
            echo '✓ Binary installed'
        else
            echo '✗ Binary NOT found'
            exit 1
        fi

        if [ -f /usr/lib/systemd/system/monoterminal.service ]; then
            echo '✓ systemd service installed'
        else
            echo '✗ systemd service NOT found'
            exit 1
        fi

        if getent passwd monoterminal > /dev/null 2>&1; then
            echo '✓ Service user created'
        else
            echo '✗ Service user NOT created'
            exit 1
        fi

        echo ''
        echo '✓ Fedora 39 test PASSED'
    "
else
    echo ""
    echo "========================================="
    echo "Test 3: RPM Package - SKIPPED"
    echo "========================================="
    echo "RPM package not found. Build it first with ./build-rpm.sh"
fi

echo ""
echo "========================================="
echo "Test Suite Complete"
echo "========================================="
echo "✓ All tested packages passed validation"
echo ""
echo "Next steps:"
echo "  1. Test service startup (requires systemd-enabled container or VM)"
echo "  2. Test upgrade path (install old version, upgrade to new)"
echo "  3. Test removal (dpkg -r / rpm -e)"
echo "  4. Test purge (apt purge / dnf remove)"
echo "========================================="
