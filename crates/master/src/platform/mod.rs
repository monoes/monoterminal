// Platform-specific functionality
// Phase 3 Week 3: Cross-platform file paths
// Phase 3 Week 4: Service management (systemd/launchd)
//
// Provides cross-platform abstractions for:
// - Data directories (system-wide and per-user)
// - Log directories
// - Database paths
// - Service management (install/uninstall/status)
//
// Platform support:
// - Windows: %ProgramData%, %LOCALAPPDATA%
// - Linux: /var/lib, ~/.local/share, XDG compliance, systemd
// - macOS: /Library/Application Support, ~/Library, launchd

pub mod paths;
pub mod service;

pub use paths::{data_dir, log_dir, session_db_path, user_data_dir};
pub use service::{install_service, service_status, uninstall_service, ServiceStatus};
