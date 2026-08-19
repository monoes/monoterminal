// systemd Service Management Implementation (Linux)
// Phase 3 Week 4: task-55
//
// Implements service installation, uninstallation, and status checking for Linux systemd.
// Based on architecture design from task-54.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::ServiceStatus;
use crate::platform::paths::{data_dir, log_dir};

/// Service name (systemd unit name without .service suffix)
const SERVICE_NAME: &str = "monoterminal";

/// Service user name
const SERVICE_USER: &str = "monoterminal";

/// Service group name
const SERVICE_GROUP: &str = "monoterminal";

/// Binary installation path
const BINARY_PATH: &str = "/usr/local/bin/monoterminal-master";

/// systemd unit file path
const UNIT_FILE_PATH: &str = "/etc/systemd/system/monoterminal.service";

/// Install monoterminal as a systemd service
///
/// Steps:
/// 1. Check if already installed
/// 2. Copy binary to /usr/local/bin/
/// 3. Create service user and group
/// 4. Create data and log directories
/// 5. Install systemd unit file
/// 6. Reload systemd daemon
/// 7. Enable service (auto-start on boot)
/// 8. Start service
///
/// # Errors
///
/// Returns an error if any installation step fails.
pub fn install_service() -> Result<()> {
    tracing::info!("Installing MONOTERMINAL as systemd service...");

    // 1. Check if already installed
    if Path::new(UNIT_FILE_PATH).exists() {
        bail!(
            "Service already installed at {}. Uninstall first or use --force.",
            UNIT_FILE_PATH
        );
    }

    // 2. Copy binary to system location
    install_binary()?;

    // 3. Create service user and group
    create_service_user()?;

    // 4. Create directories with correct permissions
    create_directories()?;

    // 5. Install unit file
    install_unit_file()?;

    // 6. Reload systemd
    reload_systemd()?;

    // 7. Enable service (auto-start on boot)
    enable_service()?;

    // 8. Start service
    start_service()?;

    tracing::info!("✓ MONOTERMINAL service installed successfully");
    println!("");
    println!("MONOTERMINAL service installed successfully!");
    println!("");
    println!("Service status:");
    println!("  sudo systemctl status {}", SERVICE_NAME);
    println!("");
    println!("View logs:");
    println!("  sudo journalctl -u {} -f", SERVICE_NAME);
    println!("");
    println!("To uninstall:");
    println!("  sudo monoterminal-master uninstall-service");

    Ok(())
}

/// Uninstall monoterminal systemd service
///
/// Steps:
/// 1. Stop service
/// 2. Disable service
/// 3. Remove unit file
/// 4. Reload systemd daemon
/// 5. Remove binary
/// 6. (Optional) Remove data directories
/// 7. (Optional) Remove service user
pub fn uninstall_service() -> Result<()> {
    tracing::info!("Uninstalling MONOTERMINAL systemd service...");

    // 1. Stop service (if running)
    let _ = stop_service(); // Ignore errors if not running

    // 2. Disable service (if enabled)
    let _ = disable_service(); // Ignore errors if not enabled

    // 3. Remove unit file
    if Path::new(UNIT_FILE_PATH).exists() {
        fs::remove_file(UNIT_FILE_PATH).context("Failed to remove unit file")?;
        tracing::info!("Removed unit file: {}", UNIT_FILE_PATH);
    }

    // 4. Reload systemd
    reload_systemd()?;

    // 5. Remove binary
    if Path::new(BINARY_PATH).exists() {
        fs::remove_file(BINARY_PATH).context("Failed to remove binary")?;
        tracing::info!("Removed binary: {}", BINARY_PATH);
    }

    // 6. Prompt for data directory removal
    // TODO: Interactive prompt (for now, keep data)
    tracing::info!("Data directory preserved: {}", data_dir().display());
    tracing::info!("Log directory preserved: {}", log_dir().display());

    // 7. Prompt for user removal
    // TODO: Interactive prompt (for now, keep user)
    tracing::info!("Service user preserved: {}", SERVICE_USER);

    tracing::info!("✓ MONOTERMINAL service uninstalled successfully");
    println!("");
    println!("MONOTERMINAL service uninstalled successfully!");
    println!("");
    println!("Data directory preserved: {}", data_dir().display());
    println!("To remove data: sudo rm -rf {}", data_dir().display());
    println!("");
    println!("Service user preserved: {}", SERVICE_USER);
    println!("To remove user: sudo userdel {}", SERVICE_USER);

    Ok(())
}

