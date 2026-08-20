//! Database schema initialization
//! ADR-012 §2: Core Schema Design

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Initialize database schema if not exists
///
/// Checks for schema_migrations table, creates all tables if missing
pub fn init_schema(conn: &Connection) -> Result<()> {
    // Check if schema exists
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
        [],
        |row| row.get(0),
    )?;

    if table_exists == 0 {
        // Fresh database: create all tables
        tracing::info!("Initializing fresh database schema");

        let migration_sql = include_str!("../../../../migrations/001_initial_schema.sql");
        conn.execute_batch(migration_sql)
            .context("Failed to execute initial schema migration")?;

        tracing::info!("Database schema initialized successfully");
    } else {
        tracing::debug!("Database schema already exists");
    }

    Ok(())
}

/// Get current schema version
pub fn current_version(conn: &Connection) -> Result<i32> {
    let version: i32 = conn.query_row(
        "SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_init_schema() {
        let conn = Connection::open_in_memory().unwrap();

        init_schema(&conn).unwrap();

        // Verify all tables exist
        let tables = [
            "schema_migrations",
            "sessions",
            "scrollback",
            "configuration",
            "audit_logs",
        ];
        for table in &tables {
            let count: i64 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
                        table
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "Table {} should exist", table);
        }

        // Verify schema version is 1
        let version = current_version(&conn).unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_init_schema_idempotent() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize twice - should not error
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();

        // Should still be version 1 (not duplicated)
        let version = current_version(&conn).unwrap();
        assert_eq!(version, 1);
    }
}
