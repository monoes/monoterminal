//! Scrollback persistence with zstd compression
//! ADR-012 §2.2: Scrollback Storage

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Scrollback line record
#[derive(Debug, Clone)]
pub struct ScrollbackLine {
    pub session_id: Uuid,
    pub line_number: u64,
    pub data: Vec<u8>,
    pub timestamp_ms: u64,
    pub sequence_number: u64,
}

/// Compression threshold (512 bytes per ADR-012)
const COMPRESSION_THRESHOLD: usize = 512;

/// zstd compression level (1 = fastest, 22 = highest compression)
const COMPRESSION_LEVEL: i32 = 1;

/// Store a single scrollback line (with optional compression)
pub fn store_line(conn: &Connection, line: &ScrollbackLine) -> Result<()> {
    let (blob, compressed) = if line.data.len() > COMPRESSION_THRESHOLD {
        // Compress if >512 bytes
        let compressed = zstd::encode_all(&line.data[..], COMPRESSION_LEVEL)
            .context("Failed to compress scrollback line")?;
        (compressed, true)
    } else {
        (line.data.clone(), false)
    };

    conn.execute(
        "INSERT INTO scrollback (session_id, line_number, data, data_compressed, timestamp_ms, sequence_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            line.session_id.to_string(),
            line.line_number,
            blob,
            compressed,
            line.timestamp_ms,
            line.sequence_number,
        ],
    )?;

    Ok(())
}

/// Store multiple scrollback lines in a single transaction (batched write)
/// ADR-012 §4.1: Write Batching
pub fn store_lines_batch(conn: &mut Connection, lines: &[ScrollbackLine]) -> Result<()> {
    if lines.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO scrollback (session_id, line_number, data, data_compressed, timestamp_ms, sequence_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?;

        for line in lines {
            let (blob, compressed) = if line.data.len() > COMPRESSION_THRESHOLD {
                let compressed = zstd::encode_all(&line.data[..], COMPRESSION_LEVEL)
                    .context("Failed to compress scrollback line")?;
                (compressed, true)
            } else {
                (line.data.clone(), false)
            };

            stmt.execute(params![
                line.session_id.to_string(),
                line.line_number,
                blob,
                compressed,
                line.timestamp_ms,
                line.sequence_number,
            ])?;
        }
    }

    tx.commit()?;

    tracing::debug!("Stored {} scrollback lines (batched)", lines.len());
    Ok(())
}

