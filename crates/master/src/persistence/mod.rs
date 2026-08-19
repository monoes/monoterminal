//! SQLite persistence layer for MONOTERMINAL
//!
//! ADR-012: Persistence Layer Design
//! Phase 2 implementation - Session state, scrollback, configuration, and audit logs
//!
//! # Architecture
//! - WAL mode for concurrent access (readers don't block writers)
//! - Connection pooling (r2d2) for multi-threaded access
//! - zstd compression for scrollback (60-80% reduction)
//! - Automatic daily backups (7-day retention)
//! - Disk space monitoring (80% warning, 95% emergency purge)

pub mod schema;
pub mod migrations;
pub mod session;
pub mod scrollback;
pub mod backup;
pub mod disk_monitor;
pub mod audit;

use std::path::{Path, PathBuf};
use std::time::Duration;
use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

/// Database connection pool
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
    db_path: PathBuf,
}

impl Database {
    /// Initialize database with WAL mode and connection pooling
    ///
    /// # Arguments
    /// * `db_path` - Path to SQLite database file
    ///
    /// # Returns
    /// Database instance with connection pool
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        // Create parent directory if missing
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }

        // Configure connection manager with WAL mode
        let manager = SqliteConnectionManager::file(&db_path)
            .with_init(|conn| {
                // Enable WAL mode and performance tuning (ADR-012 §1.1)
                conn.execute_batch(
                    "PRAGMA journal_mode = WAL;
                     PRAGMA synchronous = NORMAL;
                     PRAGMA foreign_keys = ON;
                     PRAGMA cache_size = -64000;      -- 64MB cache
                     PRAGMA mmap_size = 268435456;    -- 256MB mmap
                     PRAGMA temp_store = MEMORY;"
                )?;
                Ok(())
            });

        // Create connection pool (max 20 concurrent connections)
        let pool = Pool::builder()
            .max_size(20)
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .context("Failed to create connection pool")?;

        // Initialize schema if needed
        {
            let mut conn = pool.get().context("Failed to get connection from pool")?;
            schema::init_schema(&conn)?;
            migrations::apply_pending_migrations(&mut conn)?;
        }

        Ok(Database { pool, db_path })
    }

    /// Get a connection from the pool
    pub fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get()
            .context("Failed to get connection from pool")
    }

    /// Get the database file path
    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Run database integrity check (PRAGMA integrity_check)
    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("PRAGMA integrity_check")?;
        let results = stmt.query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(results)
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DatabaseStats> {
        let conn = self.get_conn()?;

        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        let db_size = page_count * page_size;

        let session_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE status != 'TERMINATED'",
            [],
            |row| row.get(0)
        )?;

        let scrollback_lines: i64 = conn.query_row(
            "SELECT COUNT(*) FROM scrollback",
            [],
            |row| row.get(0)
        )?;

        let audit_log_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM audit_logs",
            [],
            |row| row.get(0)
        )?;

        Ok(DatabaseStats {
            db_size_bytes: db_size as u64,
            session_count: session_count as u64,
            scrollback_lines: scrollback_lines as u64,
            audit_log_count: audit_log_count as u64,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub db_size_bytes: u64,
    pub session_count: u64,
    pub scrollback_lines: u64,
    pub audit_log_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_init() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::new(&db_path).unwrap();

        // Verify WAL mode is enabled
        let mut conn = db.get_conn().unwrap();
        let journal_mode: String = conn.query_row(
            "PRAGMA journal_mode",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(journal_mode, "wal");

        // Verify schema exists
        let table_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get(0)
        ).unwrap();
        assert_eq!(table_exists, 1);
    }

    #[test]
    fn test_integrity_check() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).unwrap();

        let results = db.integrity_check().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "ok");
    }
}
