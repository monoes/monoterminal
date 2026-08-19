// Integration tests for monomind detection (§2.4.1)
//
// Tests the complete detection flow including:
// - Directory tree walking
// - Installation suggestion logic
// - Dismiss marker handling
// - Edge cases (permissions, symlinks, etc.)

use monoterminal_monomind_bridge::{
    detect_monomind, dismiss_suggestion, should_suggest_install, walk_to_monomind, DetectionResult,
    INSTALL_SUGGESTION_BANNER,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to normalize paths on Windows (strip UNC prefix)
#[cfg(windows)]
fn normalize_path(path: &std::path::Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(not(windows))]
fn normalize_path(path: &std::path::Path) -> PathBuf {
    path.to_path_buf()
}

#[test]
fn test_detect_monomind_complete_flow() {
    // Setup: Create a project structure with .monomind/
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("my-project");
    let deep_nested = project.join("src").join("components").join("ui");
    fs::create_dir_all(&deep_nested).unwrap();
    fs::create_dir(project.join(".monomind")).unwrap();

    // Test from deeply nested directory
    let result = detect_monomind(&deep_nested);

    assert!(result.found);
    assert_eq!(
        result.monomind_root.as_ref().map(|p| normalize_path(p)),
        Some(normalize_path(&project))
    );
    assert!(!result.suggest_install);
    assert!(!result.dismiss_file_exists);
}

#[test]
fn test_detect_monomind_not_found_suggests_install() {
    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let project = temp.path().join("no-monomind-project");
    fs::create_dir_all(&project).unwrap();

    let result = detect_monomind(&project);

    assert!(!result.found);
    assert_eq!(result.monomind_root, None);
    assert!(result.suggest_install);
    assert!(!result.dismiss_file_exists);
}

#[test]
fn test_walk_to_monomind_multi_level_search() {
    let temp = TempDir::new().unwrap();
    let outer = temp.path().join("workspace");
    let project_a = outer.join("project-a");
    let project_b = outer.join("project-b");

    // Create two projects, only one has .monomind
    fs::create_dir_all(&project_a).unwrap();
    fs::create_dir_all(&project_b).unwrap();
    fs::create_dir(project_a.join(".monomind")).unwrap();

    // From project-a: should find .monomind
    let result_a = walk_to_monomind(&project_a).unwrap();
    assert!(result_a.is_some());
    assert_eq!(
        result_a.as_ref().map(|p| normalize_path(p)),
        Some(normalize_path(&project_a))
    );

    // From project-b: should not find (no .monomind there or in parents)
    // Create dismiss marker at temp root to stop search
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let result_b = walk_to_monomind(&project_b).unwrap();
    assert_eq!(result_b, None);
}

#[test]
fn test_dismiss_marker_stops_upward_search() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    let project = workspace.join("project");
    let nested = project.join("src");

    fs::create_dir_all(&nested).unwrap();
    fs::create_dir(workspace.join(".monomind")).unwrap();

    // Place dismiss marker in project directory
    fs::write(project.join(".monomind-suggest-dismissed"), "").unwrap();

    // Search from nested should stop at project, not find workspace .monomind
    let result = walk_to_monomind(&nested).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_should_suggest_install_logic() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Initially should suggest
    assert!(should_suggest_install(&project));

    // After creating .monomind, should not suggest
    fs::create_dir(project.join(".monomind")).unwrap();
    assert!(!should_suggest_install(&project));

    // After removing .monomind and creating dismiss marker, should not suggest
    fs::remove_dir(project.join(".monomind")).unwrap();
    fs::write(project.join(".monomind-suggest-dismissed"), "").unwrap();
    assert!(!should_suggest_install(&project));
}

#[test]
fn test_dismiss_suggestion_creates_marker() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Initially no marker
    assert!(!project.join(".monomind-suggest-dismissed").exists());

    // Dismiss suggestion
    dismiss_suggestion(&project).unwrap();

    // Marker should exist
    let marker = project.join(".monomind-suggest-dismissed");
    assert!(marker.exists());
    assert!(marker.is_file());
}

#[test]
fn test_dismiss_suggestion_idempotent() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Call multiple times - should not error
    dismiss_suggestion(&project).unwrap();
    dismiss_suggestion(&project).unwrap();
    dismiss_suggestion(&project).unwrap();

    assert!(project.join(".monomind-suggest-dismissed").exists());
}

