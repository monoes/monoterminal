// Week 1 Implementation: Per-Session Detection (§2.4.1)
//
// Core functionality:
// - walk_to_monomind: Walk upward from cwd to find .monomind/
// - should_suggest_install: Check if install suggestion is appropriate
// - dismiss_suggestion: Create dismiss marker file

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Walk upward from a starting directory to find .monomind/
///
/// Searches from the given path upward through parent directories until:
/// - A .monomind/ directory is found → returns Some(project_root)
/// - A .monomind-suggest-dismissed file is found → returns None
/// - The filesystem root is reached → returns None
///
/// # Arguments
///
/// * `start` - Starting directory path (typically PTY cwd)
///
/// # Returns
///
/// * `Ok(Some(path))` - Found .monomind/ at path
/// * `Ok(None)` - Not found or dismissed
/// * `Err(_)` - Filesystem error (permission denied, etc.)
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::walk_to_monomind;
/// use std::path::Path;
///
/// let result = walk_to_monomind(Path::new("/project/src/deep"))?;
/// match result {
///     Some(root) => println!("Found .monomind at: {}", root.display()),
///     None => println!("No .monomind found"),
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn walk_to_monomind(start: &Path) -> Result<Option<PathBuf>> {
    let mut current = start
        .canonicalize()
        .context("Failed to canonicalize starting path")?;

    loop {
        // Check for .monomind/ directory
        let monomind_dir = current.join(".monomind");
        if monomind_dir.exists() && monomind_dir.is_dir() {
            tracing::debug!(
                path = %current.display(),
                "Found .monomind directory"
            );
            return Ok(Some(current));
        }

        // Check for dismiss marker - if present, stop searching
        let dismiss_marker = current.join(".monomind-suggest-dismissed");
        if dismiss_marker.exists() {
            tracing::debug!(
                path = %current.display(),
                "Found .monomind-suggest-dismissed marker"
            );
            return Ok(None);
        }

        // Walk up to parent directory
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                // Reached filesystem root without finding .monomind
                tracing::trace!("Reached filesystem root, no .monomind found");
                return Ok(None);
            }
        }
    }
}

/// Check if install suggestion should be shown
///
/// Returns true if:
/// - No .monomind/ directory exists in the tree
/// - No .monomind-suggest-dismissed marker exists
///
/// This is a simpler check than walk_to_monomind - it only looks at the
/// immediate directory, not the entire tree. Use this when you've already
/// walked the tree and want to check the current level.
///
/// # Arguments
///
/// * `path` - Directory to check (typically project root or cwd)
///
/// # Returns
///
/// * `true` - Should suggest installation
/// * `false` - Should not suggest (already installed or dismissed)
pub fn should_suggest_install(path: &Path) -> bool {
    let monomind_dir = path.join(".monomind");
    let dismiss_file = path.join(".monomind-suggest-dismissed");

    !monomind_dir.exists() && !dismiss_file.exists()
}

