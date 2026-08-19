// Integration tests for Unix PTY + SessionManager
// Phase 3 Week 1 Day 3
//
// Tests the full lifecycle:
// - Create session via SessionManager
// - Attach client
// - Write input
// - Read output
// - Resize
// - Terminate
// - Recovery from state

#![cfg(unix)]

use monoterminal_master::{
    pty::{PtyBackend, PtyConfig, UnixPtyBackend},
    session::manager::SessionManager,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Test: Create PTY session via SessionManager
#[tokio::test]
async fn test_session_manager_create_unix_pty() {
    let manager = SessionManager::new("/bin/sh".to_string(), None, None);

    let session_id = manager
        .create_session(Some(PathBuf::from("/tmp")), 24, 80)
        .await
        .expect("Failed to create session");

    // Verify session exists
    let sessions = manager.list_sessions();
    assert!(
        sessions.contains(&session_id),
        "Session should be listed after creation"
    );

    // Cleanup
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");
}

/// Test: Full lifecycle - create, attach, write, read, terminate
#[tokio::test]
async fn test_session_manager_full_lifecycle() {
    let manager = SessionManager::new("/bin/sh".to_string(), None, None);

    // Create session
    let session_id = manager
        .create_session(Some(PathBuf::from("/tmp")), 24, 80)
        .await
        .expect("Failed to create session");

    // Create output channel
    let (output_tx, mut output_rx) = mpsc::channel(100);

    // Attach client
    let client_id = manager
        .attach_client(&session_id, output_tx)
        .await
        .expect("Failed to attach client");

    // Write command
    manager
        .send_input(&session_id, b"echo hello\n")
        .await
        .expect("Failed to send input");

    // Read output (with timeout)
    let output = tokio::time::timeout(tokio::time::Duration::from_secs(2), output_rx.recv())
        .await
        .expect("Timeout waiting for output")
        .expect("No output received");

    // Verify output contains "hello" or "echo"
    let output_str = String::from_utf8_lossy(&output);
    assert!(
        output_str.contains("hello") || output_str.contains("echo"),
        "Expected output to contain 'hello' or 'echo', got: {}",
        output_str
    );

    // Detach client
    manager
        .detach_client(&session_id, &client_id)
        .await
        .expect("Failed to detach client");

    // Kill session
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");

    // Verify session removed
    let sessions = manager.list_sessions();
    assert!(
        !sessions.contains(&session_id),
        "Session should be removed after kill"
    );
}

/// Test: Resize operation through SessionManager
#[tokio::test]
async fn test_session_manager_resize() {
    let manager = SessionManager::new("/bin/sh".to_string(), None, None);

    let session_id = manager
        .create_session(Some(PathBuf::from("/tmp")), 24, 80)
        .await
        .expect("Failed to create session");

    // Resize should not error
    manager
        .resize_session(&session_id, 30, 100)
        .await
        .expect("Failed to resize session");

    manager
        .resize_session(&session_id, 40, 120)
        .await
        .expect("Failed to resize session again");

    // Cleanup
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");
}

/// Test: Multiple clients attached to same session
#[tokio::test]
async fn test_session_manager_multiple_clients() {
    let manager = SessionManager::new("/bin/sh".to_string(), None, None);

    let session_id = manager
        .create_session(Some(PathBuf::from("/tmp")), 24, 80)
        .await
        .expect("Failed to create session");

    // Attach two clients
    let (output_tx1, mut output_rx1) = mpsc::channel(100);
    let (output_tx2, mut output_rx2) = mpsc::channel(100);

    let client_id1 = manager
        .attach_client(&session_id, output_tx1)
        .await
        .expect("Failed to attach client 1");

    let client_id2 = manager
        .attach_client(&session_id, output_tx2)
        .await
        .expect("Failed to attach client 2");

    // Write command
    manager
        .send_input(&session_id, b"echo test\n")
        .await
        .expect("Failed to send input");

    // Both clients should receive output
    let timeout = tokio::time::Duration::from_secs(2);

    let output1 = tokio::time::timeout(timeout, output_rx1.recv())
        .await
        .expect("Timeout on client 1")
        .expect("No output on client 1");

    let output2 = tokio::time::timeout(timeout, output_rx2.recv())
        .await
        .expect("Timeout on client 2")
        .expect("No output on client 2");

    // Both should see the same output
    assert_eq!(
        output1, output2,
        "Both clients should receive identical output"
    );

    // Cleanup
    manager
        .detach_client(&session_id, &client_id1)
        .await
        .expect("Failed to detach client 1");
    manager
        .detach_client(&session_id, &client_id2)
        .await
        .expect("Failed to detach client 2");
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");
}

/// Test: Session persistence integration (if database available)
#[tokio::test]
async fn test_session_manager_with_persistence() {
    // This test requires persistence layer integration
    // For now, verify basic creation works with None database
    let manager = SessionManager::new("/bin/sh".to_string(), None, None);

    let session_id = manager
        .create_session_with_user(
            Some("test-user-id".to_string()),
            Some(PathBuf::from("/tmp")),
            24,
            80,
        )
        .await
        .expect("Failed to create session with user");

    // Verify session exists
    let sessions = manager.list_sessions();
    assert!(sessions.contains(&session_id));

    // Cleanup
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");
}

/// Test: Environment variable propagation
#[tokio::test]
async fn test_unix_pty_environment_via_session_manager() {
    // Create SessionManager with custom environment
    let mut env = HashMap::new();
    env.insert("TEST_VAR".to_string(), "test_value".to_string());

    // Note: SessionManager doesn't currently expose custom env API
    // This test verifies the PTY backend itself handles env correctly
    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: env,
    };

    let mut pty = UnixPtyBackend::create(config)
        .await
        .expect("Failed to create PTY with custom env");

    // Write command to check env var
    pty.write(b"echo $TEST_VAR\n")
        .await
        .expect("Failed to write");

    // Read output
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut buf = vec![0u8; 1024];
    let n = pty.read(&mut buf).await.expect("Failed to read");

    let output = String::from_utf8_lossy(&buf[..n]);
    assert!(
        output.contains("test_value"),
        "Expected environment variable to be set, got: {}",
        output
    );

    // Cleanup
    Box::new(pty)
        .terminate()
        .await
        .expect("Failed to terminate");
}

