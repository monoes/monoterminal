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

#![allow(dead_code)]  // Phase 3 features not all integrated yet, cleanup tracked in task-63
// Platform support:
// - Windows: %ProgramData%, %LOCALAPPDATA%
// - Linux: /var/lib, ~/.local/share, XDG compliance, systemd
// - macOS: /Library/Application Support, ~/Library, launchd

pub mod paths;
pub mod service;