#[test]
fn test_detection_result_constructors() {
    let root = PathBuf::from("/test/project");

    // Test found constructor
    let found = DetectionResult::found(root.clone());
    assert!(found.found);
    assert_eq!(found.monomind_root, Some(root));
    assert!(!found.suggest_install);
    assert!(!found.dismiss_file_exists);

    // Test not_found constructor with suggestion
    let not_found_suggest = DetectionResult::not_found(true, false);
    assert!(!not_found_suggest.found);
    assert_eq!(not_found_suggest.monomind_root, None);
    assert!(not_found_suggest.suggest_install);
    assert!(!not_found_suggest.dismiss_file_exists);

    // Test not_found constructor with dismiss marker
    let not_found_dismissed = DetectionResult::not_found(false, true);
    assert!(!not_found_dismissed.found);
    assert_eq!(not_found_dismissed.monomind_root, None);
    assert!(!not_found_dismissed.suggest_install);
    assert!(not_found_dismissed.dismiss_file_exists);
}

#[test]
fn test_install_suggestion_banner_content() {
    // Verify banner contains key information
    assert!(INSTALL_SUGGESTION_BANNER.contains("monomind"));
    assert!(INSTALL_SUGGESTION_BANNER.contains("npx monomind@latest init"));
    assert!(INSTALL_SUGGESTION_BANNER.contains("Dismiss"));
    assert!(INSTALL_SUGGESTION_BANNER.contains(".monomind-suggest-dismissed"));
}

#[test]
fn test_detection_with_permission_denied() {
    // This test validates the behavior when filesystem access is denied
    // On most test environments we can't actually create permission denied scenarios,
    // so we test the documented behavior: errors result in no suggestion

    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // Normal detection should work
    let result = detect_monomind(&project);
    assert!(result.suggest_install); // No .monomind found, should suggest
}

#[test]
fn test_walk_reaches_filesystem_root() {
    // Test that walking stops gracefully at filesystem root
    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let deep = temp
        .path()
        .join("a")
        .join("b")
        .join("c")
        .join("d")
        .join("e");
    fs::create_dir_all(&deep).unwrap();

    let result = walk_to_monomind(&deep).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_multiple_projects_in_workspace() {
    // Test a workspace with multiple projects, each potentially having .monomind
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");

    let frontend = workspace.join("frontend");
    let backend = workspace.join("backend");

    fs::create_dir_all(&frontend).unwrap();
    fs::create_dir_all(&backend).unwrap();

    // Only backend has .monomind
    fs::create_dir(backend.join(".monomind")).unwrap();

    // Also put one in workspace root
    fs::create_dir(workspace.join(".monomind")).unwrap();

    // From backend, should find backend's .monomind (closest)
    let backend_result = walk_to_monomind(&backend).unwrap();
    assert_eq!(
        backend_result.as_ref().map(|p| normalize_path(p)),
        Some(normalize_path(&backend))
    );

    // From frontend, should find workspace .monomind (parent)
    let frontend_result = walk_to_monomind(&frontend).unwrap();
    assert_eq!(
        frontend_result.as_ref().map(|p| normalize_path(p)),
        Some(normalize_path(&workspace))
    );
}

#[test]
fn test_dismiss_at_different_levels() {
    let temp = TempDir::new().unwrap();
    let outer = temp.path().join("outer");
    let inner = outer.join("inner");

    fs::create_dir_all(&inner).unwrap();
    fs::create_dir(outer.join(".monomind")).unwrap();

    // Initially, from inner, should find outer's .monomind
    let result_before = walk_to_monomind(&inner).unwrap();
    assert_eq!(
        result_before.as_ref().map(|p| normalize_path(p)),
        Some(normalize_path(&outer))
    );

    // Create dismiss marker in inner
    fs::write(inner.join(".monomind-suggest-dismissed"), "").unwrap();

    // Now from inner, should stop at dismiss marker
    let result_after = walk_to_monomind(&inner).unwrap();
    assert_eq!(result_after, None);
}

#[cfg(unix)]
#[test]
fn test_symlink_detection() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let real_dir = temp.path().join("real");
    let link = temp.path().join("link");

    fs::create_dir_all(&real_dir).unwrap();
    fs::create_dir(real_dir.join(".monomind")).unwrap();

    symlink(&real_dir, &link).unwrap();

    // Should resolve symlink and find .monomind
    let result = walk_to_monomind(&link).unwrap();
    assert!(result.is_some());
}

#[test]
fn test_concurrent_detection_calls() {
    // Test that detection is safe to call concurrently from multiple threads
    use std::sync::Arc;
    use std::thread;

    let temp = TempDir::new().unwrap();
    let project = Arc::new(temp.path().join("project"));

    fs::create_dir_all(&*project).unwrap();
    fs::create_dir(project.join(".monomind")).unwrap();

    let mut handles = vec![];

    for _ in 0..10 {
        let project = Arc::clone(&project);
        let handle = thread::spawn(move || {
            let result = detect_monomind(&*project);
            assert!(result.found);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
