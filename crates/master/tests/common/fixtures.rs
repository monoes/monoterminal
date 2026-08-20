// Test fixtures and context management
// As specified in test-strategy-phase1.md §6.3

use std::path::PathBuf;
use tempfile::TempDir;

/// Test context providing temporary directories and configuration
pub struct TestContext {
    pub temp_dir: TempDir,
    #[allow(dead_code)]
    pub config_path: PathBuf,
    #[allow(dead_code)]
    pub database_path: PathBuf,
}

impl TestContext {
    /// Create a new test context with temporary directory
    pub fn new() -> anyhow::Result<Self> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");
        let database_path = temp_dir.path().join("test.db");

        Ok(Self {
            temp_dir,
            config_path,
            database_path,
        })
    }

    /// Get the temporary directory path
    pub fn temp_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new().expect("Failed to create test context")
    }
}

/// Generate a sample JWT for testing
/// TODO: Replace with actual Ed25519 signing once auth module is complete
pub fn sample_jwt() -> String {
    "eyJ0eXAiOiJKV1QiLCJhbGciOiJFZERTQSJ9.sample.test".to_string()
}

/// Generate a test session ID (UUID v4)
pub fn test_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creates_temp_dir() {
        let ctx = TestContext::new().unwrap();
        assert!(ctx.temp_path().exists());
    }

    #[test]
    fn test_sample_jwt_is_nonempty() {
        let jwt = sample_jwt();
        assert!(!jwt.is_empty());
    }

    #[test]
    fn test_session_id_is_uuid() {
        let session_id = test_session_id();
        // UUID v4 format: 8-4-4-4-12
        assert_eq!(session_id.len(), 36);
        assert_eq!(session_id.chars().filter(|c| *c == '-').count(), 4);
    }
}
