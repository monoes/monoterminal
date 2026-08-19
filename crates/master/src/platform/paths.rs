// Cross-platform file path abstractions
// Phase 3 Week 3
//
// Implements platform-specific data directory paths following OS conventions:
//
// **Linux:**
// - System data: /var/lib/monoterminal
// - User data: ~/.local/share/monoterminal (XDG_DATA_HOME)
// - Logs: ~/.local/state/monoterminal/logs (XDG_STATE_HOME)
//
// **macOS:**
// - System data: /Library/Application Support/MONOTERMINAL
// - User data: ~/Library/Application Support/MONOTERMINAL
// - Logs: ~/Library/Logs/MONOTERMINAL
//
// **Windows:**
// - System data: %ProgramData%\MONOTERMINAL
// - User data: %LOCALAPPDATA%\MONOTERMINAL
// - Logs: %LOCALAPPDATA%\MONOTERMINAL\logs

use std::path::PathBuf;
use std::env;

/// Get system-wide data directory
///
/// This is the directory for system-wide application data, typically requiring
/// elevated privileges to write.
///
/// # Platform-specific paths
///
/// - **Linux:** `/var/lib/monoterminal`
/// - **macOS:** `/Library/Application Support/MONOTERMINAL`
/// - **Windows:** `%ProgramData%\MONOTERMINAL`
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::data_dir;
///
/// let dir = data_dir();
/// // Linux: PathBuf::from("/var/lib/monoterminal")
/// // macOS: PathBuf::from("/Library/Application Support/MONOTERMINAL")
/// // Windows: PathBuf::from("C:\\ProgramData\\MONOTERMINAL")
/// ```
#[cfg(target_os = "linux")]
pub fn data_dir() -> PathBuf {
    PathBuf::from("/var/lib/monoterminal")
}

#[cfg(target_os = "macos")]
pub fn data_dir() -> PathBuf {
    PathBuf::from("/Library/Application Support/MONOTERMINAL")
}

#[cfg(windows)]
pub fn data_dir() -> PathBuf {
    env::var("ProgramData")
        .map(|s| PathBuf::from(s).join("MONOTERMINAL"))
        .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData\\MONOTERMINAL"))
}

/// Get per-user data directory
///
/// This is the directory for user-specific application data.
/// Follows XDG Base Directory Specification on Linux.
///
/// # Platform-specific paths
///
/// - **Linux:** `~/.local/share/monoterminal` or `$XDG_DATA_HOME/monoterminal`
/// - **macOS:** `~/Library/Application Support/MONOTERMINAL`
/// - **Windows:** `%LOCALAPPDATA%\MONOTERMINAL`
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::user_data_dir;
///
/// let dir = user_data_dir();
/// // Linux: PathBuf::from("/home/user/.local/share/monoterminal")
/// // macOS: PathBuf::from("/Users/user/Library/Application Support/MONOTERMINAL")
/// // Windows: PathBuf::from("C:\\Users\\user\\AppData\\Local\\MONOTERMINAL")
/// ```
#[cfg(target_os = "linux")]
pub fn user_data_dir() -> PathBuf {
    // XDG Base Directory Specification
    // https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
    if let Ok(xdg_data_home) = env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg_data_home).join("monoterminal")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local/share/monoterminal")
    } else {
        // Fallback if HOME is not set (unusual)
        PathBuf::from("/tmp/monoterminal")
    }
}

#[cfg(target_os = "macos")]
pub fn user_data_dir() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join("Library/Application Support/MONOTERMINAL")
    } else {
        // Fallback if HOME is not set (unusual on macOS)
        PathBuf::from("/tmp/MONOTERMINAL")
    }
}

#[cfg(windows)]
pub fn user_data_dir() -> PathBuf {
    env::var("LOCALAPPDATA")
        .map(|s| PathBuf::from(s).join("MONOTERMINAL"))
        .unwrap_or_else(|_| {
            // Fallback: try USERPROFILE
            env::var("USERPROFILE")
                .map(|s| PathBuf::from(s).join("AppData\\Local\\MONOTERMINAL"))
                .unwrap_or_else(|_| PathBuf::from("C:\\MONOTERMINAL"))
        })
}

