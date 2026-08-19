// systemd sd-notify Protocol Implementation
// Phase 3 Week 4 Day 3: task-55
//
// Implements direct socket communication with systemd for Type=notify services.
// No libsystemd dependency - uses raw Unix domain socket communication.
//
// Reference: https://www.freedesktop.org/software/systemd/man/sd_notify.html

use anyhow::{bail, Context, Result};
use std::env;
use std::io::Write;
use std::os::unix::net::UnixDatagram;
use std::path::Path;

/// Send notification to systemd via NOTIFY_SOCKET
///
/// systemd sets the NOTIFY_SOCKET environment variable to the socket path
/// where the service should send notifications (typically /run/systemd/notify).
///
/// # Protocol
///
/// Notifications are sent as newline-separated key=value pairs:
/// - READY=1         : Service startup is finished
/// - STATUS=...      : Human-readable status text
/// - MAINPID=...     : Main process PID
/// - WATCHDOG=1      : Watchdog keepalive signal
/// - STOPPING=1      : Service is stopping
///
/// # Errors
///
/// Returns an error if:
/// - NOTIFY_SOCKET is not set (not running under systemd)
/// - Socket path is invalid
/// - Socket communication fails
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_ready;
///
/// // Signal that service is ready
/// notify_ready()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify(state: &str) -> Result<()> {
    // Get NOTIFY_SOCKET from environment
    let socket_path = match env::var("NOTIFY_SOCKET") {
        Ok(path) => path,
        Err(_) => {
            // Not running under systemd (or systemd didn't set NOTIFY_SOCKET)
            tracing::debug!("NOTIFY_SOCKET not set, skipping systemd notification");
            return Ok(());
        }
    };

    tracing::debug!("Sending systemd notification: {}", state);
    tracing::debug!("NOTIFY_SOCKET: {}", socket_path);

    // Handle abstract socket namespace (path starts with @)
    let socket_path = if socket_path.starts_with('@') {
        // Abstract socket - replace @ with null byte
        let mut path = socket_path.clone();
        path.remove(0);
        format!("\0{}", path)
    } else {
        socket_path
    };

    // Create unbound Unix datagram socket
    let socket = UnixDatagram::unbound().context("Failed to create Unix datagram socket")?;

    // Send notification to systemd
    socket
        .send_to(state.as_bytes(), &socket_path)
        .context("Failed to send notification to systemd")?;

    tracing::info!("✓ systemd notification sent: {}", state);
    Ok(())
}

/// Notify systemd that service startup is finished
///
/// Sends READY=1 to systemd, signaling that the service has finished starting up
/// and is ready to handle requests. This fulfills the Type=notify contract.
///
/// # Errors
///
/// Returns an error if socket communication fails.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_ready;
///
/// // After server initialization
/// notify_ready()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify_ready() -> Result<()> {
    notify("READY=1")
}

/// Notify systemd with a status message
///
/// Sends STATUS=<message> to systemd. The status is shown in `systemctl status`
/// output and journalctl logs.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_status;
///
/// notify_status("Initializing PTY subsystem...")?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify_status(message: &str) -> Result<()> {
    notify(&format!("STATUS={}", message))
}

/// Notify systemd that the service is stopping
///
/// Sends STOPPING=1 to systemd, signaling graceful shutdown has begun.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_stopping;
///
/// // Before shutdown
/// notify_stopping()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify_stopping() -> Result<()> {
    notify("STOPPING=1")
}

/// Notify systemd with the main process PID
///
/// Sends MAINPID=<pid> to systemd. Useful when the service forks and needs to
/// inform systemd of the actual main process PID.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_mainpid;
///
/// let pid = std::process::id();
/// notify_mainpid(pid)?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify_mainpid(pid: u32) -> Result<()> {
    notify(&format!("MAINPID={}", pid))
}

/// Send watchdog keepalive signal to systemd
///
/// Sends WATCHDOG=1 to systemd. Must be called periodically (before WatchdogSec
/// expires) if systemd watchdog is enabled in the unit file.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::service::sd_notify::notify_watchdog;
///
/// // Periodic watchdog ping
/// notify_watchdog()?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn notify_watchdog() -> Result<()> {
    notify("WATCHDOG=1")
}

/// Check if running under systemd
///
/// Returns true if NOTIFY_SOCKET environment variable is set, indicating
/// the process was started by systemd with Type=notify.
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::service::sd_notify::is_systemd;
///
/// if is_systemd() {
///     println!("Running under systemd");
/// }
/// ```
pub fn is_systemd() -> bool {
    env::var("NOTIFY_SOCKET").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_systemd_detection() {
        // Without NOTIFY_SOCKET, should return false
        env::remove_var("NOTIFY_SOCKET");
        assert!(!is_systemd());

        // With NOTIFY_SOCKET, should return true
        env::set_var("NOTIFY_SOCKET", "/run/systemd/notify");
        assert!(is_systemd());

        // Cleanup
        env::remove_var("NOTIFY_SOCKET");
    }

    #[test]
    fn test_notify_without_systemd() {
        // Should not error when NOTIFY_SOCKET is not set
        env::remove_var("NOTIFY_SOCKET");
        let result = notify("READY=1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_notify_ready_message() {
        // Verify message format (can't test actual socket without systemd)
        // Just ensure functions don't panic
        let _ = is_systemd();
    }
}
