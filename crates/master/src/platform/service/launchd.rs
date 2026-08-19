// launchd Service Management Implementation (macOS)
// Phase 3 Week 5: task-56
//
// Implements service installation, uninstallation, and status checking for macOS launchd.
// Based on architecture design from task-54.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::ServiceStatus;
use crate::platform::paths::{data_dir, log_dir};

/// Service label (reverse-DNS format)
const SERVICE_LABEL: &str = "com.monoterminal.master";

/// Service user name (underscore prefix for system accounts on macOS)
const SERVICE_USER: &str = "_monoterminal";

/// Service group name
const SERVICE_GROUP: &str = "_monoterminal";

/// Binary installation path
const BINARY_PATH: &str = "/usr/local/bin/monoterminal-master";

/// launchd plist file path
const PLIST_PATH: &str = "/Library/LaunchDaemons/com.monoterminal.master.plist";

/// Install monoterminal as a launchd service
///
/// Steps:
/// 1. Check if already installed
/// 2. Copy binary to /usr/local/bin/
/// 3. Create service user and group
/// 4. Create data and log directories
/// 5. Install launchd plist file
/// 6. Set plist permissions
/// 7. Load service with launchctl
/// 8. Start service
///
/// # Errors
///
/// Returns an error if any installation step fails.
pub fn install_service() -> Result<()> {
    tracing::info!("Installing MONOTERMINAL as launchd service...");

    // 1. Check if already installed
    if Path::new(PLIST_PATH).exists() {
        bail!("Service already installed at {}. Uninstall first or use --force.", PLIST_PATH);
    }

    // 2. Copy binary to system location
    install_binary()?;

    // 3. Create service user and group
    create_service_user()?;

    // 4. Create directories with correct permissions
    create_directories()?;

    // 5. Install plist file
    install_plist_file()?;

    // 6. Set plist permissions (root:wheel, 644)
    set_plist_permissions()?;

    // 7. Load service with launchctl
    load_service()?;

    tracing::info!("✓ MONOTERMINAL service installed successfully");
    println!("");
    println!("MONOTERMINAL service installed successfully!");
    println!("");
    println!("Service status:");
    println!("  sudo launchctl list | grep monoterminal");
    println!("");
    println!("View logs:");
    println!("  sudo tail -f /Library/Logs/MONOTERMINAL/stdout.log");
    println!("  sudo tail -f /Library/Logs/MONOTERMINAL/stderr.log");
    println!("");
    println!("To uninstall:");
    println!("  sudo monoterminal-master uninstall-service");

    Ok(())
}

/// Read yes/no confirmation from user
fn read_confirmation() -> bool {
    use std::io::{self, Write};

    let mut input = String::new();
    io::stdout().flush().expect("Failed to flush stdout");

    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        }
        Err(_) => false,
    }
}

/// Uninstall monoterminal launchd service
///
/// Steps:
/// 1. Unload service (if loaded)
/// 2. Remove plist file
/// 3. Remove binary
/// 4. Prompt for data directory removal
/// 5. Prompt for service user removal
pub fn uninstall_service() -> Result<()> {
    tracing::info!("Uninstalling MONOTERMINAL launchd service...");

    // 1. Unload service (if loaded)
    let _ = unload_service(); // Ignore errors if not loaded

    // 2. Remove plist file
    if Path::new(PLIST_PATH).exists() {
        fs::remove_file(PLIST_PATH)
            .context("Failed to remove plist file")?;
        tracing::info!("Removed plist file: {}", PLIST_PATH);
    }

    // 3. Remove binary
    if Path::new(BINARY_PATH).exists() {
        fs::remove_file(BINARY_PATH)
            .context("Failed to remove binary")?;
        tracing::info!("Removed binary: {}", BINARY_PATH);
    }

    // 4. Interactive prompt for data directory removal
    let data_dir_path = data_dir();
    let log_dir_path = log_dir();

    print!("\nData directory: {}\nRemove data directory? [y/N]: ", data_dir_path.display());
    let remove_data = read_confirmation();

    if remove_data {
        println!("\nRemoving data directory...");
        if data_dir_path.exists() {
            fs::remove_dir_all(&data_dir_path)
                .context(format!("Failed to remove data directory: {}", data_dir_path.display()))?;
            println!("✓ Data directory removed: {}", data_dir_path.display());
        }
        if log_dir_path.exists() {
            fs::remove_dir_all(&log_dir_path)
                .context(format!("Failed to remove log directory: {}", log_dir_path.display()))?;
            println!("✓ Log directory removed: {}", log_dir_path.display());
        }
    } else {
        println!("\nData directory preserved: {}", data_dir_path.display());
        println!("Log directory preserved: {}", log_dir_path.display());
    }

    // 5. Interactive prompt for user removal
    print!("\nService user: {}\nRemove service user and group? [y/N]: ", SERVICE_USER);
    let remove_user = read_confirmation();

    if remove_user {
        println!("\nRemoving service user and group...");
        remove_service_user()?;
    } else {
        println!("\nService user preserved: {}", SERVICE_USER);
        println!("To remove later: sudo dscl . -delete /Users/{}", SERVICE_USER);
        println!("                 sudo dscl . -delete /Groups/{}", SERVICE_GROUP);
    }

    tracing::info!("✓ MONOTERMINAL service uninstalled successfully");
    println!("");
    println!("MONOTERMINAL service uninstalled successfully!");

    Ok(())
}

