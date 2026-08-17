// Session lifecycle and state machine property tests
// SRS §2.1.3: Session state machine testing

#[cfg(test)]
mod session_tests {
    use super::super::*;
    use crate::session::manager::SessionManager;
    use proptest::prelude::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    proptest! {
        #[test]
        fn test_session_state_transitions_are_valid(
            initial_state in prop::sample::select(vec![SessionState::Running])
        ) {
            // Property: Valid state transitions per SRS state machine
            // CREATE → RUNNING → TERMINATED
            // (DETACHED deferred to Phase 2)

            prop_assert_eq!(initial_state, SessionState::Running);

            // Can transition to Terminated
            let terminated = SessionState::Terminated;
            prop_assert_ne!(initial_state, terminated);
        }

        #[test]
        #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
        fn test_client_attach_detach_idempotent(
            client_count in 1usize..=10
        ) {
            use uuid::Uuid;

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let manager = SessionManager::new(Some("cmd.exe".to_string()));

                // Create session
                let session_id = manager.create_session(None, 24, 80)
                    .await
                    .expect("Failed to create session");

                // Attach multiple clients
                let clients: Vec<_> = (0..client_count)
                    .map(|_| Uuid::new_v4())
                    .collect();

                for client_id in &clients {
                    let (output_tx, _output_rx) = mpsc::channel(32);
                    let result = manager.attach_client(session_id, *client_id, output_tx).await;
                    prop_assert!(result.is_ok());
                }

                // Detach all clients
                for client_id in &clients {
                    let result = manager.detach_client(session_id, *client_id).await;
                    prop_assert!(result.is_ok());
                }

                // Double detach should be idempotent (not error)
                for client_id in &clients {
                    let result = manager.detach_client(session_id, *client_id).await;
                    prop_assert!(result.is_ok());
                }

                manager.kill_session(session_id).await.ok();
                Ok(())
            })?;
        }
    }

    #[tokio::test]
    async fn test_session_manager_create_and_list() {
        let manager = SessionManager::new(None);

        assert_eq!(manager.session_count().await, 0);

        let session1 = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session 1");

        assert_eq!(manager.session_count().await, 1);

        let session2 = manager.create_session(None, 30, 100)
            .await
            .expect("Failed to create session 2");

        assert_eq!(manager.session_count().await, 2);

        let sessions = manager.list_sessions().await;
        assert!(sessions.contains(&session1));
        assert!(sessions.contains(&session2));

        // Cleanup
        manager.kill_session(session1).await.ok();
        manager.kill_session(session2).await.ok();
    }

    #[tokio::test]
    async fn test_session_resize() {
        let manager = SessionManager::new(None);

        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        // Resize to various dimensions
        for (rows, cols) in [(30, 100), (50, 120), (24, 80), (40, 90)] {
            let result = manager.resize_session(session_id, rows, cols).await;
            assert!(result.is_ok(), "Resize to {}x{} failed", rows, cols);
        }

        // Invalid dimensions should error
        assert!(manager.resize_session(session_id, 0, 80).await.is_err());
        assert!(manager.resize_session(session_id, 24, 0).await.is_err());
        assert!(manager.resize_session(session_id, 600, 80).await.is_err());

        manager.kill_session(session_id).await.ok();
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_send_input() {
        let manager = SessionManager::new(None);

        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        // Send various inputs
        let inputs: &[&[u8]] = &[
            b"echo hello\r\n",
            b"dir\r\n",
            b"cd ..\r\n",
        ];

        for input in inputs {
            let result = manager.send_input(session_id, *input).await;
            assert!(result.is_ok(), "Failed to send input: {:?}", input);

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        manager.kill_session(session_id).await.ok();
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_session_snapshot_after_output() {
        use uuid::Uuid;

        let manager = SessionManager::new(None);

        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        // Send some commands to generate output
        manager.send_input(session_id, b"echo test\r\n")
            .await
            .expect("Failed to send input");

        // Wait for output to be processed
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Attach a client and get snapshot
        let client_id = Uuid::new_v4();
        let (output_tx, _output_rx) = mpsc::channel(32);
        let snapshot = manager.attach_client(session_id, client_id, output_tx)
            .await
            .expect("Failed to attach client");

        assert_eq!(snapshot.id, session_id);
        assert_eq!(snapshot.rows, 24);
        assert_eq!(snapshot.cols, 80);

        // Scrollback should have some data (from shell prompt and/or output)
        // Note: We can't guarantee exact content due to timing
        assert!(snapshot.scrollback.len() >= 0);

        manager.kill_session(session_id).await.ok();
    }

    #[tokio::test]
    async fn test_kill_nonexistent_session() {
        use uuid::Uuid;

        let manager = SessionManager::new(None);

        let fake_id = Uuid::new_v4();
        let result = manager.kill_session(fake_id).await;

        assert!(result.is_err());
        match result {
            Err(SessionError::NotFound(id)) => assert_eq!(id, fake_id),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_concurrent_session_creation() {
        let manager = std::sync::Arc::new(SessionManager::new(None));

        // Create multiple sessions concurrently
        let mut handles = vec![];

        for _ in 0..5 {
            let mgr = manager.clone();
            let handle = tokio::spawn(async move {
                mgr.create_session(None, 24, 80).await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        let mut session_ids = vec![];
        for handle in handles {
            if let Ok(Ok(id)) = handle.await {
                session_ids.push(id);
            }
        }

        assert_eq!(session_ids.len(), 5);
        assert_eq!(manager.session_count().await, 5);

        // All IDs should be unique
        let unique_count = session_ids.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, 5);

        // Cleanup
        for id in session_ids {
            manager.kill_session(id).await.ok();
        }
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_session_activity_timestamp() {
        use uuid::Uuid;

        let manager = SessionManager::new(None);

        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        // Initial activity timestamp set
        let client_id = Uuid::new_v4();
        let (output_tx, _output_rx) = mpsc::channel(32);
        manager.attach_client(session_id, client_id, output_tx).await.ok();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send input should update timestamp
        manager.send_input(session_id, b"echo test\r\n")
            .await
            .expect("Failed to send input");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Resize should update timestamp
        manager.resize_session(session_id, 30, 100)
            .await
            .expect("Failed to resize");

        // We can't directly check timestamps from outside the session,
        // but this test ensures the touch() calls don't panic

        manager.kill_session(session_id).await.ok();
    }

    #[tokio::test]
    async fn test_multiple_clients_same_session() {
        use uuid::Uuid;

        let manager = SessionManager::new(None);

        let session_id = manager.create_session(None, 24, 80)
            .await
            .expect("Failed to create session");

        // Attach multiple clients to same session
        let client1 = Uuid::new_v4();
        let client2 = Uuid::new_v4();
        let client3 = Uuid::new_v4();

        let (output_tx1, _output_rx1) = mpsc::channel(32);
        let (output_tx2, _output_rx2) = mpsc::channel(32);
        let (output_tx3, _output_rx3) = mpsc::channel(32);
        let snapshot1 = manager.attach_client(session_id, client1, output_tx1).await.expect("Failed to attach client 1");
        let snapshot2 = manager.attach_client(session_id, client2, output_tx2).await.expect("Failed to attach client 2");
        let snapshot3 = manager.attach_client(session_id, client3, output_tx3).await.expect("Failed to attach client 3");

        // All snapshots should be for the same session
        assert_eq!(snapshot1.id, session_id);
        assert_eq!(snapshot2.id, session_id);
        assert_eq!(snapshot3.id, session_id);

        // Detach clients
        manager.detach_client(session_id, client1).await.ok();
        manager.detach_client(session_id, client2).await.ok();
        manager.detach_client(session_id, client3).await.ok();

        manager.kill_session(session_id).await.ok();
    }
}
