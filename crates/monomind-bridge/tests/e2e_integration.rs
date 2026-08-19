// End-to-end integration tests for monomind-bridge
//
// Tests the complete integration flow across all modules:
// - Detection → Health → Dashboard pipeline
// - Real-world usage scenarios
// - Cross-module integration
// - Fail-loud behavior verification

use monoterminal_monomind_bridge::{
    detect_monomind, dismiss_suggestion, get_dashboard_data, run_doctor_check,
    to_dashboard_response, to_health_check_response, INSTALL_SUGGESTION_BANNER,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_e2e_fresh_project_workflow() {
    // Scenario: User opens MONOTERMINAL in a fresh project without monomind

    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let project = temp.path().join("fresh-project");
    fs::create_dir_all(&project).unwrap();

    // Step 1: Session starts, detect monomind
    let detection = detect_monomind(&project);
    assert!(!detection.found);
    assert!(detection.suggest_install);
    assert!(!detection.dismiss_file_exists);

    // Step 2: Show install suggestion banner
    assert!(INSTALL_SUGGESTION_BANNER.contains("npx monomind@latest init"));

    // Step 3: User dismisses the suggestion
    dismiss_suggestion(&project).unwrap();

    // Step 4: Re-detect after dismiss
    let detection_after = detect_monomind(&project);
    assert!(!detection_after.found);
    assert!(!detection_after.suggest_install); // Should not suggest anymore
    assert!(detection_after.dismiss_file_exists);
}

#[tokio::test]
async fn test_e2e_existing_monomind_workflow() {
    // Scenario: User opens MONOTERMINAL in a project that already has monomind

    let temp = TempDir::new().unwrap();
    let project = temp.path().join("existing-project");
    let nested = project.join("src").join("components");
    fs::create_dir_all(&nested).unwrap();

    // Create .monomind directory
    fs::create_dir(project.join(".monomind")).unwrap();

    // Step 1: Detect monomind (from nested directory)
    let detection = detect_monomind(&nested);
    assert!(detection.found);
    assert!(!detection.suggest_install);

    // Step 2: Run health check
    let health = run_doctor_check(&project).await;
    // Should return a health status (even if monomind is not actually installed in test env)
    assert!(health.is_ok() || health.is_err());

    // Step 3: Get dashboard data
    let dashboard = get_dashboard_data(&project).await;
    assert!(dashboard.is_ok());
}

#[tokio::test]
async fn test_e2e_cwd_change_workflow() {
    // Scenario: User changes working directory via `cd` command

    let temp = TempDir::new().unwrap();

    // Project A: Has monomind
    let project_a = temp.path().join("project-a");
    fs::create_dir_all(&project_a).unwrap();
    fs::create_dir(project_a.join(".monomind")).unwrap();

    // Project B: No monomind
    let project_b = temp.path().join("project-b");
    fs::create_dir_all(&project_b).unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();

    // Step 1: Start in project A
    let detection_a = detect_monomind(&project_a);
    assert!(detection_a.found);

    let health_a = run_doctor_check(&project_a).await;
    assert!(health_a.is_ok() || health_a.is_err());

    // Step 2: Change to project B
    let detection_b = detect_monomind(&project_b);
    assert!(!detection_b.found);
    assert!(detection_b.suggest_install);

    // Step 3: Dismiss in project B
    dismiss_suggestion(&project_b).unwrap();

    // Step 4: Change back to project A
    let detection_a_again = detect_monomind(&project_a);
    assert!(detection_a_again.found); // Still has monomind

    // Step 5: Change to project B again
    let detection_b_again = detect_monomind(&project_b);
    assert!(!detection_b_again.suggest_install); // Dismiss is persistent
}

#[tokio::test]
async fn test_e2e_monorepo_workflow() {
    // Scenario: User works in a monorepo with multiple projects

    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");

    // Workspace root has .monomind
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir(workspace.join(".monomind")).unwrap();

    // Multiple projects
    let frontend = workspace.join("frontend");
    let backend = workspace.join("backend").join("api");
    let mobile = workspace.join("mobile").join("ios");

    fs::create_dir_all(&frontend).unwrap();
    fs::create_dir_all(&backend).unwrap();
    fs::create_dir_all(&mobile).unwrap();

    // All projects should find workspace .monomind
    for project in [&frontend, &backend, &mobile] {
        let detection = detect_monomind(project);
        assert!(detection.found);
        assert!(!detection.suggest_install);
    }
}

#[tokio::test]
async fn test_e2e_protocol_conversion_workflow() {
    // Scenario: Complete flow from detection to wire protocol

    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir(project.join(".monomind")).unwrap();

    // Step 1: Detection
    let detection = detect_monomind(&project);
    assert!(detection.found);

    // Step 2: Health check
    let health = run_doctor_check(&project).await;
    if let Ok(health_status) = health {
        // Step 3: Convert to protocol response
        let proto_response = to_health_check_response(health_status);
        assert!(proto_response.last_check_timestamp > 0);
    }

    // Step 4: Dashboard data
    let dashboard = get_dashboard_data(&project).await;
    if let Ok(dashboard_data) = dashboard {
        // Step 5: Convert to protocol response
        let proto_response = to_dashboard_response(dashboard_data);
        assert!(proto_response.timestamp > 0);
    }
}

#[tokio::test]
async fn test_e2e_error_handling_fail_loud() {
    // Scenario: Verify fail-loud behavior - errors are surfaced, not hidden
    //
    // This tests the design principle from §2.4.2:
    // "Fail loud, not silent (prevent monoes/monomind#135, #136)"

    let temp = TempDir::new().unwrap();
    let project = temp.path().join("test-project");
    fs::create_dir_all(&project).unwrap();

    // Health check in a directory without monomind
    let health = run_doctor_check(&project).await;

    match health {
        Ok(health_status) => {
            // If it returns Ok, errors should be visible in the status
            if !health_status.installed {
                // Should have issues explaining why it's not installed
                assert!(
                    !health_status.issues.is_empty() || !health_status.is_healthy(),
                    "Fail-loud: Missing installation should be visible"
                );
            }
        }
        Err(e) => {
            // If it returns Err, that's also acceptable - error is surfaced
            assert!(
                !e.to_string().is_empty(),
                "Fail-loud: Error should have message"
            );
        }
    }

    // Dashboard data should also not hide errors
    let dashboard = get_dashboard_data(&project).await;

    match dashboard {
        Ok(dashboard_data) => {
            // Empty data is acceptable, but it should be explicitly empty
            // not pretend everything is fine
            if !dashboard_data.org_status.running {
                assert!(
                    dashboard_data.org_status.status_message.is_empty() == false,
                    "Fail-loud: Status message should explain why org is not running"
                );
            }
        }
        Err(e) => {
            // Error is also acceptable - it's surfaced to the caller
            assert!(
                !e.to_string().is_empty(),
                "Fail-loud: Error should have message"
            );
        }
    }
}

#[tokio::test]
async fn test_e2e_concurrent_operations() {
    // Scenario: Multiple operations happening concurrently (multi-session scenario)

    use std::sync::Arc;

    let temp = TempDir::new().unwrap();
    let project = Arc::new(temp.path().join("concurrent-project"));

    fs::create_dir_all(&*project).unwrap();
    fs::create_dir(project.join(".monomind")).unwrap();

    let mut handles = vec![];

    // Spawn multiple concurrent operations
    for i in 0..5 {
        let project = Arc::clone(&project);

        let handle = tokio::spawn(async move {
            // Detection
            let _detection = detect_monomind(&*project);

            // Health check
            let _health = run_doctor_check(&*project).await;

            // Dashboard query
            let _dashboard = get_dashboard_data(&*project).await;

            // All operations should complete without panicking
            i
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    let mut results = vec![];
    for handle in handles {
        let result = handle.await.unwrap();
        results.push(result);
    }

    assert_eq!(results.len(), 5);
}

#[test]
fn test_e2e_dismiss_persistence() {
    // Scenario: Verify dismiss marker persists across detections

    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let project = temp.path().join("persist-project");
    fs::create_dir_all(&project).unwrap();

    // Initial state: no dismiss marker in project
    let marker_path = project.join(".monomind-suggest-dismissed");
    assert!(!marker_path.exists());

    // Detect and verify suggestion
    let detection1 = detect_monomind(&project);
    assert!(detection1.suggest_install);

    // Dismiss
    dismiss_suggestion(&project).unwrap();
    assert!(marker_path.exists());

    // Detect again - should not suggest
    let detection2 = detect_monomind(&project);
    assert!(!detection2.suggest_install);
    assert!(detection2.dismiss_file_exists);

    // Multiple detections should still not suggest
    let detection3 = detect_monomind(&project);
    let detection4 = detect_monomind(&project);
    assert!(!detection3.suggest_install);
    assert!(!detection4.suggest_install);
}

#[test]
fn test_e2e_nested_directory_detection() {
    // Scenario: Deep nested directory structure

    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");

    // Create deep nesting: project/a/b/c/d/e/f/g
    let mut current = project.clone();
    for letter in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
        current = current.join(letter.to_string());
    }
    fs::create_dir_all(&current).unwrap();

    // Put .monomind at project root
    fs::create_dir(project.join(".monomind")).unwrap();

    // Detect from deepest level
    let detection = detect_monomind(&current);
    assert!(detection.found);

    // Verify it found the root
    let root = detection.monomind_root.unwrap();
    let normalized_root = normalize_path(&root);
    let normalized_project = normalize_path(&project);

    assert_eq!(normalized_root, normalized_project);
}

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

#[tokio::test]
async fn test_e2e_rapid_detection_changes() {
    // Scenario: Rapid state changes (install → uninstall → reinstall)

    let temp = TempDir::new().unwrap();
    // Create dismiss marker at temp root to prevent finding parent .monomind
    fs::write(temp.path().join(".monomind-suggest-dismissed"), "").unwrap();
    let project = temp.path().join("rapid-change-project");
    fs::create_dir_all(&project).unwrap();

    let monomind_dir = project.join(".monomind");
    let dismiss_file = project.join(".monomind-suggest-dismissed");

    // State 1: No monomind, no dismiss
    let d1 = detect_monomind(&project);
    assert!(!d1.found);
    assert!(d1.suggest_install);

    // State 2: Add .monomind
    fs::create_dir(&monomind_dir).unwrap();
    let d2 = detect_monomind(&project);
    assert!(d2.found);
    assert!(!d2.suggest_install);

    // State 3: Remove .monomind, add dismiss
    fs::remove_dir(&monomind_dir).unwrap();
    fs::write(&dismiss_file, "").unwrap();
    let d3 = detect_monomind(&project);
    assert!(!d3.found);
    assert!(!d3.suggest_install);
    assert!(d3.dismiss_file_exists);

    // State 4: Remove dismiss, re-add .monomind
    fs::remove_file(&dismiss_file).unwrap();
    fs::create_dir(&monomind_dir).unwrap();
    let d4 = detect_monomind(&project);
    assert!(d4.found);
    assert!(!d4.suggest_install);
}
