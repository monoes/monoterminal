//! Database backup and restore
//! ADR-012 §3.3: Backup Strategy

use anyhow::{Context, Result};
use rusqlite::{backup::Backup, Connection};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

/// Backup a database to a destination path
///
/// Uses SQLite online backup API - doesn't lock the database
/// ADR-012: "SQLite online backup (doesn't lock database)"
pub fn backup_database(src_path: &Path, dest_path: &Path) -> Result<()> {
    // Create destination directory if needed
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create backup directory")?;
    }

    let src_conn = Connection::open(src_path)
        .context("Failed to open source database")?;

    let mut dest_conn = Connection::open(dest_path)
        .context("Failed to open destination database")?;

    // SQLite online backup (5 pages at a time, 250ms sleep between batches)
    let backup = Backup::new(&src_conn, &mut dest_conn)
        .context("Failed to initialize backup")?;

    backup.run_to_completion(5, Duration::from_millis(250), None)
        .context("Failed to complete backup")?;

    info!("Database backup completed: {:?}", dest_path);
    Ok(())
}

/// Restore a database from a backup
pub fn restore_database(backup_path: &Path, dest_path: &Path) -> Result<()> {
    // Verify backup exists
    if !backup_path.exists() {
        anyhow::bail!("Backup file not found: {:?}", backup_path);
    }

    // Create destination directory if needed
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create destination directory")?;
    }

    // Simple file copy for restore
    std::fs::copy(backup_path, dest_path)
        .context("Failed to copy backup file")?;

    info!("Database restored from: {:?}", backup_path);
    Ok(())
}

/// Schedule automatic daily backups
///
/// ADR-012: "Daily backups: Keep last 7 days"
pub async fn schedule_daily_backups(db_path: PathBuf, backup_dir: PathBuf) {
    let mut interval = interval(Duration::from_secs(86400)); // 24 hours

    loop {
        interval.tick().await;

        let timestamp = chrono::Utc::now().format("%Y%m%d");
        let backup_path = backup_dir.join(format!("monoterminal-{}.db", timestamp));

        match backup_database(&db_path, &backup_path) {
            Ok(_) => info!("Scheduled backup completed: {:?}", backup_path),
            Err(e) => error!("Scheduled backup failed: {}", e),
        }

        // Cleanup old backups (keep last 7)
        if let Err(e) = cleanup_old_backups(&backup_dir, 7) {
            error!("Failed to cleanup old backups: {}", e);
        }
    }
}

/// Delete old backup files, keeping only the most recent N
pub fn cleanup_old_backups(backup_dir: &Path, keep_count: usize) -> Result<()> {
    if !backup_dir.exists() {
        return Ok(());
    }

    // List all backup files
    let mut backups: Vec<PathBuf> = std::fs::read_dir(backup_dir)
        .context("Failed to read backup directory")?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().map_or(false, |ext| ext == "db")
                && path.file_name()
                    .and_then(|n| n.to_str())
                    .map_or(false, |n| n.starts_with("monoterminal-"))
        })
        .collect();

    // Sort by modification time (newest first)
    backups.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    backups.reverse();

    // Delete old backups beyond keep_count
    if backups.len() > keep_count {
        for backup_path in backups.iter().skip(keep_count) {
            match std::fs::remove_file(backup_path) {
                Ok(_) => info!("Deleted old backup: {:?}", backup_path),
                Err(e) => error!("Failed to delete old backup {:?}: {}", backup_path, e),
            }
        }
    }

    Ok(())
}

/// List available backups
pub fn list_backups(backup_dir: &Path) -> Result<Vec<BackupInfo>> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups: Vec<BackupInfo> = std::fs::read_dir(backup_dir)
        .context("Failed to read backup directory")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }

            let metadata = std::fs::metadata(&path).ok()?;
            let modified = metadata.modified().ok()?;
            let size = metadata.len();

            Some(BackupInfo {
                path,
                created_at: modified,
                size_bytes: size,
            })
        })
        .collect();

    // Sort by creation time (newest first)
    backups.sort_by_key(|b| b.created_at);
    backups.reverse();

    Ok(backups)
}

/// Backup file information
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub created_at: std::time::SystemTime,
    pub size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_and_restore() {
        let temp_dir = TempDir::new().unwrap();
        let src_db = temp_dir.path().join("source.db");
        let backup_db = temp_dir.path().join("backup.db");
        let restore_db = temp_dir.path().join("restore.db");

        // Create a source database
        let conn = Connection::open(&src_db).unwrap();
        crate::persistence::schema::init_schema(&conn).unwrap();
        drop(conn);

        // Backup
        backup_database(&src_db, &backup_db).unwrap();
        assert!(backup_db.exists());

        // Restore
        restore_database(&backup_db, &restore_db).unwrap();
        assert!(restore_db.exists());

        // Verify restored database is valid
        let conn = Connection::open(&restore_db).unwrap();
        let version = crate::persistence::schema::current_version(&conn).unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_cleanup_old_backups() {
        let temp_dir = TempDir::new().unwrap();

        // Create 10 dummy backup files
        for i in 0..10 {
            let path = temp_dir.path().join(format!("monoterminal-2026081{}.db", i));
            std::fs::write(&path, b"dummy").unwrap();
        }

        // Keep only last 5
        cleanup_old_backups(temp_dir.path(), 5).unwrap();

        // Count remaining files
        let remaining = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .count();

        assert_eq!(remaining, 5);
    }

    #[test]
    fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();

        // Create some backup files
        for i in 0..3 {
            let path = temp_dir.path().join(format!("monoterminal-2026081{}.db", i));
            std::fs::write(&path, b"dummy").unwrap();
        }

        let backups = list_backups(temp_dir.path()).unwrap();
        assert_eq!(backups.len(), 3);
    }
}