/// Get systemd service status
pub fn service_status() -> Result<ServiceStatus> {
    let output = Command::new("systemctl")
        .args(&["is-active", SERVICE_NAME])
        .output()
        .context("Failed to check service status")?;

    let running = output.status.success();

    let output_enabled = Command::new("systemctl")
        .args(&["is-enabled", SERVICE_NAME])
        .output()
        .context("Failed to check service enabled status")?;

    let enabled = output_enabled.status.success();

    let installed = Path::new(UNIT_FILE_PATH).exists();

    // Get PID if running
    let pid = if running {
        get_service_pid().ok()
    } else {
        None
    };

    let message = if running {
        format!(
            "Service is running (PID: {})",
            pid.map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )
    } else if installed {
        "Service is installed but not running".to_string()
    } else {
        "Service is not installed".to_string()
    };

    Ok(ServiceStatus {
        installed,
        running,
        enabled,
        pid,
        message,
    })
}

/// Install binary to system location
fn install_binary() -> Result<()> {
    // Get current executable path
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;

    tracing::info!(
        "Copying binary: {} -> {}",
        current_exe.display(),
        BINARY_PATH
    );

    // Copy binary
    fs::copy(&current_exe, BINARY_PATH).context("Failed to copy binary to /usr/local/bin")?;

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(BINARY_PATH, permissions)
            .context("Failed to set binary permissions")?;
    }

    tracing::info!("✓ Binary installed: {}", BINARY_PATH);
    Ok(())
}

/// Create service user and group
fn create_service_user() -> Result<()> {
    tracing::info!("Creating service user: {}", SERVICE_USER);

    // Check if user already exists
    let check = Command::new("id").arg(SERVICE_USER).output();

    if check.is_ok() && check.unwrap().status.success() {
        tracing::info!("Service user already exists: {}", SERVICE_USER);
        return Ok(());
    }

    // Create system user with useradd
    let status = Command::new("useradd")
        .args(&[
            "--system",         // System user (UID < 1000)
            "--no-create-home", // No home directory
            "--shell",
            "/usr/sbin/nologin", // No login shell
            "--comment",
            "MONOTERMINAL master daemon",
            SERVICE_USER,
        ])
        .status()
        .context("Failed to create service user")?;

    if !status.success() {
        bail!("useradd failed with exit code: {:?}", status.code());
    }

    tracing::info!("✓ Service user created: {}", SERVICE_USER);
    Ok(())
}

/// Create data and log directories with correct ownership
fn create_directories() -> Result<()> {
    let data_dir_path = data_dir();
    let log_dir_path = log_dir();

    tracing::info!("Creating data directory: {}", data_dir_path.display());
    tracing::info!("Creating log directory: {}", log_dir_path.display());

    // Create directories
    fs::create_dir_all(&data_dir_path).context("Failed to create data directory")?;
    fs::create_dir_all(&log_dir_path).context("Failed to create log directory")?;

    // Set ownership to service user
    set_directory_owner(&data_dir_path, SERVICE_USER, SERVICE_GROUP)?;
    set_directory_owner(&log_dir_path, SERVICE_USER, SERVICE_GROUP)?;

    // Set permissions (0755)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&data_dir_path, permissions)
            .context("Failed to set data directory permissions")?;
        fs::set_permissions(&log_dir_path, permissions)
            .context("Failed to set log directory permissions")?;
    }

    tracing::info!("✓ Directories created with correct ownership");
    Ok(())
}

