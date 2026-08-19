//! Database migration system
//! ADR-012 §5: Migration Strategy

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing::{info, warn};

/// Migration definition
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

/// All available migrations (beyond initial schema)
/// Migrations are forward-only: v1 → v2 → v3
pub const MIGRATIONS: &[Migration] = &[
    // Migration 002 will be added in future phases
    // Example:
    // Migration {
    //     version: 2,
    //     description: "Add session tags support",
    //     sql: include_str!("../../../migrations/002_add_session_tags.sql"),
    // },
];

/// Apply all pending migrations
pub fn apply_pending_migrations(conn: &mut Connection) -> Result<()> {
    // Get current version
    let current_version = super::schema::current_version(conn).unwrap_or(0);

    info!("Current schema version: {}", current_version);

    // Find pending migrations
    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|m| m.version > current_version)
        .collect();

    if pending.is_empty() {
        info!("No pending migrations");
        return Ok(());
    }

    info!("Found {} pending migrations", pending.len());

    // Apply each migration in order
    for migration in pending {
        info!(
            "Applying migration {}: {}",
            migration.version, migration.description
        );

        apply_migration(conn, migration)
            .with_context(|| format!("Failed to apply migration {}", migration.version))?;

        info!("Migration {} applied successfully", migration.version);
    }

    Ok(())
}

/// Apply a single migration
fn apply_migration(conn: &mut Connection, migration: &Migration) -> Result<()> {
    let tx = conn.transaction()?;

    // Execute migration SQL
    tx.execute_batch(migration.sql)?;

    // Record migration in schema_migrations table
    tx.execute(
        "INSERT INTO schema_migrations (version, applied_at, description) VALUES (?1, datetime('now'), ?2)",
        rusqlite::params![migration.version, migration.description],
    )?;

    tx.commit()?;

    Ok(())
}

/// Verify migration history integrity
pub fn verify_migrations(conn: &Connection) -> Result<()> {
    let mut stmt =
        conn.prepare("SELECT version, description FROM schema_migrations ORDER BY version ASC")?;

    let applied: Vec<(i32, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;

    info!("Applied migrations:");
    for (version, description) in &applied {
        info!("  v{}: {}", version, description);
    }

    // Verify no gaps in version sequence
    for (i, (version, _)) in applied.iter().enumerate() {
        let expected = i as i32 + 1;
        if *version != expected {
            warn!(
                "Migration version gap detected: expected v{}, found v{}",
                expected, version
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_apply_pending_migrations_empty() {
        let mut conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();

        // No migrations to apply (MIGRATIONS is empty in Phase 2)
        apply_pending_migrations(&mut conn).unwrap();

        let version = super::super::schema::current_version(&conn).unwrap();
        assert_eq!(version, 1); // Still at initial schema version
    }

    #[test]
    fn test_verify_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();

        verify_migrations(&conn).unwrap();
    }
}