/// Get launchd service status
pub fn service_status() -> Result<ServiceStatus> {
    // Check if plist is installed
    let installed = Path::new(PLIST_PATH).exists();

    // Check if service is loaded and running
    let output = Command::new("launchctl")
        .args(&["list", SERVICE_LABEL])
        .output()
        .context("Failed to check service status")?;

    let running = output.status.success();

    // Parse PID from launchctl output
    let pid = if running {
        get_service_pid().ok()
    } else {
        None
    };

    // launchd services are always "enabled" if plist exists (RunAtLoad=true)
    let enabled = installed;

    let message = if running {
        format!("Service is running (PID: {})", pid.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string()))
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
    let current_exe = std::env::current_exe()
        .context("Failed to get current executable path")?;

    tracing::info!("Copying binary: {} -> {}", current_exe.display(), BINARY_PATH);

    // Copy binary
    fs::copy(&current_exe, BINARY_PATH)
        .context("Failed to copy binary to /usr/local/bin")?;

    // Set executable permissions (755)
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

/// Create service user and group using dscl (Directory Service)
fn create_service_user() -> Result<()> {
    tracing::info!("Creating service user: {}", SERVICE_USER);

    // Check if user already exists
    let check = Command::new("dscl")
        .args(&[".", "-read", &format!("/Users/{}", SERVICE_USER)])
        .output();

    if check.is_ok() && check.unwrap().status.success() {
        tracing::info!("Service user already exists: {}", SERVICE_USER);
        return Ok(());
    }

    // Find next available system UID (200-399 range for macOS system accounts)
    let next_uid = find_next_system_uid()?;

    // Create user with dscl
    // Note: macOS system accounts use underscore prefix and UID < 500
    let commands = vec![
        // Create user record
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER)],
        // Set real name
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER), "RealName", "MONOTERMINAL Master Daemon"],
        // Set UID
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER), "UniqueID", &next_uid.to_string()],
        // Set primary group ID (same as UID)
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER), "PrimaryGroupID", &next_uid.to_string()],
        // Set shell to /usr/bin/false (no login)
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER), "UserShell", "/usr/bin/false"],
        // Set NFSHomeDirectory
        vec![".", "-create", &format!("/Users/{}", SERVICE_USER), "NFSHomeDirectory", "/var/empty"],
    ];

    for args in commands {
        let status = Command::new("dscl")
            .args(&args)
            .status()
            .context("Failed to create service user")?;

        if !status.success() {
            bail!("dscl command failed: dscl {}", args.join(" "));
        }
    }

    // Create group with same GID
    let group_commands = vec![
        vec![".", "-create", &format!("/Groups/{}", SERVICE_GROUP)],
        vec![".", "-create", &format!("/Groups/{}", SERVICE_GROUP), "PrimaryGroupID", &next_uid.to_string()],
    ];

    for args in group_commands {
        let status = Command::new("dscl")
            .args(&args)
            .status()
            .context("Failed to create service group")?;

        if !status.success() {
            bail!("dscl command failed: dscl {}", args.join(" "));
        }
    }

    tracing::info!("✓ Service user created: {} (UID: {})", SERVICE_USER, next_uid);
    Ok(())
}