/// Set directory owner using chown
fn set_directory_owner(path: &Path, user: &str, group: &str) -> Result<()> {
    let owner = format!("{}:{}", user, group);

    let status = Command::new("chown")
        .args(&["-R", &owner, path.to_str().unwrap()])
        .status()
        .context("Failed to set directory ownership")?;

    if !status.success() {
        bail!("chown failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Install systemd unit file
fn install_unit_file() -> Result<()> {
    tracing::info!("Installing systemd unit file: {}", UNIT_FILE_PATH);

    // Generate unit file content
    let unit_content = generate_unit_file();

    // Write unit file
    let mut file = fs::File::create(UNIT_FILE_PATH).context("Failed to create unit file")?;
    file.write_all(unit_content.as_bytes())
        .context("Failed to write unit file")?;

    // Set permissions (0644)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o644);
        fs::set_permissions(UNIT_FILE_PATH, permissions)
            .context("Failed to set unit file permissions")?;
    }

    tracing::info!("✓ Unit file installed: {}", UNIT_FILE_PATH);
    Ok(())
}

/// Generate systemd unit file content
///
/// Uses template from templates/systemd/monoterminal.service with dynamic paths
fn generate_unit_file() -> String {
    // For now, use inline template
    // TODO: Load from templates/systemd/monoterminal.service in production

    let data_dir_path = data_dir();
    let log_dir_path = log_dir();

    format!(
        r#"[Unit]
Description=MONOTERMINAL Master Daemon
Documentation=https://github.com/monoterminal/monoterminal
After=network.target

[Service]
Type=notify
ExecStart={binary}
 --systemd
Restart=on-failure
RestartSec=5s

User={user}
Group={group}

WorkingDirectory={working_dir}
StateDirectory=monoterminal
LogsDirectory=monoterminal
ConfigurationDirectory=monoterminal

Environment="RUST_LOG=info"
Environment="TERM=xterm-256color"

StandardOutput=journal
StandardError=journal
SyslogIdentifier=monoterminal

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths={data_dir} {log_dir}

LimitNOFILE=65536
LimitNPROC=512

NotifyAccess=main
TimeoutStartSec=60s
TimeoutStopSec=30s

[Install]
WantedBy=multi-user.target
"#,
        binary = BINARY_PATH,
        user = SERVICE_USER,
        group = SERVICE_GROUP,
        working_dir = data_dir_path.display(),
        data_dir = data_dir_path.display(),
        log_dir = log_dir_path.display(),
    )
}

/// Reload systemd daemon
fn reload_systemd() -> Result<()> {
    tracing::info!("Reloading systemd daemon...");

    let status = Command::new("systemctl")
        .arg("daemon-reload")
        .status()
        .context("Failed to reload systemd daemon")?;

    if !status.success() {
        bail!("systemctl daemon-reload failed");
    }

    tracing::info!("✓ systemd daemon reloaded");
    Ok(())
}

/// Enable service (auto-start on boot)
fn enable_service() -> Result<()> {
    tracing::info!("Enabling service (auto-start on boot)...");

    let status = Command::new("systemctl")
        .args(&["enable", SERVICE_NAME])
        .status()
        .context("Failed to enable service")?;

    if !status.success() {
        bail!("systemctl enable failed");
    }

    tracing::info!("✓ Service enabled");
    Ok(())
}

/// Start service
fn start_service() -> Result<()> {
    tracing::info!("Starting service...");

    let status = Command::new("systemctl")
        .args(&["start", SERVICE_NAME])
        .status()
        .context("Failed to start service")?;

    if !status.success() {
        bail!("systemctl start failed");
    }

    tracing::info!("✓ Service started");
    Ok(())
}

/// Stop service
fn stop_service() -> Result<()> {
    tracing::info!("Stopping service...");

    let status = Command::new("systemctl")
        .args(&["stop", SERVICE_NAME])
        .status()
        .context("Failed to stop service")?;

    if !status.success() {
        bail!("systemctl stop failed");
    }

    tracing::info!("✓ Service stopped");
    Ok(())
}

/// Disable service
fn disable_service() -> Result<()> {
    tracing::info!("Disabling service...");

    let status = Command::new("systemctl")
        .args(&["disable", SERVICE_NAME])
        .status()
        .context("Failed to disable service")?;

    if !status.success() {
        bail!("systemctl disable failed");
    }

    tracing::info!("✓ Service disabled");
    Ok(())
}

/// Get service main process PID
fn get_service_pid() -> Result<u32> {
    let output = Command::new("systemctl")
        .args(&["show", "--property=MainPID", SERVICE_NAME])
        .output()
        .context("Failed to get service PID")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "MainPID=1234" format
    let pid_str = stdout
        .trim()
        .strip_prefix("MainPID=")
        .ok_or_else(|| anyhow::anyhow!("Failed to parse MainPID"))?;

    let pid: u32 = pid_str.parse().context("Failed to parse PID as number")?;

    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unit_file() {
        let content = generate_unit_file();

        // Verify key components
        assert!(content.contains("[Unit]"));
        assert!(content.contains("[Service]"));
        assert!(content.contains("[Install]"));
        assert!(content.contains("Type=notify"));
        assert!(content.contains(&format!("User={}", SERVICE_USER)));
        assert!(content.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn test_service_constants() {
        assert_eq!(SERVICE_NAME, "monoterminal");
        assert_eq!(SERVICE_USER, "monoterminal");
        assert_eq!(BINARY_PATH, "/usr/local/bin/monoterminal-master");
        assert_eq!(UNIT_FILE_PATH, "/etc/systemd/system/monoterminal.service");
    }
}
