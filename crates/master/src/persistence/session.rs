//! Session persistence operations
//! ADR-012 §2.1: Sessions Table

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Persisted session record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub created_at: String,
    pub last_accessed_at: String,
    pub status: SessionStatus,
    pub shell_path: String,
    pub working_dir: PathBuf,
    pub env_vars: Option<HashMap<String, String>>,
    pub rows: u16,
    pub cols: u16,
    pub owner_user_id: Option<String>,
    pub acl: Option<HashMap<String, String>>, // user_email -> permission
    pub metadata: Option<serde_json::Value>,
}

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Detached,
    Terminated,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Running => "RUNNING",
            SessionStatus::Detached => "DETACHED",
            SessionStatus::Terminated => "TERMINATED",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "RUNNING" => Ok(SessionStatus::Running),
            "DETACHED" => Ok(SessionStatus::Detached),
            "TERMINATED" => Ok(SessionStatus::Terminated),
            _ => anyhow::bail!("Invalid session status: {}", s),
        }
    }
}

/// Create a new session record in the database
pub fn create_session(conn: &Connection, record: &SessionRecord) -> Result<()> {
    let env_vars_json = record.env_vars.as_ref()
        .map(|e| serde_json::to_string(e))
        .transpose()?;

    let acl_json = record.acl.as_ref()
        .map(|a| serde_json::to_string(a))
        .transpose()?;

    let metadata_json = record.metadata.as_ref()
        .map(|m| serde_json::to_string(m))
        .transpose()?;

    conn.execute(
        "INSERT INTO sessions
         (session_id, created_at, last_accessed_at, status, shell_path, working_dir,
          env_vars, rows, cols, owner_user_id, acl, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.session_id.to_string(),
            record.created_at,
            record.last_accessed_at,
            record.status.as_str(),
            record.shell_path,
            record.working_dir.to_string_lossy().to_string(),
            env_vars_json,
            record.rows,
            record.cols,
            record.owner_user_id,
            acl_json,
            metadata_json,
        ],
    ).context("Failed to insert session record")?;

    tracing::info!("Created session record: {}", record.session_id);
    Ok(())
}

/// Load a session record from the database
pub fn load_session(conn: &Connection, session_id: &Uuid) -> Result<SessionRecord> {
    let mut stmt = conn.prepare(
        "SELECT session_id, created_at, last_accessed_at, status, shell_path, working_dir,
                env_vars, rows, cols, owner_user_id, acl, metadata
         FROM sessions WHERE session_id = ?1"
    )?;

    let record = stmt.query_row(params![session_id.to_string()], |row| {
        let env_vars: Option<String> = row.get(6)?;
        let acl: Option<String> = row.get(10)?;
        let metadata: Option<String> = row.get(11)?;

        Ok(SessionRecord {
            session_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            created_at: row.get(1)?,
            last_accessed_at: row.get(2)?,
            status: SessionStatus::from_str(&row.get::<_, String>(3)?).unwrap(),
            shell_path: row.get(4)?,
            working_dir: PathBuf::from(row.get::<_, String>(5)?),
            env_vars: env_vars.and_then(|s| serde_json::from_str(&s).ok()),
            rows: row.get(7)?,
            cols: row.get(8)?,
            owner_user_id: row.get(9)?,
            acl: acl.and_then(|s| serde_json::from_str(&s).ok()),
            metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),
        })
    }).context("Failed to load session record")?;

    Ok(record)
}

/// Update session status
pub fn update_session_status(
    conn: &Connection,
    session_id: &Uuid,
    status: SessionStatus,
) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET status = ?1, last_accessed_at = datetime('now') WHERE session_id = ?2",
        params![status.as_str(), session_id.to_string()],
    )?;
    Ok(())
}

/// Update session last_accessed_at timestamp
pub fn touch_session(conn: &Connection, session_id: &Uuid) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET last_accessed_at = datetime('now') WHERE session_id = ?1",
        params![session_id.to_string()],
    )?;
    Ok(())
}

/// Delete a session record (and all associated scrollback)
pub fn delete_session(conn: &mut Connection, session_id: &Uuid) -> Result<()> {
    let tx = conn.transaction()?;

    // Delete scrollback first
    tx.execute(
        "DELETE FROM scrollback WHERE session_id = ?1",
        params![session_id.to_string()],
    )?;

    // Delete session
    tx.execute(
        "DELETE FROM sessions WHERE session_id = ?1",
        params![session_id.to_string()],
    )?;

    tx.commit()?;

    tracing::info!("Deleted session: {}", session_id);
    Ok(())
}

