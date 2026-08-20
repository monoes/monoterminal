// MONOTERMINAL Master Daemon
// Phase 1: Windows + Web (ConPTY, wgpu rendering, WebSocket server)
// See: docs/monoterminal-srs.md

mod auth;
mod persistence;
mod platform;
mod pty;
mod server;
mod session;
mod ui;
mod webrtc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

use auth::{keys::load_or_generate_keypair, Ed25519AuthService, RateLimiter};
use monoterminal_monomind_bridge::HealthStatus;

/// MONOTERMINAL master daemon
#[derive(Parser, Debug)]
#[command(name = "monoterminal-master")]
#[command(about = "MONOTERMINAL master daemon", long_about = None)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Enable development mode (bypasses Ed25519 challenge-response auth)
    /// WARNING: Automatically issues JWT tokens without signature verification.
    /// DO NOT use in production - for E2E testing only.
    #[arg(long, default_value_t = false, global = true)]
    dev_mode: bool,

    /// Server bind address
    #[arg(long, default_value = "127.0.0.1:5000", global = true)]
    bind_addr: String,

    /// systemd mode (Type=notify readiness signaling)
    #[arg(long, hide = true)]
    systemd: bool,

    /// launchd mode (macOS launch daemon)
    #[arg(long, hide = true)]
    launchd: bool,
}

/// MONOTERMINAL commands
#[derive(Subcommand, Debug)]
enum Command {
    /// Install MONOTERMINAL as a system service
    #[command(name = "install-service")]
    InstallService {
        /// Force reinstall if already installed
        #[arg(long)]
        force: bool,
    },

    /// Uninstall MONOTERMINAL system service
    #[command(name = "uninstall-service")]
    UninstallService {
        /// Remove data directories without prompting
        #[arg(long)]
        remove_data: bool,

        /// Remove service user without prompting
        #[arg(long)]
        remove_user: bool,
    },