/// Find next available system UID in range 200-399
fn find_next_system_uid() -> Result<u32> {
    // List all users and their UIDs
    let output = Command::new("dscl")
        .args(&[".", "-list", "/Users", "UniqueID"])
        .output()
        .context("Failed to list users")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse UIDs
    let mut uids: Vec<u32> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                parts[1].parse::<u32>().ok()
            } else {
                None
            }
        })
        .filter(|&uid| uid >= 200 && uid < 400)
        .collect();

    uids.sort();

    // Find first gap in sequence
    for uid in 200..400 {
        if !uids.contains(&uid) {
            return Ok(uid);
        }
    }

    bail!("No available system UIDs in range 200-399")
}

/// Create data and log directories with correct ownership
fn create_directories() -> Result<()> {
    let data_dir_path = data_dir();
    let log_dir_path = log_dir();

    tracing::info!("Creating data directory: {}", data_dir_path.display());
    tracing::info!("Creating log directory: {}", log_dir_path.display());

    // Create directories
    fs::create_dir_all(&data_dir_path)
        .context("Failed to create data directory")?;
    fs::create_dir_all(&log_dir_path)
        .context("Failed to create log directory")?;

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

/// Install launchd plist file
fn install_plist_file() -> Result<()> {
    tracing::info!("Installing launchd plist file: {}", PLIST_PATH);

    // Generate plist content
    let plist_content = generate_plist();

    // Write plist file
    let mut file = fs::File::create(PLIST_PATH)
        .context("Failed to create plist file")?;
    file.write_all(plist_content.as_bytes())
        .context("Failed to write plist file")?;

    tracing::info!("✓ Plist file installed: {}", PLIST_PATH);
    Ok(())
}

/// Set plist file permissions (root:wheel, 644)
fn set_plist_permissions() -> Result<()> {
    // Set ownership to root:wheel
    let status = Command::new("chown")
        .args(&["root:wheel", PLIST_PATH])
        .status()
        .context("Failed to set plist ownership")?;

    if !status.success() {
        bail!("chown failed for plist file");
    }

    // Set permissions (644)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o644);
        fs::set_permissions(PLIST_PATH, permissions)
            .context("Failed to set plist permissions")?;
    }

    tracing::info!("✓ Plist permissions set: root:wheel 644");
    Ok(())
}

/// Generate launchd plist content
///
/// Uses template from templates/launchd/com.monoterminal.master.plist with dynamic paths
fn generate_plist() -> String {
    let data_dir_path = data_dir();
    let log_dir_path = log_dir();

    format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Service identification -->
    <key>Label</key>
    <string>{label}</string>

    <!-- Program and arguments -->
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>--launchd</string>
    </array>

    <!-- Working directory -->
    <key>WorkingDirectory</key>
    <string>{working_dir}</string>

    <!-- Auto-start on boot -->
    <key>RunAtLoad</key>
    <true/>

    <!-- Keep alive (auto-restart on crash) -->
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>

    <!-- Restart throttle: minimum 5 seconds between restarts -->
    <key>ThrottleInterval</key>
    <integer>5</integer>

    <!-- Standard output and error -->
    <key>StandardOutPath</key>
    <string>{log_dir}/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{log_dir}/stderr.log</string>

    <!-- User and group -->
    <key>UserName</key>
    <string>{user}</string>
    <key>GroupName</key>
    <string>{group}</string>

    <!-- Environment variables -->
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
        <key>TERM</key>
        <string>xterm-256color</string>
    </dict>

    <!-- Resource limits -->
    <key>SoftResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>65536</integer>
        <key>NumberOfProcesses</key>
        <integer>512</integer>
    </dict>

    <key>HardResourceLimits</key>
    <dict>
        <key>NumberOfFiles</key>
        <integer>65536</integer>
        <key>NumberOfProcesses</key>
        <integer>512</integer>
    </dict>

    <!-- Process type: Adaptive for daemons -->
    <key>ProcessType</key>
    <string>Adaptive</string>

    <!-- Session creation -->
    <key>SessionCreate</key>
    <true/>

    <!-- Nice priority -->
    <key>Nice</key>
    <integer>0</integer>

    <!-- Abandoned process cleanup -->
    <key>AbandonProcessGroup</key>
    <true/>
</dict>
</plist>
"#,
        label = SERVICE_LABEL,
        binary = BINARY_PATH,
        working_dir = data_dir_path.display(),
        log_dir = log_dir_path.display(),
        user = SERVICE_USER,
        group = SERVICE_GROUP,
    )
}

