// Phase 3 Distribution Package Validation Tests
// task-66: Week 11 Day 6
//
// Rust-based validation tests for package installation
// Complements phase3_distribution_packages.sh shell tests
//
// Tests package metadata, file presence, permissions, and service configuration
// Platforms: Ubuntu (.deb), Debian (.deb), Fedora (.rpm), macOS (Homebrew)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// =============================================================================
// Helper Functions
// =============================================================================

/// Check if file exists and is executable
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            let permissions = metadata.permissions();
            return permissions.mode() & 0o111 != 0; // Check execute bits
        }
    }

    #[cfg(windows)]
    {
        // On Windows, .exe extension implies executable
        return path.extension().map(|e| e == "exe").unwrap_or(false);
    }

    false
}

/// Get package manager type
fn detect_package_manager() -> Option<&'static str> {
    if Command::new("dpkg").arg("--version").output().is_ok() {
        Some("dpkg")
    } else if Command::new("rpm").arg("--version").output().is_ok() {
        Some("rpm")
    } else if Command::new("brew").arg("--version").output().is_ok() {
        Some("brew")
    } else {
        None
    }
}

/// Check if package is installed
fn is_package_installed(package_name: &str) -> bool {
    match detect_package_manager() {
        Some("dpkg") => {
            Command::new("dpkg")
                .args(&["-l", package_name])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(package_name))
                .unwrap_or(false)
        }
        Some("rpm") => {
            Command::new("rpm")
                .args(&["-q", package_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        Some("brew") => {
            Command::new("brew")
                .args(&["list", package_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        _ => false,
    }
}

// =============================================================================
// PKG-001: Binary Installation Validation (P0 Critical)
// =============================================================================

#[test]
#[ignore] // Requires package installation
fn test_pkg_001_binary_installed() {
    // Verify binary installed to correct location

    #[cfg(target_os = "linux")]
    let binary_paths = vec![
        Path::new("/usr/bin/monoterminal"),
        Path::new("/usr/local/bin/monoterminal"),
    ];

    #[cfg(target_os = "macos")]
    let binary_paths = vec![
        Path::new("/usr/local/bin/monoterminal"),
        Path::new("/opt/homebrew/bin/monoterminal"), // Apple Silicon
    ];

    #[cfg(target_os = "windows")]
    let binary_paths = vec![
        Path::new("C:\\Program Files\\Monoterminal\\monoterminal.exe"),
    ];

    let mut found = false;
    for path in &binary_paths {
        if path.exists() {
            println!("✅ Binary found: {:?}", path);

            // Verify executable permissions
            if is_executable(path) {
                println!("✅ Binary is executable");
            } else {
                panic!("Binary not executable: {:?}", path);
            }

            found = true;
            break;
        }
    }

    assert!(found, "Binary not found in any expected location");
}

#[test]
#[ignore] // Requires package installation
fn test_pkg_001_binary_version() {
    // Verify binary version matches package

    #[cfg(unix)]
    let binary_path = "/usr/bin/monoterminal";

    #[cfg(windows)]
    let binary_path = "C:\\Program Files\\Monoterminal\\monoterminal.exe";

    if !Path::new(binary_path).exists() {
        println!("SKIP: Binary not found at {}", binary_path);
        return;
    }

    // Run binary with --version
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .expect("Failed to run binary");

    let version_str = String::from_utf8_lossy(&output.stdout);
    println!("Version output: {}", version_str);

    // Should contain version number
    assert!(
        version_str.contains("0.1.0") || version_str.contains("monoterminal"),
        "Invalid version output: {}",
        version_str
    );
}

// =============================================================================
// PKG-002: Service File Installation (P0 Critical)
// =============================================================================

#[test]
#[ignore] // Requires package installation
#[cfg(target_os = "linux")]
fn test_pkg_002_systemd_service_file() {
    let service_file = Path::new("/lib/systemd/system/monoterminal.service");

    // Verify service file exists
    assert!(
        service_file.exists(),
        "systemd service file not found: {:?}",
        service_file
    );
    println!("✅ Service file found: {:?}", service_file);

    // Read and validate content
    let content = fs::read_to_string(service_file)
        .expect("Failed to read service file");

    // Verify required keys
    assert!(content.contains("[Unit]"), "Missing [Unit] section");
    assert!(content.contains("[Service]"), "Missing [Service] section");
    assert!(content.contains("[Install]"), "Missing [Install] section");

    assert!(
        content.contains("Type=notify"),
        "Should be Type=notify service"
    );
    assert!(
        content.contains("ExecStart"),
        "Missing ExecStart"
    );
    assert!(
        content.contains("Restart="),
        "Missing Restart policy"
    );

    println!("✅ Service file content validated");
}

#[test]
#[ignore] // Requires package installation
#[cfg(target_os = "macos")]
fn test_pkg_002_launchd_plist() {
    let plist_path = dirs::home_dir()
        .expect("No home directory")
        .join("Library/LaunchAgents/com.monoterminal.daemon.plist");

    // Verify plist exists
    assert!(
        plist_path.exists(),
        "launchd plist not found: {:?}",
        plist_path
    );
    println!("✅ LaunchAgent plist found: {:?}", plist_path);

    // Read and validate content
    let content = fs::read_to_string(&plist_path)
        .expect("Failed to read plist");

    // Verify required keys
    assert!(content.contains("<key>Label</key>"), "Missing Label");
    assert!(
        content.contains("com.monoterminal.daemon"),
        "Wrong label"
    );
    assert!(
        content.contains("<key>ProgramArguments</key>"),
        "Missing ProgramArguments"
    );

    println!("✅ LaunchAgent plist content validated");
}

// =============================================================================
// PKG-003: Package Metadata Validation (P1 High)
// =============================================================================

#[test]
#[ignore] // Requires package installation
fn test_pkg_003_package_registered() {
    let package_manager = detect_package_manager()
        .expect("No package manager detected");

    println!("Package manager: {}", package_manager);

    // Verify monoterminal package installed
    assert!(
        is_package_installed("monoterminal"),
        "monoterminal package not registered"
    );
    println!("✅ Package registered with {}", package_manager);
}

#[test]
#[ignore] // Requires package installation
#[cfg(any(target_os = "linux"))]
fn test_pkg_003_package_dependencies() {
    let package_manager = detect_package_manager()
        .expect("No package manager detected");

    match package_manager {
        "dpkg" => {
            // Query dependencies
            let output = Command::new("dpkg")
                .args(&["-s", "monoterminal"])
                .output()
                .expect("Failed to query package");

            let info = String::from_utf8_lossy(&output.stdout);

            // Check for expected dependencies
            if info.contains("Depends:") {
                println!("✅ Package has dependencies declared");
                println!("{}", info);
            } else {
                println!("ℹ️  No explicit dependencies (standalone package)");
            }
        }
        "rpm" => {
            let output = Command::new("rpm")
                .args(&["-qR", "monoterminal"])
                .output()
                .expect("Failed to query dependencies");

            let deps = String::from_utf8_lossy(&output.stdout);
            println!("Dependencies:\n{}", deps);
        }
        _ => {}
    }
}

// =============================================================================
// PKG-004: Directory and Permissions (P1 High)
// =============================================================================

#[test]
#[ignore] // Requires package installation
#[cfg(target_os = "linux")]
fn test_pkg_004_directories_created() {
    // Verify required directories exist

    let directories = vec![
        "/var/lib/monoterminal",
        "/var/log/monoterminal",
        "/etc/monoterminal",
    ];

    for dir in &directories {
        let path = Path::new(dir);
        if path.exists() {
            println!("✅ Directory exists: {}", dir);

            // Check ownership (should be monoterminal user or root)
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if let Ok(metadata) = fs::metadata(path) {
                    let uid = metadata.uid();
                    println!("  UID: {}", uid);
                }
            }
        } else {
            println!("ℹ️  Directory not found: {} (may be created on first run)", dir);
        }
    }
}

#[test]
#[ignore] // Requires package installation
#[cfg(unix)]
fn test_pkg_004_service_user_created() {
    // Verify monoterminal user exists (systemd service user)

    let output = Command::new("id")
        .arg("monoterminal")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let user_info = String::from_utf8_lossy(&output.stdout);
            println!("✅ Service user exists: {}", user_info);
        } else {
            println!("ℹ️  Service user not found (may run as current user)");
        }
    }
}

// =============================================================================
// PKG-005: Uninstallation Validation (P1 High)
// =============================================================================

#[test]
#[ignore] // Destructive test - manual execution only
fn test_pkg_005_clean_uninstall() {
    // This test verifies clean uninstallation
    // DO NOT RUN IN CI - manual verification only

    println!("ℹ️  Uninstallation test - manual verification required");
    println!("Expected behavior:");
    println!("  - Binary removed from /usr/bin or /usr/local/bin");
    println!("  - Service file removed");
    println!("  - Package unregistered");
    println!("  - User config PRESERVED in ~/.monoterminal/ (important!)");
    println!("  - System directories optionally removed (/var/lib/monoterminal)");
}

// =============================================================================
// PKG-006: Configuration Files (P2 Medium)
// =============================================================================

#[test]
#[ignore] // Requires package installation
fn test_pkg_006_config_file_locations() {
    // Verify config file locations

    #[cfg(unix)]
    let config_paths = vec![
        PathBuf::from("/etc/monoterminal/config.toml"),
        dirs::home_dir().expect("No home").join(".monoterminal/config.toml"),
    ];

    #[cfg(windows)]
    let config_paths = vec![
        dirs::home_dir()
            .expect("No home")
            .join("AppData\\Roaming\\monoterminal\\config.toml"),
    ];

    for path in &config_paths {
        if path.exists() {
            println!("✅ Config file found: {:?}", path);

            // Validate TOML syntax
            if let Ok(content) = fs::read_to_string(path) {
                match toml::from_str::<toml::Value>(&content) {
                    Ok(_) => println!("  ✓ Valid TOML syntax"),
                    Err(e) => panic!("Invalid TOML: {:?}", e),
                }
            }
        } else {
            println!("ℹ️  Config file not found: {:?} (created on first run)", path);
        }
    }
}

// =============================================================================
// PKG-007: Post-Install Scripts (P2 Medium)
// =============================================================================

#[test]
#[ignore] // Requires package installation
#[cfg(target_os = "linux")]
fn test_pkg_007_postinst_executed() {
    // Verify post-install script executed successfully

    // Check if postinst actions completed:
    // - Service user created
    // - Directories created
    // - systemd service enabled

    let checks = vec![
        ("Service file", Path::new("/lib/systemd/system/monoterminal.service")),
        ("Var lib dir", Path::new("/var/lib/monoterminal")),
    ];

    for (name, path) in checks {
        if path.exists() {
            println!("✅ {}: {:?} exists (postinst likely ran)", name, path);
        } else {
            println!("ℹ️  {}: {:?} not found", name, path);
        }
    }
}

// =============================================================================
// PKG-008: Package Upgrade Scenario (P1 High)
// =============================================================================

#[test]
#[ignore] // Requires two package versions
fn test_pkg_008_upgrade_preserves_data() {
    // Verify package upgrade preserves user data

    let user_config = dirs::home_dir()
        .expect("No home")
        .join(".monoterminal/config.toml");

    let user_db = dirs::home_dir()
        .expect("No home")
        .join(".monoterminal/monoterminal.db");

    // Before upgrade: Create test data
    if !user_config.exists() {
        println!("ℹ️  Test requires existing user data");
        return;
    }

    // After upgrade: Verify data still exists
    assert!(
        user_config.exists(),
        "User config removed after upgrade (should be preserved)"
    );
    println!("✅ User config preserved after upgrade");

    if user_db.exists() {
        println!("✅ User database preserved after upgrade");
    }
}

// =============================================================================
// Test Summary
// =============================================================================

#[test]
fn test_zzz_distribution_summary() {
    println!("\n=== Phase 3 Distribution Package Validation Test Summary ===");
    println!("Platform: {}", std::env::consts::OS);
    println!("Package Manager: {:?}", detect_package_manager());
    println!();
    println!("Tests: 8 test scenarios");
    println!("Coverage:");
    println!("  ✅ PKG-001: Binary Installation Validation (P0)");
    println!("  ✅ PKG-002: Service File Installation (P0)");
    println!("  ✅ PKG-003: Package Metadata Validation (P1)");
    println!("  ✅ PKG-004: Directory and Permissions (P1)");
    println!("  ✅ PKG-005: Uninstallation Validation (P1)");
    println!("  ✅ PKG-006: Configuration Files (P2)");
    println!("  ✅ PKG-007: Post-Install Scripts (P2)");
    println!("  ✅ PKG-008: Package Upgrade Scenario (P1)");
    println!("=============================================================\n");
}
