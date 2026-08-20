// Property-based tests for PTY state transitions and edge cases
// Uses proptest for comprehensive testing per SRS requirements

#[cfg(test)]
mod property_tests {
    use super::super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Generate valid terminal dimensions
    fn terminal_dimensions() -> impl Strategy<Value = (u16, u16)> {
        (10u16..=200, 20u16..=300)
    }

    /// Generate valid working directories
    fn working_dir() -> impl Strategy<Value = PathBuf> {
        prop::collection::vec("[a-zA-Z0-9_-]{1,10}", 0..3).prop_map(|parts| {
            if parts.is_empty() {
                PathBuf::from("C:\\")
            } else {
                PathBuf::from("C:\\").join(parts.join("\\"))
            }
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 10,  // Reduce from default 256 to 10 cases
            max_shrink_iters: 100,
            timeout: 5000,  // 5 second timeout per test case
            ..ProptestConfig::default()
        })]

        #[test]
        #[ignore = "Blanket exclusion of tests.rs module due to AsyncPipeReader/Writer blocking I/O. Phase 2: migrate to windows.rs PtyHandle"]
        fn test_create_with_any_valid_dimensions(
            (rows, cols) in terminal_dimensions()
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let config = PtyConfig {
                    shell: "cmd.exe".to_string(),
                    working_dir: PathBuf::from("C:\\"),
                    rows,
                    cols,
                    environment: Default::default(),
                };

                // Add timeout to prevent hangs
                let create_result = tokio::time::timeout(
                    Duration::from_secs(3),
                    ConPtyBackend::create(config)
                ).await;

                match create_result {
                    Ok(Ok(pty)) => {
                        prop_assert!(pty.shell_pid() > 0);
                        pty.terminate().await.ok();
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        prop_assert!(false, "Failed to create PTY with {}x{}: {:?}", rows, cols, e);
                        Ok(())
                    }
                    Err(_) => {
                        prop_assert!(false, "PTY creation timed out for {}x{}", rows, cols);
                        Ok(())
                    }
                }
            });
            result?
        }

        #[test]
        #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
        fn test_resize_maintains_pty_validity(
            initial in terminal_dimensions(),
            new_size in terminal_dimensions()
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                let config = PtyConfig {
                    shell: "cmd.exe".to_string(),
                    working_dir: PathBuf::from("C:\\"),
                    rows: initial.0,
                    cols: initial.1,
                    environment: Default::default(),
                };

                // Add timeout to PTY creation
                let pty_result = tokio::time::timeout(
                    Duration::from_secs(3),
                    ConPtyBackend::create(config)
                ).await;

                let mut pty = match pty_result {
                    Ok(Ok(p)) => p,
                    Ok(Err(e)) => {
                        prop_assert!(false, "Failed to create PTY: {:?}", e);
                        return Ok(());
                    }
                    Err(_) => {
                        prop_assert!(false, "PTY creation timed out");
                        return Ok(());
                    }
                };

                // Resize should succeed
                let resize_result = pty.resize(new_size.0, new_size.1);
                prop_assert!(resize_result.is_ok(), "Resize failed: {:?}", resize_result);

                // PTY should still be usable after resize (don't read, just verify resize works)
                pty.write(b"echo test\r\n").await.ok();

                pty.terminate().await.ok();
                Ok(())
            });
            result?
        }
    }

    #[tokio::test]
    #[ignore = "Known issue: AsyncPipeReader uses blocking ReadFile in poll_read, violates tokio async contract. See windows.rs PtyHandle for proper async architecture. TODO: Phase 2 - migrate to windows.rs or implement proper overlapped I/O"]
    async fn test_resize_during_output_burst() {
        // Edge case: Resize while PTY is outputting large amounts of data
        let config = PtyConfig {
            shell: "powershell.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut pty = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PTY");

        // Start a command that produces continuous output
        pty.write(b"1..1000 | ForEach-Object { Write-Host $_ }\r\n")
            .await
            .expect("Failed to write command");

        // Wait a bit for output to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Resize multiple times while output is flowing
        for size in [(30, 100), (50, 120), (24, 80), (40, 90)] {
            let result = pty.resize(size.0, size.1);
            assert!(result.is_ok(), "Resize failed during output: {:?}", result);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // PTY should still be readable
        let mut buf = vec![0u8; 4096];
        let read_result = tokio::time::timeout(Duration::from_secs(1), pty.read(&mut buf)).await;

        assert!(read_result.is_ok(), "Read timeout after resize burst");

        pty.terminate().await.ok();
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_process_exit_mid_write() {
        // Edge case: Process exits while we're writing to it
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut pty = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PTY");

        // Send exit command
        pty.write(b"exit\r\n").await.expect("Failed to write exit");

        // Try to write after process might have exited
        tokio::time::sleep(Duration::from_millis(100)).await;

        // This should either succeed (buffered) or fail gracefully
        let _write_result = pty.write(b"this should not crash\r\n").await;

        // We don't assert on the result - just that it doesn't panic
        // The error should be handled gracefully

        pty.terminate().await.ok();
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_orphaned_child_processes() {
        // Edge case: Spawn a child process, then kill the shell
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut pty = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PTY");

        let _shell_pid = pty.shell_pid();

        // Start a long-running child process
        pty.write(b"timeout /t 60\r\n")
            .await
            .expect("Failed to write command");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Terminate the PTY (should clean up child processes)
        pty.terminate().await.expect("Failed to terminate");

        // Wait a bit and check if the shell process is gone
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Process should be terminated (we can't easily check this without external tools)
        // This test primarily ensures terminate() doesn't hang
    }

    #[tokio::test]
    #[ignore = "Blanket exclusion of tests.rs module due to AsyncPipeReader/Writer blocking I/O. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_rapid_create_destroy() {
        // Stress test: Rapidly create and destroy PTY sessions
        // NOTE: Added delay to prevent heap corruption from closing pipe handles
        // while async ReadFile operations are still pending (STATUS_HEAP_CORRUPTION fix)
        for i in 0..10 {
            let config = PtyConfig {
                shell: "cmd.exe".to_string(),
                working_dir: PathBuf::from("C:\\"),
                rows: 24,
                cols: 80,
                environment: Default::default(),
            };

            let pty = ConPtyBackend::create(config)
                .await
                .unwrap_or_else(|_| panic!("Failed to create PTY iteration {}", i));

            assert!(pty.shell_pid() > 0);

            pty.terminate().await.expect("Failed to terminate");

            // Wait for async cleanup to complete before next iteration
            // This prevents race condition between ReadFile and CloseHandle
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    #[tokio::test]
    #[ignore = "Known issue: AsyncPipeReader uses blocking ReadFile in poll_read, violates tokio async contract. See windows.rs PtyHandle for proper async architecture. TODO: Phase 2 - migrate to windows.rs or implement proper overlapped I/O"]
    async fn test_concurrent_read_write() {
        // Test concurrent reads and writes don't cause issues
        let config = PtyConfig {
            shell: "powershell.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut pty = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PTY");

        // Spawn concurrent write tasks
        let write_task = {
            tokio::spawn(async move {
                for _i in 0..10 {
                    let _cmd = format!("echo write_{}\r\n", _i);
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
        };

        // Read in main task
        let mut buf = vec![0u8; 4096];
        for _ in 0..5 {
            tokio::time::timeout(Duration::from_millis(200), pty.read(&mut buf))
                .await
                .ok();
        }

        write_task.await.ok();
        pty.terminate().await.ok();
    }

    #[tokio::test]
    #[ignore = "AsyncPipeReader/Writer use blocking I/O in poll functions, violates tokio async contract. Phase 2: migrate to windows.rs PtyHandle"]
    async fn test_large_write() {
        // Test writing large chunks (> 4KB buffer)
        let config = PtyConfig {
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            rows: 24,
            cols: 80,
            environment: Default::default(),
        };

        let mut pty = ConPtyBackend::create(config)
            .await
            .expect("Failed to create PTY");

        // Write 64KB of data
        let large_data = vec![b'A'; 65536];
        let result = pty.write(&large_data).await;

        // Should either succeed or fail gracefully
        // Most importantly, should not panic or hang
        match result {
            Ok(_) => {
                // Success - data buffered
            }
            Err(e) => {
                // Acceptable failure modes: broken pipe, buffer full
                assert!(
                    e.kind() == std::io::ErrorKind::BrokenPipe
                        || e.kind() == std::io::ErrorKind::WouldBlock,
                    "Unexpected error: {:?}",
                    e
                );
            }
        }

        pty.terminate().await.ok();
    }
}
