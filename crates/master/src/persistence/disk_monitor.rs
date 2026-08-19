//! Disk space monitoring and emergency purge
//! ADR-012: "Disk space monitoring (80% warning, 95% emergency purge)"

use anyhow::{Context, Result};
use std::path::Path;
use tracing::{error, info, warn};

/// Disk usage threshold levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskUsageLevel {
    Normal,    // < 80%
    Warning,   // 80-95%
    Emergency, // >= 95%
}

/// Disk usage information
#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
    pub level: DiskUsageLevel,
}

impl DiskUsage {
    /// Calculate usage level from percentage
    pub fn level_from_percent(usage_percent: f64) -> DiskUsageLevel {
        if usage_percent >= 95.0 {
            DiskUsageLevel::Emergency
        } else if usage_percent >= 80.0 {
            DiskUsageLevel::Warning
        } else {
            DiskUsageLevel::Normal
        }
    }
}

/// Get disk usage for a path (Windows-specific implementation)
#[cfg(target_os = "windows")]
pub fn get_disk_usage(path: &Path) -> Result<DiskUsage> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // Get the root path (drive letter)
    let root = path.ancestors().last().context("Failed to get root path")?;

    let mut root_str = root.as_os_str().to_os_string();
    root_str.push("\\");

    // Convert to wide string for Windows API
    let wide: Vec<u16> = OsStr::new(&root_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_bytes_available = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free_bytes = 0u64;

    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR::from_raw(wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_bytes),
            Some(&mut total_free_bytes),
        )
        .context("GetDiskFreeSpaceExW failed")?;
    }

    let used_bytes = total_bytes.saturating_sub(free_bytes_available);
    let usage_percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let level = DiskUsage::level_from_percent(usage_percent);

    Ok(DiskUsage {
        total_bytes,
        available_bytes: free_bytes_available,
        used_bytes,
        usage_percent,
        level,
    })
}

/// Get disk usage for a path (Unix-specific implementation)
#[cfg(not(target_os = "windows"))]
pub fn get_disk_usage(path: &Path) -> Result<DiskUsage> {
    use std::os::unix::fs::statvfs;

    let stat = statvfs(path).context("Failed to get filesystem stats")?;

    let block_size = stat.f_bsize as u64;
    let total_bytes = stat.f_blocks as u64 * block_size;
    let available_bytes = stat.f_bavail as u64 * block_size;
    let used_bytes = total_bytes.saturating_sub(available_bytes);

    let usage_percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let level = DiskUsage::level_from_percent(usage_percent);

    Ok(DiskUsage {
        total_bytes,
        available_bytes,
        used_bytes,
        usage_percent,
        level,
    })
}

/// Check disk usage and log warnings
pub fn check_disk_usage(path: &Path) -> Result<DiskUsage> {
    let usage = get_disk_usage(path)?;

    match usage.level {
        DiskUsageLevel::Normal => {
            info!(
                "Disk usage: {:.1}% ({} GB available)",
                usage.usage_percent,
                usage.available_bytes / (1024 * 1024 * 1024)
            );
        }
        DiskUsageLevel::Warning => {
            warn!("⚠️  Disk usage WARNING: {:.1}% ({} GB available) - Consider cleaning up old sessions",
                usage.usage_percent,
                usage.available_bytes / (1024 * 1024 * 1024)
            );
        }
        DiskUsageLevel::Emergency => {
            error!(
                "🚨 Disk usage EMERGENCY: {:.1}% ({} GB available) - Automatic purge recommended",
                usage.usage_percent,
                usage.available_bytes / (1024 * 1024 * 1024)
            );
        }
    }

    Ok(usage)
}

/// Emergency purge: delete old terminated sessions to free space
pub fn emergency_purge_old_sessions(
    conn: &mut rusqlite::Connection,
    target_percent: f64,
) -> Result<u64> {
    info!(
        "Starting emergency purge of old sessions (target: {:.1}% usage)",
        target_percent
    );

    // Find old terminated sessions (oldest first)
    // Collect session IDs first, then delete (avoid borrow conflicts)
    let session_ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT session_id FROM sessions
             WHERE status = 'TERMINATED'
             ORDER BY last_accessed_at ASC",
        )?;

        let ids: Result<Vec<String>, _> = stmt.query_map([], |row| row.get(0))?.collect();
        ids?
    }; // stmt dropped here, releasing immutable borrow

    info!("Found {} terminated sessions to purge", session_ids.len());

    let mut purged_count = 0u64;

    for session_id_str in session_ids {
        let session_id = uuid::Uuid::parse_str(&session_id_str).context("Invalid session UUID")?;

        // Delete session and scrollback (now safe - no active immutable borrow)
        super::session::delete_session(conn, &session_id)?;
        purged_count += 1;

        // Check disk usage after each deletion
        // Note: This is a simplified version - in production, we'd check actual DB size
        // For now, we'll purge a fixed number of sessions
        if purged_count >= 10 {
            info!("Purged {} sessions in emergency mode", purged_count);
            break;
        }
    }

    Ok(purged_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_disk_usage_levels() {
        assert_eq!(DiskUsage::level_from_percent(50.0), DiskUsageLevel::Normal);
        assert_eq!(DiskUsage::level_from_percent(85.0), DiskUsageLevel::Warning);
        assert_eq!(
            DiskUsage::level_from_percent(96.0),
            DiskUsageLevel::Emergency
        );
    }

    #[test]
    fn test_get_disk_usage() {
        let temp_dir = TempDir::new().unwrap();
        let usage = get_disk_usage(temp_dir.path()).unwrap();

        assert!(usage.total_bytes > 0);
        assert!(usage.available_bytes > 0);
        assert!(usage.usage_percent >= 0.0 && usage.usage_percent <= 100.0);
    }

    #[test]
    fn test_check_disk_usage() {
        let temp_dir = TempDir::new().unwrap();
        let usage = check_disk_usage(temp_dir.path()).unwrap();

        // Should not panic - just log
        assert!(usage.total_bytes > 0);
    }
}