/// List all active sessions (not TERMINATED)
pub fn list_active_sessions(conn: &Connection) -> Result<Vec<SessionRecord>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, created_at, last_accessed_at, status, shell_path, working_dir,
                env_vars, rows, cols, owner_user_id, acl, metadata
         FROM sessions WHERE status != 'TERMINATED'
         ORDER BY last_accessed_at DESC"
    )?;

    let sessions = stmt.query_map([], |row| {
        let env_vars: Option<String> = row.get(6)?;
        let acl: Option<String> = row.get(10)?;
        let metadata: Option<String> = row.get(11)?;

        Ok(SessionRecord {
            session_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            created_at: row.get(1)?,
            last_accessed_at: row.get(2)?,
            status: SessionStatus::from_str(&row.get::<_, String>(3)?).unwrap(),
            shell_path: row.get(4)?,
            working_dir: PathBuf::from(row.get::<_, String>(5)?),
            env_vars: env_vars.and_then(|s| serde_json::from_str(&s).ok()),
            rows: row.get(7)?,
            cols: row.get(8)?,
            owner_user_id: row.get(9)?,
            acl: acl.and_then(|s| serde_json::from_str(&s).ok()),
            metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Get session count by status
pub fn count_sessions_by_status(conn: &Connection) -> Result<HashMap<SessionStatus, u64>> {
    let mut counts = HashMap::new();

    for status in &[SessionStatus::Running, SessionStatus::Detached, SessionStatus::Terminated] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE status = ?1",
            params![status.as_str()],
            |row| row.get(0),
        )?;
        counts.insert(*status, count as u64);
    }

    Ok(counts)
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
    fn test_create_and_load_session() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let record = SessionRecord {
            session_id,
            created_at: "2026-08-19T10:00:00Z".to_string(),
            last_accessed_at: "2026-08-19T10:00:00Z".to_string(),
            status: SessionStatus::Running,
            shell_path: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\Users\\Alice"),
            env_vars: Some(HashMap::from([("PATH".to_string(), "C:\\bin".to_string())])),
            rows: 24,
            cols: 80,
            owner_user_id: Some("alice@example.com".to_string()),
            acl: Some(HashMap::from([("bob@example.com".to_string(), "viewer".to_string())])),
            metadata: Some(serde_json::json!({"name": "test-session"})),
        };

        create_session(&conn, &record).unwrap();

        let loaded = load_session(&conn, &session_id).unwrap();
        assert_eq!(loaded.session_id, session_id);
        assert_eq!(loaded.shell_path, "cmd.exe");
        assert_eq!(loaded.status, SessionStatus::Running);
    }

    #[test]
    fn test_update_session_status() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let record = SessionRecord {
            session_id,
            created_at: "2026-08-19T10:00:00Z".to_string(),
            last_accessed_at: "2026-08-19T10:00:00Z".to_string(),
            status: SessionStatus::Running,
            shell_path: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            env_vars: None,
            rows: 24,
            cols: 80,
            owner_user_id: None,
            acl: None,
            metadata: None,
        };

        create_session(&conn, &record).unwrap();
        update_session_status(&conn, &session_id, SessionStatus::Terminated).unwrap();

        let loaded = load_session(&conn, &session_id).unwrap();
        assert_eq!(loaded.status, SessionStatus::Terminated);
    }

    #[test]
    fn test_list_active_sessions() {
        let conn = setup_test_db();

        // Create 2 running sessions
        for i in 0..2 {
            let record = SessionRecord {
                session_id: Uuid::new_v4(),
                created_at: format!("2026-08-19T10:{:02}:00Z", i),
                last_accessed_at: format!("2026-08-19T10:{:02}:00Z", i),
                status: SessionStatus::Running,
                shell_path: "cmd.exe".to_string(),
                working_dir: PathBuf::from("C:\\"),
                env_vars: None,
                rows: 24,
                cols: 80,
                owner_user_id: None,
                acl: None,
                metadata: None,
            };
            create_session(&conn, &record).unwrap();
        }

        // Create 1 terminated session
        let terminated_record = SessionRecord {
            session_id: Uuid::new_v4(),
            created_at: "2026-08-19T10:02:00Z".to_string(),
            last_accessed_at: "2026-08-19T10:02:00Z".to_string(),
            status: SessionStatus::Terminated,
            shell_path: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            env_vars: None,
            rows: 24,
            cols: 80,
            owner_user_id: None,
            acl: None,
            metadata: None,
        };
        create_session(&conn, &terminated_record).unwrap();

        let active = list_active_sessions(&conn).unwrap();
        assert_eq!(active.len(), 2); // Only running sessions
    }

    #[test]
    fn test_delete_session() {
        let mut conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let record = SessionRecord {
            session_id,
            created_at: "2026-08-19T10:00:00Z".to_string(),
            last_accessed_at: "2026-08-19T10:00:00Z".to_string(),
            status: SessionStatus::Running,
            shell_path: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            env_vars: None,
            rows: 24,
            cols: 80,
            owner_user_id: None,
            acl: None,
            metadata: None,
        };

        create_session(&conn, &record).unwrap();
        delete_session(&mut conn, &session_id).unwrap();

        // Should not exist
        let result = load_session(&conn, &session_id);
        assert!(result.is_err());
    }
}
