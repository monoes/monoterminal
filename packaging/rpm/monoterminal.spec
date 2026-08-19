Name:           monoterminal
Version:        0.1.0
Release:        1%{?dist}
Summary:        Modern terminal session management daemon

License:        MIT
URL:            https://github.com/monoterminal/monoterminal
Source0:        %{name}-%{version}.tar.gz

# Build requirements
BuildRequires:  cargo >= 1.70
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

Features:
- Persistent terminal sessions (survive disconnects)
- Multi-client collaboration (multiple users per session)
- Cross-platform PTY support (Windows ConPTY, Linux pty.rs, macOS util.h)
- WebSocket protocol with TLS 1.3 encryption
- Ed25519/JWT authentication with RBAC
- P2P WebRTC networking for direct connections
- SQLite persistence for session state
- React PWA web client with xterm.js
- Monomind integration for health monitoring

%prep
%setup -q

%build
# Build release binary with Cargo
export CARGO_HOME=$(pwd)/.cargo
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
- systemd service integration (Type=notify)
- Cross-platform builds (Windows, Linux, macOS)
- WebSocket protocol with TLS 1.3
- Ed25519/JWT authentication with RBAC
- React PWA web client with xterm.js
- Monomind integration