/// Get log directory
///
/// This is the directory for log files.
/// Follows XDG Base Directory Specification on Linux.
///
/// # Platform-specific paths
///
/// - **Linux:** `~/.local/state/monoterminal/logs` or `$XDG_STATE_HOME/monoterminal/logs`
///            (falls back to system `/var/log/monoterminal` if running as service)
/// - **macOS:** `~/Library/Logs/MONOTERMINAL`
/// - **Windows:** `%LOCALAPPDATA%\MONOTERMINAL\logs`
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::log_dir;
///
/// let dir = log_dir();
/// // Linux: PathBuf::from("/home/user/.local/state/monoterminal/logs")
/// // macOS: PathBuf::from("/Users/user/Library/Logs/MONOTERMINAL")
/// // Windows: PathBuf::from("C:\\Users\\user\\AppData\\Local\\MONOTERMINAL\\logs")
/// ```
#[cfg(target_os = "linux")]
pub fn log_dir() -> PathBuf {
    // XDG_STATE_HOME for user logs (XDG Base Directory Specification)
    if let Ok(xdg_state_home) = env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg_state_home).join("monoterminal/logs")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".local/state/monoterminal/logs")
    } else {
        // Fallback for system service (requires elevated privileges)
        PathBuf::from("/var/log/monoterminal")
    }
}

#[cfg(target_os = "macos")]
pub fn log_dir() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join("Library/Logs/MONOTERMINAL")
    } else {
        // Fallback for system service
        PathBuf::from("/Library/Logs/MONOTERMINAL")
    }
}

#[cfg(windows)]
pub fn log_dir() -> PathBuf {
    user_data_dir().join("logs")
}

/// Get SQLite database path for session persistence
///
/// Returns the full path to the session database file.
/// Uses user data directory for per-user sessions.
///
/// # Platform-specific paths
///
/// - **Linux:** `~/.local/share/monoterminal/sessions.db`
/// - **macOS:** `~/Library/Application Support/MONOTERMINAL/sessions.db`
/// - **Windows:** `%LOCALAPPDATA%\MONOTERMINAL\sessions.db`
///
/// # Examples
///
/// ```
/// use monoterminal_master::platform::session_db_path;
///
/// let db_path = session_db_path();
/// // Linux: PathBuf::from("/home/user/.local/share/monoterminal/sessions.db")
/// // macOS: PathBuf::from("/Users/user/Library/Application Support/MONOTERMINAL/sessions.db")
/// // Windows: PathBuf::from("C:\\Users\\user\\AppData\\Local\\MONOTERMINAL\\sessions.db")
/// ```
pub fn session_db_path() -> PathBuf {
    user_data_dir().join("sessions.db")
}

/// Ensure directory exists with appropriate permissions
///
/// Creates the directory and all parent directories if they don't exist.
/// On Unix platforms, sets permissions to 0755 (rwxr-xr-x).
///
/// # Arguments
///
/// * `path` - The directory path to create
///
/// # Errors
///
/// Returns an error if directory creation fails.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::paths::ensure_dir_exists;
/// use std::path::Path;
///
/// ensure_dir_exists(Path::new("/var/lib/monoterminal"))?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn ensure_dir_exists(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;

        // Set Unix permissions: 0755 (rwxr-xr-x)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(path, permissions)?;
        }
    }

    Ok(())
}