/// Dismiss the install suggestion for a project
///
/// Creates a .monomind-suggest-dismissed marker file in the given directory.
/// This prevents the install suggestion from appearing again for this project.
///
/// The marker is a simple empty file that signals "user has seen and dismissed
/// the suggestion for this specific project."
///
/// # Arguments
///
/// * `project_root` - Project root directory
///
/// # Returns
///
/// * `Ok(())` - Marker created successfully
/// * `Err(_)` - Failed to create marker (permission denied, etc.)
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::dismiss_suggestion;
/// use std::path::Path;
///
/// dismiss_suggestion(Path::new("/project"))?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn dismiss_suggestion(project_root: &Path) -> Result<()> {
    let dismiss_file = project_root.join(".monomind-suggest-dismissed");

    std::fs::write(&dismiss_file, "")
        .context("Failed to create .monomind-suggest-dismissed marker")?;

    tracing::info!(
        path = %project_root.display(),
        "Created .monomind-suggest-dismissed marker"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Normalize paths for comparison on Windows
    /// Strips UNC prefix (\\?\) that canonicalize() adds on Windows
    #[cfg(windows)]
    fn normalize_path(path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            PathBuf::from(stripped)
        } else {
            path.to_path_buf()
        }
    }

    #[cfg(not(windows))]
    fn normalize_path(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    #[test]
    fn test_walk_to_monomind_found_at_root() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(project.join(".monomind")).unwrap();

        let result = walk_to_monomind(&project).unwrap();

        assert_eq!(
            result.as_ref().map(|p| normalize_path(p)),
            Some(normalize_path(&project))
        );
    }

    #[test]
    fn test_walk_to_monomind_found_in_parent() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let nested = project.join("src").join("deep").join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(project.join(".monomind")).unwrap();

        let result = walk_to_monomind(&nested).unwrap();

        assert_eq!(
            result.as_ref().map(|p| normalize_path(p)),
            Some(normalize_path(&project))
        );
    }

    #[test]
    fn test_walk_to_monomind_not_found() {
        let temp = TempDir::new().unwrap();
        // Create dismiss marker at temp root to prevent finding parent .monomind
        fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        let result = walk_to_monomind(&project).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_walk_to_monomind_dismissed() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let nested = project.join("src");
        fs::create_dir_all(&nested).unwrap();
        fs::write(project.join(".monomind-suggest-dismissed"), "").unwrap();

        let result = walk_to_monomind(&nested).unwrap();

        // Should return None when dismiss marker is found
        assert_eq!(result, None);
    }

    #[test]
    fn test_walk_to_monomind_dismissed_stops_at_marker() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().join("outer");
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();

        // Create .monomind in outer
        fs::create_dir(outer.join(".monomind")).unwrap();
        // Create dismiss marker in inner
        fs::write(inner.join(".monomind-suggest-dismissed"), "").unwrap();

        let result = walk_to_monomind(&inner).unwrap();

        // Should stop at dismiss marker, not find outer .monomind
        assert_eq!(result, None);
    }

    #[test]
    fn test_should_suggest_install_yes() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        assert!(should_suggest_install(&project));
    }

    #[test]
    fn test_should_suggest_install_no_already_installed() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(project.join(".monomind")).unwrap();

        assert!(!should_suggest_install(&project));
    }

    #[test]
    fn test_should_suggest_install_no_dismissed() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".monomind-suggest-dismissed"), "").unwrap();

        assert!(!should_suggest_install(&project));
    }

    #[test]
    fn test_dismiss_suggestion_creates_marker() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        dismiss_suggestion(&project).unwrap();

        let marker = project.join(".monomind-suggest-dismissed");
        assert!(marker.exists());
        assert!(marker.is_file());
    }

    #[test]
    fn test_dismiss_suggestion_idempotent() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        // Call twice - should not error
        dismiss_suggestion(&project).unwrap();
        dismiss_suggestion(&project).unwrap();

        let marker = project.join(".monomind-suggest-dismissed");
        assert!(marker.exists());
    }

    #[test]
    fn test_integration_detect_and_dismiss() {
        let temp = TempDir::new().unwrap();
        // Create dismiss marker at temp root to prevent finding parent .monomind
        fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
        let project = temp.path().join("project");
        let nested = project.join("src");
        fs::create_dir_all(&nested).unwrap();

        // Initially should suggest
        assert!(should_suggest_install(&project));
        let result = walk_to_monomind(&nested).unwrap();
        assert_eq!(result, None);

        // Dismiss
        dismiss_suggestion(&project).unwrap();

        // Should no longer suggest
        assert!(!should_suggest_install(&project));
        let result = walk_to_monomind(&nested).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_walk_stops_at_filesystem_boundary() {
        // This test walks from a deep temp directory up to root
        // Should return None without panicking
        let temp = TempDir::new().unwrap();
        // Create dismiss marker at temp root to prevent finding parent .monomind
        fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
        let deep = temp.path().join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();

        let result = walk_to_monomind(&deep).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_walk_handles_symlinks() {
        let temp = TempDir::new().unwrap();
        let real_dir = temp.path().join("real");
        fs::create_dir_all(&real_dir).unwrap();
        fs::create_dir(real_dir.join(".monomind")).unwrap();

        // Note: Symlink tests may fail on Windows without admin privileges
        #[cfg(unix)]
        {
            let link = temp.path().join("link");
            std::os::unix::fs::symlink(&real_dir, &link).unwrap();

            let result = walk_to_monomind(&link).unwrap();

            // Should resolve symlink and find .monomind
            assert!(result.is_some());
        }
    }
}