    /// Check MONOTERMINAL service status
    #[command(name = "service-status")]
    ServiceStatus,
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

/// Handle service management commands
async fn handle_service_command(command: Command) -> Result<()> {
    use platform::service::{install_service, require_root, service_status, uninstall_service};

    match command {
        Command::InstallService { force } => {
            // Check root privileges
            if !platform::service::is_root() {
                eprintln!("Error: Installation requires root privileges");
                eprintln!();
                eprintln!("Please run with sudo:");
                #[cfg(unix)]
                eprintln!("  sudo monoterminal-master install-service");
                #[cfg(windows)]
                eprintln!("  Run as Administrator");
                std::process::exit(1);
            }

            if force {
                // Uninstall first if already installed
                println!("Force mode: Uninstalling existing service...");
                let _ = uninstall_service(); // Ignore errors
            }

            install_service()?;
        }

        Command::UninstallService {
            remove_data,
            remove_user,
        } => {
            require_root()?;

            // Interactive prompts for data/user removal (if not already specified)
            let confirm_remove_data = if remove_data {
                true
            } else {
                print!(
                    "\nData directory: {}\nRemove data directory? [y/N]: ",
                    platform::paths::data_dir().display()
                );
                read_confirmation()
            };

            let confirm_remove_user = if remove_user {
                true
            } else {
                print!("\nService user: monoterminal\nRemove service user? [y/N]: ");
                read_confirmation()
            };

            uninstall_service()?;

            // Post-uninstall cleanup based on user confirmation
            if confirm_remove_data {
                println!("\nRemoving data directory...");
                let data_dir = platform::paths::data_dir();
                if data_dir.exists() {
                    std::fs::remove_dir_all(&data_dir).context(format!(
                        "Failed to remove data directory: {}",
                        data_dir.display()
                    ))?;
                    println!("✓ Data directory removed: {}", data_dir.display());
                }
            } else {
                println!(
                    "\nData directory preserved: {}",
                    platform::paths::data_dir().display()
                );
            }

            if confirm_remove_user {
                println!("\nRemoving service user...");
                #[cfg(target_os = "linux")]
                {
                    let status = std::process::Command::new("userdel")
                        .arg("monoterminal")
                        .status();

                    match status {
                        Ok(s) if s.success() => println!("✓ Service user removed: monoterminal"),
                        _ => eprintln!(
                            "Warning: Failed to remove service user (may need manual cleanup)"
                        ),
                    }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    println!("User removal not implemented for this platform");
                }
            } else {
                println!("\nService user preserved: monoterminal");
            }
        }

        Command::ServiceStatus => {
            let status = service_status()?;

            println!();
            println!("MONOTERMINAL Service Status");
            println!("============================");
            println!();
            println!(
                "Installed: {}",
                if status.installed {
                    "✓ Yes"
                } else {
                    "✗ No"
                }
            );
            println!(
                "Running:   {}",
                if status.running { "✓ Yes" } else { "✗ No" }
            );
            println!(
                "Enabled:   {}",
                if status.enabled {
                    "✓ Yes (auto-start on boot)"
                } else {
                    "✗ No"
                }
            );

            if let Some(pid) = status.pid {
                println!("PID:       {}", pid);
            }

            println!();
            println!("Status: {}", status.message);
            println!();

            #[cfg(target_os = "linux")]
            {
                println!("Commands:");
                println!("  View logs:  sudo journalctl -u monoterminal -f");
                println!("  Restart:    sudo systemctl restart monoterminal");
                println!("  Stop:       sudo systemctl stop monoterminal");
            }

            // Exit with code 0 if running, 1 if not
            if !status.running {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Handle service management commands (non-daemon mode)
    if let Some(command) = args.command {
        return handle_service_command(command).await;
    }

    // Initialize logging based on environment
    // SOAK_TEST_MODE=1 enables structured JSON logging to file for 24h soak tests
    // Otherwise: compact console output for normal operation
    if std::env::var("SOAK_TEST_MODE").is_ok() {
        // Soak test mode: JSON logs to hourly-rotated files
        let file_appender = tracing_appender::rolling::hourly("./soak-logs", "monoterminal.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::fmt()
            .json() // Structured JSON for post-test parsing
            .with_current_span(false)
            .with_span_list(false)
            .with_writer(non_blocking)
            .init();

        // Guard must live for program lifetime - box it and leak (acceptable for daemon)
        Box::leak(Box::new(_guard));

        tracing::info!(
            "MONOTERMINAL master daemon starting (SOAK TEST MODE - JSON logging enabled)"
        );
    } else {
        // Normal mode: compact console output
        tracing_subscriber::fmt()
            .with_target(false)
            .compact()
            .init();

        tracing::info!("MONOTERMINAL master daemon starting...");
    }

    tracing::info!("Phase 1: Windows + Web client");

    // Check if running under systemd (Type=notify)
    #[cfg(target_os = "linux")]
    let systemd_mode = args.systemd || platform::service::sd_notify::is_systemd();
    #[cfg(not(target_os = "linux"))]
    let systemd_mode = false;

    if systemd_mode {
        tracing::info!("Running in systemd mode (Type=notify)");
        #[cfg(target_os = "linux")]
        {
            platform::service::sd_notify::notify_status("Starting MONOTERMINAL daemon...")?;
        }
    }

    if args.dev_mode {
        tracing::warn!("⚠️  DEV MODE ENABLED - Auth bypass active (auto-issuing JWT tokens)");
        tracing::warn!("⚠️  DO NOT use in production - for E2E testing only");
    }

    // Phase 1 Implementation Status:
    // ✅ WebSocket server with TLS 1.3 (§3.1.2, §3.2.1)
    // ✅ Protocol Runtime Integration (§3.1.1)
    // ✅ Ed25519/JWT authentication (§3.2.2)
    // ✅ Monomind bridge integration (§2.4) - task-2 COMPLETE
    // 🔄 ConPTY session manager (§2.1.2) - IN PROGRESS
    // ⏳ wgpu + egui rendering (§2.1.1, §4.2.1) - BLOCKED (gpu-rendering-engineer)

    println!("MONOTERMINAL v0.1.0 - Phase 1 (Windows)");
    if args.dev_mode {
        println!("⚠️  RUNNING IN DEV MODE - Auth bypass enabled");
    }

    // 1. Load or generate Ed25519 keypair for JWT signing
    #[cfg(target_os = "linux")]
    if systemd_mode {
        platform::service::sd_notify::notify_status("Loading Ed25519 keypair...")?;
    }

    let keypair = load_or_generate_keypair().context("Failed to load Ed25519 keypair")?;
    tracing::info!("Ed25519 keypair loaded");

    // 2. Create authentication service (Ed25519 + JWT)
    let auth_service = Arc::new(Ed25519AuthService::new(&keypair)?);
    tracing::info!("Ed25519 authentication service initialized");

    // 3. Create rate limiter (connection + auth + session limits)
    let rate_limiter = Arc::new(RateLimiter::new());
    tracing::info!("Rate limiter initialized");

    // 4. Create session manager
    #[cfg(target_os = "linux")]
    if systemd_mode {
        platform::service::sd_notify::notify_status("Initializing session manager...")?;
    }

    let session_manager = Arc::new(session::manager::SessionManager::new(None));
    tracing::info!("Session manager initialized");

    // 5. Create broadcast channel for health status updates
    let (health_tx, _health_rx) = broadcast::channel::<HealthStatus>(16);

    // 6. Start daily health check scheduler (24h interval)
    let project_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    start_health_scheduler(project_dir, health_tx.clone());
    tracing::info!("Health check scheduler started (24h interval)");

    // 7. Create server configuration
    let mut server_config = server::ServerConfig::default();
    #[allow(clippy::field_reassign_with_default)]
    {
        server_config.bind_addr = args.bind_addr.parse().context("Invalid bind address")?;
        server_config.dev_mode = args.dev_mode;
    }

    tracing::info!(
        "Server configuration: bind_addr={}, max_connections={}, dev_mode={}",
        server_config.bind_addr,
        server_config.max_connections,
        server_config.dev_mode
    );

    // 8. Create WebSocket server with auth + rate limiting
    #[cfg(target_os = "linux")]
    if systemd_mode {
        platform::service::sd_notify::notify_status("Creating WebSocket server...")?;
    }

    let server = server::Server::new(
        server_config,
        session_manager,
        rate_limiter,
        auth_service,
        health_tx,
    )?;
    tracing::info!("WebSocket server created");

    // 9. Notify systemd that we're ready (Type=notify contract)
    #[cfg(target_os = "linux")]
    if systemd_mode {
        platform::service::sd_notify::notify_ready()?;
        tracing::info!("✓ systemd notified: service ready");
    }

    // 10. Run server (blocking until shutdown signal)
    server.run().await?;

    // 11. Notify systemd that we're stopping
    #[cfg(target_os = "linux")]
    if systemd_mode {
        platform::service::sd_notify::notify_stopping()?;
        tracing::info!("✓ systemd notified: service stopping");
    }

    Ok(())
}

/// Load or generate JWT signing key
///
/// Priority order:
/// 1. Environment variable MONOTERMINAL_JWT_KEY (hex-encoded 32 bytes)
/// 2. File: %ProgramData%\MONOTERMINAL\jwt_key.bin (service mode)
/// 3. File: %LOCALAPPDATA%\monoterminal\jwt_key.bin (console mode)
/// 4. Auto-generate and save new key
#[allow(dead_code)] // Reserved for production deployment, cleanup tracked in task-63
fn load_or_generate_jwt_key() -> Result<[u8; 32]> {
    use std::env;
    use std::fs;

    // 1. Try environment variable first
    if let Ok(key_hex) = env::var("MONOTERMINAL_JWT_KEY") {
        let key_bytes =
            hex::decode(&key_hex).context("Invalid JWT key in MONOTERMINAL_JWT_KEY (not hex)")?;
        if key_bytes.len() != 32 {
            anyhow::bail!("JWT key must be exactly 32 bytes (got {})", key_bytes.len());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        tracing::info!("JWT key loaded from environment variable");
        return Ok(key);
    }

    // 2. Try file (service mode: %ProgramData%, console mode: %LOCALAPPDATA%)
    let key_path = if let Ok(program_data) = env::var("ProgramData") {
        // Service mode
        PathBuf::from(program_data)
            .join("MONOTERMINAL")
            .join("jwt_key.bin")
    } else if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
        // Console mode
        PathBuf::from(local_appdata)
            .join("monoterminal")
            .join("jwt_key.bin")
    } else {
        // Fallback: current directory (development only)
        PathBuf::from("jwt_key.bin")
    };

    // Try to load existing key
    if key_path.exists() {
        let key_bytes = fs::read(&key_path).context("Failed to read JWT key file")?;
        if key_bytes.len() != 32 {
            anyhow::bail!(
                "JWT key file corrupt (expected 32 bytes, got {})",
                key_bytes.len()
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        tracing::info!("JWT key loaded from {}", key_path.display());
        return Ok(key);
    }

    // 3. Generate new key and save
    let key = generate_random_key();

    // Create parent directory if it doesn't exist
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent).context("Failed to create JWT key directory")?;
    }

    fs::write(&key_path, key).context("Failed to save JWT key")?;
    tracing::info!("Generated new JWT key and saved to {}", key_path.display());

    Ok(key)
}

/// Generate cryptographically secure random 32-byte key
#[allow(dead_code)] // Reserved for production deployment, cleanup tracked in task-63
fn generate_random_key() -> [u8; 32] {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Start daily health check scheduler as background task
fn start_health_scheduler(project_dir: PathBuf, health_tx: broadcast::Sender<HealthStatus>) {
    tokio::spawn(async move {
        use monoterminal_monomind_bridge::HealthScheduler;

        let scheduler = HealthScheduler::new(); // Default 24-hour interval

        tracing::info!(
            path = %project_dir.display(),
            "Starting daily health check scheduler"
        );

        let result = scheduler
            .start(&project_dir, move |health| {
                let tx = health_tx.clone();
                async move {
                    tracing::info!(
                        healthy = health.is_healthy(),
                        issues = health.issues.len(),
                        version = ?health.version,
                        "Scheduled health check complete"
                    );

                    // Broadcast to all WebSocket clients
                    if let Err(e) = tx.send(health.clone()) {
                        tracing::debug!("No health status subscribers: {}", e);
                    }
                }
            })
            .await;

        if let Err(e) = result {
            tracing::error!(error = %e, "Health scheduler terminated with error");
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic_startup() {
        // Placeholder test - main binary has minimal testable logic
        assert_eq!(2 + 2, 4);
    }
}