/// Ensure file has appropriate permissions (Unix: 0644)
///
/// Sets file permissions to 0644 (rw-r--r--) on Unix platforms.
/// No-op on Windows.
///
/// # Arguments
///
/// * `path` - The file path to set permissions on
///
/// # Errors
///
/// Returns an error if setting permissions fails.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_master::platform::paths::ensure_file_permissions;
/// use std::path::Path;
///
/// ensure_file_permissions(Path::new("/var/lib/monoterminal/sessions.db"))?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn ensure_file_permissions(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            let permissions = std::fs::Permissions::from_mode(0o644);
            std::fs::set_permissions(path, permissions)?;
        }
    }

    #[cfg(not(unix))]
    {
        // No-op on Windows
        let _ = path;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_returns_valid_path() {
        let dir = data_dir();
        assert!(!dir.as_os_str().is_empty(), "data_dir should not be empty");

        #[cfg(target_os = "linux")]
        assert_eq!(dir, PathBuf::from("/var/lib/monoterminal"));

        #[cfg(target_os = "macos")]
        assert_eq!(dir, PathBuf::from("/Library/Application Support/MONOTERMINAL"));

        #[cfg(windows)]
        assert!(
            dir.ends_with("MONOTERMINAL"),
            "Windows data_dir should end with MONOTERMINAL"
        );
    }

    #[test]
    fn test_user_data_dir_returns_valid_path() {
        let dir = user_data_dir();
        assert!(!dir.as_os_str().is_empty(), "user_data_dir should not be empty");

        #[cfg(target_os = "linux")]
        {
            // Should contain .local/share/monoterminal or XDG_DATA_HOME
            let dir_str = dir.to_string_lossy();
            assert!(
                dir_str.contains(".local/share/monoterminal") || dir_str.contains("monoterminal"),
                "Linux user_data_dir should contain .local/share/monoterminal or XDG path"
            );
        }

        #[cfg(target_os = "macos")]
        {
            let dir_str = dir.to_string_lossy();
            assert!(
                dir_str.contains("Library/Application Support/MONOTERMINAL"),
                "macOS user_data_dir should contain Library/Application Support/MONOTERMINAL"
            );
        }

        #[cfg(windows)]
        {
            assert!(
                dir.ends_with("MONOTERMINAL"),
                "Windows user_data_dir should end with MONOTERMINAL"
            );
        }
    }

    #[test]
    fn test_log_dir_returns_valid_path() {
        let dir = log_dir();
        assert!(!dir.as_os_str().is_empty(), "log_dir should not be empty");

        #[cfg(target_os = "linux")]
        {
            let dir_str = dir.to_string_lossy();
            assert!(
                dir_str.contains("logs") || dir_str.contains("/var/log"),
                "Linux log_dir should contain logs directory"
            );
        }

        #[cfg(target_os = "macos")]
        {
            let dir_str = dir.to_string_lossy();
            assert!(
                dir_str.contains("Library/Logs/MONOTERMINAL"),
                "macOS log_dir should contain Library/Logs/MONOTERMINAL"
            );
        }

        #[cfg(windows)]
        {
            assert!(
                dir.ends_with("logs"),
                "Windows log_dir should end with logs"
            );
        }
    }

    #[test]
    fn test_session_db_path_returns_valid_path() {
        let path = session_db_path();
        assert!(!path.as_os_str().is_empty(), "session_db_path should not be empty");
        assert!(
            path.ends_with("sessions.db"),
            "session_db_path should end with sessions.db"
        );
    }

    #[test]
    fn test_ensure_dir_exists_creates_directory() {
        let temp_dir = std::env::temp_dir().join("monoterminal_test_dir_creation");

        // Clean up if exists from previous test
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Should create directory
        ensure_dir_exists(&temp_dir).expect("Failed to create directory");
        assert!(temp_dir.exists(), "Directory should exist after ensure_dir_exists");

        // Should not error if directory already exists
        ensure_dir_exists(&temp_dir).expect("Failed on existing directory");

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean up test directory");
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_dir_exists_sets_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join("monoterminal_test_permissions");

        // Clean up if exists
        let _ = std::fs::remove_dir_all(&temp_dir);

        ensure_dir_exists(&temp_dir).expect("Failed to create directory");

        let metadata = std::fs::metadata(&temp_dir).expect("Failed to get metadata");
        let permissions = metadata.permissions();
        let mode = permissions.mode() & 0o777; // Extract permission bits

        assert_eq!(mode, 0o755, "Directory should have 0755 permissions");

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).expect("Failed to clean up test directory");
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_file_permissions_sets_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_file = std::env::temp_dir().join("monoterminal_test_file_permissions");

        // Create test file
        std::fs::write(&temp_file, b"test").expect("Failed to create test file");

        ensure_file_permissions(&temp_file).expect("Failed to set file permissions");

        let metadata = std::fs::metadata(&temp_file).expect("Failed to get metadata");
        let permissions = metadata.permissions();
        let mode = permissions.mode() & 0o777;

        assert_eq!(mode, 0o644, "File should have 0644 permissions");

        // Cleanup
        std::fs::remove_file(&temp_file).expect("Failed to clean up test file");
    }
}