/// Load service with launchctl
fn load_service() -> Result<()> {
    tracing::info!("Loading service with launchctl...");

    let status = Command::new("launchctl")
        .args(&["load", PLIST_PATH])
        .status()
        .context("Failed to load service")?;

    if !status.success() {
        bail!("launchctl load failed");
    }

    tracing::info!("✓ Service loaded");
    Ok(())
}

/// Unload service with launchctl
fn unload_service() -> Result<()> {
    tracing::info!("Unloading service with launchctl...");

    let status = Command::new("launchctl")
        .args(&["unload", PLIST_PATH])
        .status()
        .context("Failed to unload service")?;

    if !status.success() {
        bail!("launchctl unload failed");
    }

    tracing::info!("✓ Service unloaded");
    Ok(())
}

/// Remove service user and group using dscl
fn remove_service_user() -> Result<()> {
    tracing::info!("Removing service user and group...");

    // Remove user
    let user_status = Command::new("dscl")
        .args(&[".", "-delete", &format!("/Users/{}", SERVICE_USER)])
        .status();

    match user_status {
        Ok(s) if s.success() => {
            tracing::info!("✓ Service user removed: {}", SERVICE_USER);
            println!("✓ Service user removed: {}", SERVICE_USER);
        }
        _ => {
            eprintln!("Warning: Failed to remove service user (may need manual cleanup)");
            tracing::warn!("Failed to remove service user: {}", SERVICE_USER);
        }
    }

    // Remove group
    let group_status = Command::new("dscl")
        .args(&[".", "-delete", &format!("/Groups/{}", SERVICE_GROUP)])
        .status();

    match group_status {
        Ok(s) if s.success() => {
            tracing::info!("✓ Service group removed: {}", SERVICE_GROUP);
            println!("✓ Service group removed: {}", SERVICE_GROUP);
        }
        _ => {
            eprintln!("Warning: Failed to remove service group (may need manual cleanup)");
            tracing::warn!("Failed to remove service group: {}", SERVICE_GROUP);
        }
    }

    Ok(())
}

/// Get service PID from launchctl
fn get_service_pid() -> Result<u32> {
    let output = Command::new("launchctl")
        .args(&["list", SERVICE_LABEL])
        .output()
        .context("Failed to get service info")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse PID from launchctl list output
    // Format: "12345\t0\tcom.monoterminal.master"
    let pid_str = stdout
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse launchctl output"))?;

    // Handle "-" for not running
    if pid_str == "-" {
        bail!("Service not running");
    }

    let pid: u32 = pid_str
        .parse()
        .context("Failed to parse PID as number")?;

    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plist() {
        let content = generate_plist();

        // Verify key components
        assert!(content.contains("<?xml version=\"1.0\""));
        assert!(content.contains("<key>Label</key>"));
        assert!(content.contains(SERVICE_LABEL));
        assert!(content.contains("<key>UserName</key>"));
        assert!(content.contains(SERVICE_USER));
        assert!(content.contains("<key>RunAtLoad</key>"));
        assert!(content.contains("<true/>"));
    }

    #[test]
    fn test_service_constants() {
        assert_eq!(SERVICE_LABEL, "com.monoterminal.master");
        assert_eq!(SERVICE_USER, "_monoterminal");
        assert_eq!(BINARY_PATH, "/usr/local/bin/monoterminal-master");
        assert_eq!(PLIST_PATH, "/Library/LaunchDaemons/com.monoterminal.master.plist");
    }
}
