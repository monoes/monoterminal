// Monomind Integration Bridge for MONOTERMINAL
// See: docs/monoterminal-srs.md §2.4, docs/monomind-bridge-design.md
//
// Implementation Status:
// ✅ Week 1: Core Detection (§2.4.1) - COMPLETE
// ✅ Week 2: Health Check & Upgrade (§2.4.3) - COMPLETE
// ✅ Week 4: Embedded Dashboard API (§2.4.2) - COMPLETE
//
// Responsibilities:
// - Per-session .monomind/ detection (§2.4.1)
// - Embedded dashboard API (§2.4.2)
// - Health check & upgrade (§2.4.3)

use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

// Module declarations
mod detection;
mod health;
mod dashboard;

// Re-export detection functions
pub use detection::{walk_to_monomind, should_suggest_install, dismiss_suggestion};

// Re-export health types and functions
pub use health::{
    run_doctor_check, upgrade_monomind, HealthScheduler, HealthStatus, HealthIssue,
    Severity, UpgradeResult,
};

// Re-export dashboard types and functions
pub use dashboard::{
    get_dashboard_data, DashboardData, OrgStatus, AgentInfo, RunInfo, MemoryStats,
};

/// Detection result from per-session monomind check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionResult {
    /// Whether .monomind/ was found in the directory tree
    pub found: bool,
    /// Root directory containing .monomind/ (if found)
    pub monomind_root: Option<PathBuf>,
    /// Whether to suggest installation to the user
    pub suggest_install: bool,
    /// Whether the dismiss marker exists
    pub dismiss_file_exists: bool,
}

impl DetectionResult {
    /// Create a new detection result indicating monomind was found
    pub fn found(root: PathBuf) -> Self {
        Self {
            found: true,
            monomind_root: Some(root),
            suggest_install: false,
            dismiss_file_exists: false,
        }
    }

    /// Create a new detection result indicating monomind was not found
    pub fn not_found(suggest: bool, dismissed: bool) -> Self {
        Self {
            found: false,
            monomind_root: None,
            suggest_install: suggest,
            dismiss_file_exists: dismissed,
        }
    }
}

/// Install suggestion banner text (MOTD-style)
pub const INSTALL_SUGGESTION_BANNER: &str = r#"
╭─────────────────────────────────────────────────────────────╮
│ This project doesn't have monomind installed               │
│                                                             │
│ Install to unlock org/agent/swarm features:                │
│   npx monomind@latest init                                 │
│                                                             │
│ Dismiss: touch .monomind-suggest-dismissed                 │
╰─────────────────────────────────────────────────────────────╯
"#;

/// Detect monomind in a directory tree (Week 1 implementation)
///
/// Walks upward from the given path to find .monomind/ directory.
/// Returns detection result with installation suggestion if appropriate.
///
/// # Examples
///
/// ```no_run
/// use monoterminal_monomind_bridge::detect_monomind;
/// use std::path::Path;
///
/// let result = detect_monomind(Path::new("/project/src/deep"));
/// if result.suggest_install {
///     println!("Suggest installing monomind");
/// }
/// ```
pub fn detect_monomind(path: &Path) -> DetectionResult {
    match walk_to_monomind(path) {
        Ok(Some(root)) => DetectionResult::found(root),
        Ok(None) => {
            let suggest = should_suggest_install(path);
            let dismissed = path.join(".monomind-suggest-dismissed").exists();
            DetectionResult::not_found(suggest, dismissed)
        }
        Err(_) => {
            // On error (e.g., permission denied), don't suggest installation
            DetectionResult::not_found(false, false)
        }
    }
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
    fn test_detection_result_found() {
        let root = PathBuf::from("/project");
        let result = DetectionResult::found(root.clone());

        assert!(result.found);
        assert_eq!(result.monomind_root, Some(root));
        assert!(!result.suggest_install);
        assert!(!result.dismiss_file_exists);
    }

    #[test]
    fn test_detection_result_not_found() {
        let result = DetectionResult::not_found(true, false);

        assert!(!result.found);
        assert_eq!(result.monomind_root, None);
        assert!(result.suggest_install);
        assert!(!result.dismiss_file_exists);
    }

    #[test]
    fn test_detect_monomind_found() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let nested = project.join("src").join("deep");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(project.join(".monomind")).unwrap();

        let result = detect_monomind(&nested);

        assert!(result.found);
        assert_eq!(
            result.monomind_root.as_ref().map(|p| normalize_path(p)),
            Some(normalize_path(&project))
        );
        assert!(!result.suggest_install);
    }

    #[test]
    fn test_detect_monomind_not_found() {
        let temp = TempDir::new().unwrap();
        // Create dismiss marker at temp root to prevent finding parent .monomind
        fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();

        let result = detect_monomind(&project);

        assert!(!result.found);
        assert_eq!(result.monomind_root, None);
        assert!(result.suggest_install);
        assert!(!result.dismiss_file_exists);
    }

    #[test]
    fn test_detect_monomind_dismissed() {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".monomind-suggest-dismissed"), "").unwrap();

        let result = detect_monomind(&project);

        assert!(!result.found);
        assert!(!result.suggest_install);
        assert!(result.dismiss_file_exists);
    }
}