/// Fetch a range of scrollback lines
/// ADR-012 §2.2: Scrollback fetch (pagination)
pub fn fetch_range(
    conn: &Connection,
    session_id: &Uuid,
    start_line: u64,
    limit: usize,
) -> Result<Vec<ScrollbackLine>> {
    let mut stmt = conn.prepare(
        "SELECT line_number, data, data_compressed, timestamp_ms, sequence_number
         FROM scrollback
         WHERE session_id = ?1 AND line_number >= ?2
         ORDER BY line_number ASC
         LIMIT ?3",
    )?;

    let lines = stmt
        .query_map(params![session_id.to_string(), start_line, limit], |row| {
            let line_number: u64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            let data_compressed: bool = row.get(2)?;
            let timestamp_ms: u64 = row.get(3)?;
            let sequence_number: u64 = row.get(4)?;

            // Decompress if needed
            let data = if data_compressed {
                zstd::decode_all(&blob[..])
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
            } else {
                blob
            };

            Ok(ScrollbackLine {
                session_id: *session_id,
                line_number,
                data,
                timestamp_ms,
                sequence_number,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(lines)
}

/// Get total line count for a session
pub fn count_lines(conn: &Connection, session_id: &Uuid) -> Result<u64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM scrollback WHERE session_id = ?1",
        params![session_id.to_string()],
        |row| row.get(0),
    )?;
    Ok(count as u64)
}

/// Get compression statistics for a session
pub fn compression_stats(conn: &Connection, session_id: &Uuid) -> Result<CompressionStats> {
    let mut stmt =
        conn.prepare("SELECT data, data_compressed FROM scrollback WHERE session_id = ?1")?;

    let mut total_raw_size = 0u64;
    let mut total_stored_size = 0u64;
    let mut compressed_count = 0u64;
    let mut total_count = 0u64;

    let rows = stmt.query_map(params![session_id.to_string()], |row| {
        let blob: Vec<u8> = row.get(0)?;
        let is_compressed: bool = row.get(1)?;
        Ok((blob, is_compressed))
    })?;

    for result in rows {
        let (blob, is_compressed) = result?;
        total_count += 1;
        total_stored_size += blob.len() as u64;

        if is_compressed {
            compressed_count += 1;
            // Decompress to get original size
            let decompressed =
                zstd::decode_all(&blob[..]).context("Failed to decompress for stats")?;
            total_raw_size += decompressed.len() as u64;
        } else {
            total_raw_size += blob.len() as u64;
        }
    }

    let compression_ratio = if total_raw_size > 0 {
        total_stored_size as f64 / total_raw_size as f64
    } else {
        1.0
    };

    Ok(CompressionStats {
        total_lines: total_count,
        compressed_lines: compressed_count,
        total_raw_bytes: total_raw_size,
        total_stored_bytes: total_stored_size,
        compression_ratio,
    })
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub total_lines: u64,
    pub compressed_lines: u64,
    pub total_raw_bytes: u64,
    pub total_stored_bytes: u64,
    pub compression_ratio: f64, // stored/raw (< 1.0 means good compression)
}

/// Delete old scrollback lines (retention policy)
/// Keeps only the last `keep_lines` lines per session
pub fn prune_old_lines(conn: &Connection, session_id: &Uuid, keep_lines: u64) -> Result<u64> {
    let deleted = conn.execute(
        "DELETE FROM scrollback
         WHERE session_id = ?1
         AND line_number < (
             SELECT MAX(line_number) - ?2
             FROM scrollback
             WHERE session_id = ?1
         )",
        params![session_id.to_string(), keep_lines],
    )?;

    tracing::info!(
        "Pruned {} old scrollback lines for session {}",
        deleted,
        session_id
    );
    Ok(deleted as u64)
}

/// Current timestamp in milliseconds
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn test_store_and_fetch_line() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let line = ScrollbackLine {
            session_id,
            line_number: 1,
            data: b"Hello, world!".to_vec(),
            timestamp_ms: now_millis(),
            sequence_number: 1,
        };

        store_line(&conn, &line).unwrap();

        let lines = fetch_range(&conn, &session_id, 0, 10).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].data, b"Hello, world!");
    }

    #[test]
    fn test_compression() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        // Large line (>512 bytes) should be compressed
        let large_data = b"INFO: Build succeeded\n".repeat(100);
        let line = ScrollbackLine {
            session_id,
            line_number: 1,
            data: large_data.clone(),
            timestamp_ms: now_millis(),
            sequence_number: 1,
        };

        store_line(&conn, &line).unwrap();

        // Verify it was compressed
        let compressed: bool = conn
            .query_row(
                "SELECT data_compressed FROM scrollback WHERE session_id = ?1",
                params![session_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(compressed);

        // Verify we can decompress and get original data
        let lines = fetch_range(&conn, &session_id, 0, 10).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].data, large_data);
    }

    #[test]
    fn test_batch_write() {
        let mut conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let lines: Vec<ScrollbackLine> = (0..100)
            .map(|i| ScrollbackLine {
                session_id,
                line_number: i,
                data: format!("Line {}", i).into_bytes(),
                timestamp_ms: now_millis(),
                sequence_number: i,
            })
            .collect();

        store_lines_batch(&mut conn, &lines).unwrap();

        let count = count_lines(&conn, &session_id).unwrap();
        assert_eq!(count, 100);
    }

    #[test]
    fn test_compression_stats() {
        let mut conn = setup_test_db();
        let session_id = Uuid::new_v4();

        // Add mix of compressed and uncompressed lines
        let lines = vec![
            ScrollbackLine {
                session_id,
                line_number: 1,
                data: b"short".to_vec(), // Won't compress
                timestamp_ms: now_millis(),
                sequence_number: 1,
            },
            ScrollbackLine {
                session_id,
                line_number: 2,
                data: b"INFO: Build succeeded\n".repeat(100), // Will compress
                timestamp_ms: now_millis(),
                sequence_number: 2,
            },
        ];

        store_lines_batch(&mut conn, &lines).unwrap();

        let stats = compression_stats(&conn, &session_id).unwrap();
        assert_eq!(stats.total_lines, 2);
        assert_eq!(stats.compressed_lines, 1);
        assert!(stats.compression_ratio < 0.5); // Good compression ratio
    }

    #[test]
    fn test_prune_old_lines() {
        let mut conn = setup_test_db();
        let session_id = Uuid::new_v4();

        // Add 100 lines
        let lines: Vec<ScrollbackLine> = (0..100)
            .map(|i| ScrollbackLine {
                session_id,
                line_number: i,
                data: format!("Line {}", i).into_bytes(),
                timestamp_ms: now_millis(),
                sequence_number: i,
            })
            .collect();

        store_lines_batch(&mut conn, &lines).unwrap();

        // Keep only last 10
        prune_old_lines(&conn, &session_id, 10).unwrap();

        let count = count_lines(&conn, &session_id).unwrap();
        assert_eq!(count, 10);
    }
}