/// Test: Working directory propagation
#[tokio::test]
async fn test_unix_pty_working_dir_via_session_manager() {
    let config = PtyConfig {
        rows: 24,
        cols: 80,
        shell: "/bin/sh".to_string(),
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
    };

    let mut pty = UnixPtyBackend::create(config)
        .await
        .expect("Failed to create PTY");

    // Check working directory
    pty.write(b"pwd\n").await.expect("Failed to write");

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let mut buf = vec![0u8; 1024];
    let n = pty.read(&mut buf).await.expect("Failed to read");

    let output = String::from_utf8_lossy(&buf[..n]);
    assert!(
        output.contains("/tmp"),
        "Expected working directory to be /tmp, got: {}",
        output
    );

    // Cleanup
    Box::new(pty)
        .terminate()
        .await
        .expect("Failed to terminate");
}

/// Test: Concurrent operations (write + resize)
#[tokio::test]
async fn test_session_manager_concurrent_operations() {
    let manager = Arc::new(SessionManager::new("/bin/sh".to_string(), None, None));

    let session_id = manager
        .create_session(Some(PathBuf::from("/tmp")), 24, 80)
        .await
        .expect("Failed to create session");

    let (output_tx, _output_rx) = mpsc::channel(100);
    let _client_id = manager
        .attach_client(&session_id, output_tx)
        .await
        .expect("Failed to attach client");

    // Spawn concurrent operations
    let manager1 = manager.clone();
    let session_id1 = session_id.clone();
    let write_task = tokio::spawn(async move {
        for i in 0..10 {
            manager1
                .send_input(&session_id1, format!("echo test{}\n", i).as_bytes())
                .await
                .expect("Failed to send input");
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });

    let manager2 = manager.clone();
    let session_id2 = session_id.clone();
    let resize_task = tokio::spawn(async move {
        for i in 0..10 {
            manager2
                .resize_session(&session_id2, 24 + i, 80 + i)
                .await
                .expect("Failed to resize");
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });

    // Wait for both to complete
    write_task.await.expect("Write task failed");
    resize_task.await.expect("Resize task failed");

    // Cleanup
    manager
        .kill_session(&session_id)
        .await
        .expect("Failed to kill session");
}
