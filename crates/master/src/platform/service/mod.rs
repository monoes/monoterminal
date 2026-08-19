// Service Management Module
// Phase 3 Week 4: systemd/launchd service installation and lifecycle management
//
// Provides unified API for system service management across platforms:
// - Linux: systemd
// - macOS: launchd
// - Windows: Windows Service Control Manager (already implemented Phase 1)
//
// Based on architecture design from task-54

#[cfg(target_os = "linux")]
pub mod systemd;

#[cfg(target_os = "macos")]
pub mod launchd;

use anyhow::{bail, Result};
use std::fmt;

/// Service manager type detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceManager {
    /// Linux systemd
    Systemd,
    /// macOS launchd
    Launchd,
    /// Windows Service Control Manager (Phase 1)
    WindowsSCM,
}

impl fmt::Display for ServiceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceManager::Systemd => write!(f, "systemd"),
            ServiceManager::Launchd => write!(f, "launchd"),
            ServiceManager::WindowsSCM => write!(f, "Windows SCM"),
        }
    }
}

/// Service status information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    /// Is the service installed?
    pub installed: bool,
    /// Is the service currently running?
    pub running: bool,
    /// Is the service enabled (auto-start on boot)?
    pub enabled: bool,
    /// Process ID (if running)
    pub pid: Option<u32>,
    /// Additional status message
    pub message: String,
}

/// Detect the service manager for the current platform
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::service::detect_service_manager;
///
/// let manager = detect_service_manager()?;
/// println!("Service manager: {}", manager);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn detect_service_manager() -> Result<ServiceManager> {
    #[cfg(target_os = "linux")]
    {
        // Check if systemd is running (PID 1 or /run/systemd/system exists)
        if std::path::Path::new("/run/systemd/system").exists() {
            Ok(ServiceManager::Systemd)
        } else {
            bail!("systemd not detected (required for Linux service management)")
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS always uses launchd (since Mac OS X 10.4)
        Ok(ServiceManager::Launchd)
    }

    #[cfg(windows)]
    {
        // Windows Service Control Manager (already implemented Phase 1)
        Ok(ServiceManager::WindowsSCM)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        bail!("Unsupported platform for service management")
    }
}

/// Check if running as root/administrator
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::is_root;
///
/// if !is_root() {
///     eprintln!("This command requires root privileges");
///     std::process::exit(1);
/// }
/// ```
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        // Check effective user ID (euid)
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    {
        // Windows privilege checking (Phase 1 already implements this)
        // Placeholder: assume true for now
        true
    }
}

/// Require root privileges, bail if not root
///
/// # Errors
///
/// Returns an error if not running as root.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::require_root;
///
/// require_root()?;
/// // Continues only if root
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn require_root() -> Result<()> {
    if !is_root() {
        bail!("This command requires root privileges. Run with sudo.");
    }
    Ok(())
}

/// Install system service
///
/// Platform-specific installation:
/// - Linux: Install systemd unit file, create service user, enable and start
/// - macOS: Install launchd plist, create service user, load service
/// - Windows: Already implemented in Phase 1
///
/// # Errors
///
/// Returns an error if installation fails or not running as root.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::install_service;
///
/// install_service()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn install_service() -> Result<()> {
    require_root()?;

    let manager = detect_service_manager()?;

    match manager {
        #[cfg(target_os = "linux")]
        ServiceManager::Systemd => systemd::install_service(),

        #[cfg(target_os = "macos")]
        ServiceManager::Launchd => launchd::install_service(),

        #[cfg(windows)]
        ServiceManager::WindowsSCM => {
            bail!("Windows service installation already implemented in Phase 1")
        }

        #[allow(unreachable_patterns)]
        _ => bail!("Service installation not implemented for {}", manager),
    }
}

/// Uninstall system service
///
/// # Errors
///
/// Returns an error if uninstallation fails or not running as root.
pub fn uninstall_service() -> Result<()> {
    require_root()?;

    let manager = detect_service_manager()?;

    match manager {
        #[cfg(target_os = "linux")]
        ServiceManager::Systemd => systemd::uninstall_service(),

        #[cfg(target_os = "macos")]
        ServiceManager::Launchd => launchd::uninstall_service(),

        #[cfg(windows)]
        ServiceManager::WindowsSCM => {
            bail!("Windows service uninstallation already implemented in Phase 1")
        }

        #[allow(unreachable_patterns)]
        _ => bail!("Service uninstallation not implemented for {}", manager),
    }
}

/// Get service status
///
/// # Errors
///
/// Returns an error if status check fails.
pub fn service_status() -> Result<ServiceStatus> {
    let manager = detect_service_manager()?;

    match manager {
        #[cfg(target_os = "linux")]
        ServiceManager::Systemd => systemd::service_status(),

        #[cfg(target_os = "macos")]
        ServiceManager::Launchd => launchd::service_status(),

        #[cfg(windows)]
        ServiceManager::WindowsSCM => {
            bail!("Windows service status already implemented in Phase 1")
        }

        #[allow(unreachable_patterns)]
        _ => bail!("Service status not implemented for {}", manager),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_service_manager() {
        let manager = detect_service_manager().unwrap();

        #[cfg(target_os = "linux")]
        assert_eq!(manager, ServiceManager::Systemd);

        #[cfg(target_os = "macos")]
        assert_eq!(manager, ServiceManager::Launchd);

        #[cfg(windows)]
        assert_eq!(manager, ServiceManager::WindowsSCM);
    }

    #[test]
    fn test_service_manager_display() {
        assert_eq!(ServiceManager::Systemd.to_string(), "systemd");
        assert_eq!(ServiceManager::Launchd.to_string(), "launchd");
        assert_eq!(ServiceManager::WindowsSCM.to_string(), "Windows SCM");
    }

    #[test]
    fn test_is_root() {
        // Can't reliably test this without actual root privileges
        // Just verify it doesn't panic
        let _ = is_root();
    }
}
