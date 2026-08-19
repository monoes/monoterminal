//! Audit logging for compliance
//! ADR-012 §2.4: Audit Logs Table

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuditEvent {
    SessionCreate {
        session_id: Uuid,
        shell_path: String,
    },
    SessionAttach {
        session_id: Uuid,
        client_id: Uuid,
    },
    SessionDetach {
        session_id: Uuid,
        client_id: Uuid,
    },
    Input {
        session_id: Uuid,
        data_length: usize, // NOT raw input (privacy)
    },
    SessionTerminate {
        session_id: Uuid,
        exit_code: Option<i32>,
    },
}

impl AuditEvent {
    /// Get the event type as a string
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::SessionCreate { .. } => "SESSION_CREATE",
            AuditEvent::SessionAttach { .. } => "SESSION_ATTACH",
            AuditEvent::SessionDetach { .. } => "SESSION_DETACH",
            AuditEvent::Input { .. } => "INPUT",
            AuditEvent::SessionTerminate { .. } => "SESSION_TERMINATE",
        }
    }

    /// Get the session ID for this event
    pub fn session_id(&self) -> Uuid {
        match self {
            AuditEvent::SessionCreate { session_id, .. } => *session_id,
            AuditEvent::SessionAttach { session_id, .. } => *session_id,
            AuditEvent::SessionDetach { session_id, .. } => *session_id,
            AuditEvent::Input { session_id, .. } => *session_id,
            AuditEvent::SessionTerminate { session_id, .. } => *session_id,
        }
    }
}

/// Log an audit event
///
/// ADR-012: "Log input length, NOT raw input (prevents password leaks in audit logs)"
pub fn log_audit_event(
    conn: &Connection,
    event: AuditEvent,
    user_id: Option<&str>,
    client_id: Option<&str>,
    ip_address: Option<&str>,
) -> Result<()> {
    let event_type = event.event_type();
    let session_id = event.session_id();
    let payload = serde_json::to_string(&event)?;

    conn.execute(
        "INSERT INTO audit_logs (event_type, session_id, user_id, client_id, ip_address, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event_type,
            session_id.to_string(),
            user_id,
            client_id,
            ip_address,
            payload,
        ],
    )?;

    tracing::debug!("Audit log: {} for session {}", event_type, session_id);
    Ok(())
}

/// Query audit logs for a session
pub fn query_session_audit_logs(
    conn: &Connection,
    session_id: &Uuid,
    limit: usize,
) -> Result<Vec<AuditLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, event_type, session_id, user_id, client_id, ip_address, payload
         FROM audit_logs
         WHERE session_id = ?1
         ORDER BY timestamp DESC
         LIMIT ?2",
    )?;

    let logs = stmt
        .query_map(params![session_id.to_string(), limit], |row| {
            Ok(AuditLogEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                event_type: row.get(2)?,
                session_id: Uuid::parse_str(&row.get::<_, String>(3)?).unwrap(),
                user_id: row.get(4)?,
                client_id: row.get(5)?,
                ip_address: row.get(6)?,
                payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(logs)
}

/// Query audit logs for a user
pub fn query_user_audit_logs(
    conn: &Connection,
    user_id: &str,
    limit: usize,
) -> Result<Vec<AuditLogEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, event_type, session_id, user_id, client_id, ip_address, payload
         FROM audit_logs
         WHERE user_id = ?1
         ORDER BY timestamp DESC
         LIMIT ?2",
    )?;

    let logs = stmt
        .query_map(params![user_id, limit], |row| {
            Ok(AuditLogEntry {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                event_type: row.get(2)?,
                session_id: Uuid::parse_str(&row.get::<_, String>(3)?).unwrap(),
                user_id: row.get(4)?,
                client_id: row.get(5)?,
                ip_address: row.get(6)?,
                payload: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(logs)
}

/// Delete old audit logs (retention policy)
/// ADR-012: "Phase 4: Configurable retention (90 days default, auto-delete older entries)"
pub fn delete_old_audit_logs(conn: &Connection, retention_days: i64) -> Result<u64> {
    let deleted = conn.execute(
        "DELETE FROM audit_logs
         WHERE timestamp < datetime('now', ?1)",
        params![format!("-{} days", retention_days)],
    )?;

    tracing::info!(
        "Deleted {} old audit log entries (retention: {} days)",
        deleted,
        retention_days
    );
    Ok(deleted as u64)
}

/// Get audit log count
pub fn count_audit_logs(conn: &Connection) -> Result<u64> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))?;
    Ok(count as u64)
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub event_type: String,
    pub session_id: Uuid,
    pub user_id: Option<String>,
    pub client_id: Option<String>,
    pub ip_address: Option<String>,
    pub payload: String,
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
    fn test_log_audit_event() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        let event = AuditEvent::SessionCreate {
            session_id,
            shell_path: "cmd.exe".to_string(),
        };

        log_audit_event(&conn, event, Some("alice@example.com"), None, None).unwrap();

        let count = count_audit_logs(&conn).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_query_session_audit_logs() {
        let conn = setup_test_db();
        let session_id = Uuid::new_v4();

        // Log multiple events
        for i in 0..5 {
            let event = AuditEvent::Input {
                session_id,
                data_length: i,
            };
            log_audit_event(&conn, event, Some("alice@example.com"), None, None).unwrap();
        }

        let logs = query_session_audit_logs(&conn, &session_id, 10).unwrap();
        assert_eq!(logs.len(), 5);
    }

    #[test]
    fn test_query_user_audit_logs() {
        let conn = setup_test_db();

        // Log events for different users
        for _i in 0..3 {
            let session_id = Uuid::new_v4();
            let event = AuditEvent::SessionCreate {
                session_id,
                shell_path: "cmd.exe".to_string(),
            };
            log_audit_event(&conn, event, Some("alice@example.com"), None, None).unwrap();
        }

        let logs = query_user_audit_logs(&conn, "alice@example.com", 10).unwrap();
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn test_privacy_preserving() {
        // Verify that Input event does NOT contain raw input
        let session_id = Uuid::new_v4();
        let event = AuditEvent::Input {
            session_id,
            data_length: 100, // Only length, not data
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("password")); // Should not contain sensitive data
        assert!(json.contains("data_length")); // Should only have length
    }
}
