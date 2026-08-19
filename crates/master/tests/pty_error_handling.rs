// PTY error handling and edge case tests
// Tests error types and error propagation
// SRS §2.1.2: PTY Management

use monoterminal_master::pty::error::*;
use std::io;

#[test]
fn test_create_failed_error() {
    let err = PtyError::CreateFailed("Failed to allocate console".to_string());
    assert!(err.to_string().contains("Failed to create pseudo-console"));
    assert!(err.to_string().contains("Failed to allocate console"));
}

#[test]
fn test_spawn_failed_error() {
    let err = PtyError::SpawnFailed("Process creation error".to_string());
    assert!(err.to_string().contains("Failed to spawn child process"));
    assert!(err.to_string().contains("Process creation error"));
}

#[test]
fn test_process_exited_error() {
    let err = PtyError::ProcessExited;
    assert_eq!(err.to_string(), "Child process has exited");
}

#[test]
fn test_already_closed_error() {
    let err = PtyError::AlreadyClosed;
    assert_eq!(err.to_string(), "PTY session is already closed");
}

#[test]
fn test_invalid_config_error() {
    let err = PtyError::InvalidConfig("Invalid dimensions".to_string());
    assert!(err.to_string().contains("Invalid PTY configuration"));
    assert!(err.to_string().contains("Invalid dimensions"));
}

#[test]
fn test_timeout_error() {
    let err = PtyError::Timeout("Waiting for process start".to_string());
    assert!(err.to_string().contains("Timeout waiting for process"));
    assert!(err.to_string().contains("Waiting for process start"));
}

#[test]
fn test_disconnected_error() {
    let err = PtyError::Disconnected;
    assert_eq!(err.to_string(), "PTY disconnected");
}

#[test]
fn test_io_error_conversion() {
    let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe broken");
    let pty_err: PtyError = io_err.into();

    match pty_err {
        PtyError::Io(e) => {
            assert_eq!(e.kind(), io::ErrorKind::BrokenPipe);
        }
        _ => panic!("Expected Io error variant"),
    }
}

#[test]
fn test_error_is_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<PtyError>();
    assert_sync::<PtyError>();
}

#[test]
fn test_pty_result_ok() {
    let result: PtyResult<i32> = Ok(42);
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_pty_result_err() {
    let result: PtyResult<i32> = Err(PtyError::ProcessExited);
    assert!(result.is_err());
}

#[test]
fn test_error_chaining() {
    fn inner_fn() -> PtyResult<()> {
        Err(PtyError::ProcessExited)
    }

    fn outer_fn() -> PtyResult<()> {
        inner_fn()?;
        Ok(())
    }

    let result = outer_fn();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PtyError::ProcessExited));
}

#[test]
fn test_multiple_error_types() {
    let errors = vec![
        PtyError::CreateFailed("test1".into()),
        PtyError::SpawnFailed("test2".into()),
        PtyError::ProcessExited,
        PtyError::AlreadyClosed,
        PtyError::InvalidConfig("test3".into()),
        PtyError::Timeout("test4".into()),
        PtyError::Disconnected,
    ];

    // All should have non-empty string representations
    for err in errors {
        assert!(!err.to_string().is_empty());
    }
}

#[cfg(test)]
mod pty_config_tests {
    use monoterminal_master::pty::PtyConfig;
    use std::path::PathBuf;

    #[test]
    fn test_default_config() {
        let config = PtyConfig::default();

        assert_eq!(config.rows, 24);
        assert_eq!(config.cols, 80);
        #[cfg(windows)]
        assert_eq!(config.shell, "powershell.exe");
        #[cfg(not(windows))]
        assert_eq!(config.shell, "/bin/bash");
    }

    #[test]
    fn test_custom_config() {
        use std::collections::HashMap;

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let config = PtyConfig {
            rows: 50,
            cols: 120,
            shell: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\test"),
            environment: env.clone(),
        };

        assert_eq!(config.rows, 50);
        assert_eq!(config.cols, 120);
        assert_eq!(config.shell, "cmd.exe");
        assert_eq!(config.working_dir, PathBuf::from("C:\\test"));
        assert_eq!(
            config.environment.get("TEST_VAR"),
            Some(&"test_value".to_string())
        );
    }

    #[test]
    fn test_config_clone() {
        let config1 = PtyConfig {
            rows: 30,
            cols: 100,
            shell: "bash".to_string(),
            working_dir: PathBuf::from("/home"),
            environment: Default::default(),
        };

        let config2 = config1.clone();

        assert_eq!(config1.rows, config2.rows);
        assert_eq!(config1.cols, config2.cols);
        assert_eq!(config1.shell, config2.shell);
        assert_eq!(config1.working_dir, config2.working_dir);
    }

    #[test]
    fn test_config_with_environment() {
        use std::collections::HashMap;

        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());

        let config = PtyConfig {
            environment: env,
            ..Default::default()
        };

        assert_eq!(config.environment.len(), 2);
        assert!(config.environment.contains_key("PATH"));
        assert!(config.environment.contains_key("HOME"));
    }

    #[test]
    fn test_extreme_dimensions() {
        let config = PtyConfig {
            rows: 1,
            cols: 1,
            ..Default::default()
        };
        assert_eq!(config.rows, 1);
        assert_eq!(config.cols, 1);

        let config = PtyConfig {
            rows: 500,
            cols: 500,
            ..Default::default()
        };
        assert_eq!(config.rows, 500);
        assert_eq!(config.cols, 500);
    }
}
