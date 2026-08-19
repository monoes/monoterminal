//! Integration Tests: Persistence + SessionManager
//! Task: task-37 | Date: 2026-08-19
//!
//! Tests the integration between the persistence layer and session management.
//! Covers: Session persistence, recovery, multi-session state, and ACL functionality.

use anyhow::Result;
use monoterminal_master::persistence::{
    session::{SessionRecord, SessionStatus},
    Database,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

// ===== Test Helpers =====

fn setup_test_db() -> Result<(TempDir, Database)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path)?;
    Ok((temp_dir, db))
}

fn create_test_record(
    session_id: Option<Uuid>,
    status: SessionStatus,
    owner: Option<String>,
    acl: Option<HashMap<String, String>>,
) -> SessionRecord {
    let now = chrono::Utc::now().to_rfc3339();
    SessionRecord {
        session_id: session_id.unwrap_or_else(Uuid::new_v4),
        created_at: now.clone(),
        last_accessed_at: now,
        status,
        shell_path: "cmd.exe".to_string(),
        working_dir: PathBuf::from("C:\\test"),
        env_vars: None,
        rows: 24,
        cols: 80,
        owner_user_id: owner,
        acl,
        metadata: None,
    }
}

// ===== 1. Session State Persistence Tests =====

#[test]
fn test_create_and_persist_session() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(
        Some(session_id),
        SessionStatus::Running,
        Some("alice@example.com".to_string()),
        None,
    );

    // Create
    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // Load
    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert_eq!(loaded.session_id, session_id);
    assert_eq!(loaded.status, SessionStatus::Running);
    assert_eq!(loaded.owner_user_id, Some("alice@example.com".to_string()));
    Ok(())
}

#[test]
fn test_update_session_status() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(Some(session_id), SessionStatus::Running, None, None);

    // Create
    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // Update
    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::update_session_status(
            &conn,
            &session_id,
            SessionStatus::Detached,
        )?;
    }

    // Verify
    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert_eq!(loaded.status, SessionStatus::Detached);
    Ok(())
}

#[test]
fn test_touch_session() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(Some(session_id), SessionStatus::Running, None, None);

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    std::thread::sleep(std::time::Duration::from_millis(10));

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::touch_session(&conn, &session_id)?;
    }

    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert_ne!(loaded.created_at, loaded.last_accessed_at);
    Ok(())
}

// ===== 2. Session Recovery Tests =====

#[test]
fn test_recover_active_sessions() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;

    // Create 3 sessions (2 running, 1 terminated)
    let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    for (i, id) in ids.iter().enumerate() {
        let status = if i == 2 {
            SessionStatus::Terminated
        } else {
            SessionStatus::Running
        };
        let record = create_test_record(
            Some(*id),
            status,
            Some(format!("user{}@example.com", i)),
            None,
        );

        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // List active
    let active = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::list_active_sessions(&conn)?
    };

    assert_eq!(active.len(), 2);
    let active_ids: Vec<Uuid> = active.iter().map(|s| s.session_id).collect();
    assert!(active_ids.contains(&ids[0]));
    assert!(active_ids.contains(&ids[1]));
    assert!(!active_ids.contains(&ids[2]));
    Ok(())
}

#[test]
fn test_recover_full_state() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();

    let mut env_vars = HashMap::new();
    env_vars.insert("PATH".to_string(), "/usr/bin".to_string());

    let metadata = serde_json::json!({"theme": "dark"});

    let record = SessionRecord {
        session_id,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_accessed_at: chrono::Utc::now().to_rfc3339(),
        status: SessionStatus::Detached,
        shell_path: "bash".to_string(),
        working_dir: PathBuf::from("/home/user"),
        env_vars: Some(env_vars.clone()),
        rows: 30,
        cols: 120,
        owner_user_id: Some("charlie@example.com".to_string()),
        acl: None,
        metadata: Some(metadata.clone()),
    };

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert_eq!(loaded.shell_path, "bash");
    assert_eq!(loaded.rows, 30);
    assert_eq!(loaded.cols, 120);
    assert!(loaded.env_vars.is_some());
    assert_eq!(
        loaded.env_vars.unwrap().get("PATH"),
        Some(&"/usr/bin".to_string())
    );
    assert_eq!(loaded.metadata.unwrap()["theme"], "dark");
    Ok(())
}

#[test]
fn test_missing_session() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let nonexistent = Uuid::new_v4();

    let result = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &nonexistent)
    };

    assert!(result.is_err());
    Ok(())
}

// ===== 3. Multi-Session State Management (ADR-013) =====

#[test]
fn test_multiple_concurrent_sessions() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;

    let sessions: Vec<(Uuid, String)> = vec![
        (Uuid::new_v4(), "user1@example.com".to_string()),
        (Uuid::new_v4(), "user1@example.com".to_string()), // Same user, different session
        (Uuid::new_v4(), "user2@example.com".to_string()),
    ];

    for (sid, owner) in &sessions {
        let record = create_test_record(
            Some(*sid),
            SessionStatus::Running,
            Some(owner.clone()),
            None,
        );
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let counts = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::count_sessions_by_status(&conn)?
    };

    assert_eq!(*counts.get(&SessionStatus::Running).unwrap_or(&0), 3);

    for (sid, owner) in &sessions {
        let loaded = {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::load_session(&conn, sid)?
        };
        assert_eq!(loaded.owner_user_id.as_ref(), Some(owner));
    }

    Ok(())
}

#[test]
fn test_session_isolation() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;

    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();

    let r1 = create_test_record(
        Some(s1),
        SessionStatus::Running,
        Some("alice@example.com".to_string()),
        None,
    );
    let r2 = create_test_record(
        Some(s2),
        SessionStatus::Detached,
        Some("bob@example.com".to_string()),
        None,
    );

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &r1)?;
        monoterminal_master::persistence::session::create_session(&conn, &r2)?;
    }

    // Update s1 only
    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::update_session_status(
            &conn,
            &s1,
            SessionStatus::Terminated,
        )?;
    }

    let loaded1 = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &s1)?
    };

    let loaded2 = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &s2)?
    };

    assert_eq!(loaded1.status, SessionStatus::Terminated);
    assert_eq!(loaded2.status, SessionStatus::Detached); // Unchanged
    Ok(())
}

// ===== 4. ACL Column Functionality Tests (CRITICAL) =====

#[test]
fn test_acl_persistence() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();

    let mut acl = HashMap::new();
    acl.insert("alice@example.com".to_string(), "owner".to_string());
    acl.insert("bob@example.com".to_string(), "read-write".to_string());

    let record = create_test_record(
        Some(session_id),
        SessionStatus::Running,
        Some("alice@example.com".to_string()),
        Some(acl.clone()),
    );

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert!(loaded.acl.is_some());
    let loaded_acl = loaded.acl.unwrap();
    assert_eq!(loaded_acl.len(), 2);
    assert_eq!(
        loaded_acl.get("alice@example.com"),
        Some(&"owner".to_string())
    );
    assert_eq!(
        loaded_acl.get("bob@example.com"),
        Some(&"read-write".to_string())
    );
    Ok(())
}

#[test]
fn test_acl_null() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(Some(session_id), SessionStatus::Running, None, None);

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert!(loaded.acl.is_none());
    Ok(())
}

#[test]
fn test_acl_empty() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let empty_acl = HashMap::new();
    let record = create_test_record(
        Some(session_id),
        SessionStatus::Running,
        None,
        Some(empty_acl),
    );

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert!(loaded.acl.is_some());
    assert_eq!(loaded.acl.unwrap().len(), 0);
    Ok(())
}

#[test]
fn test_multi_session_different_acls() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;

    let s1 = Uuid::new_v4();
    let mut acl1 = HashMap::new();
    acl1.insert("alice@example.com".to_string(), "owner".to_string());

    let s2 = Uuid::new_v4();
    let mut acl2 = HashMap::new();
    acl2.insert("bob@example.com".to_string(), "owner".to_string());
    acl2.insert("alice@example.com".to_string(), "read-only".to_string());

    let s3 = Uuid::new_v4();

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(
            &conn,
            &create_test_record(
                Some(s1),
                SessionStatus::Running,
                Some("alice@example.com".to_string()),
                Some(acl1),
            ),
        )?;
        monoterminal_master::persistence::session::create_session(
            &conn,
            &create_test_record(
                Some(s2),
                SessionStatus::Running,
                Some("bob@example.com".to_string()),
                Some(acl2),
            ),
        )?;
        monoterminal_master::persistence::session::create_session(
            &conn,
            &create_test_record(
                Some(s3),
                SessionStatus::Running,
                Some("charlie@example.com".to_string()),
                None,
            ),
        )?;
    }

    let loaded1 = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &s1)?
    };
    let loaded2 = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &s2)?
    };
    let loaded3 = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &s3)?
    };

    // Verify ACL isolation
    assert_eq!(loaded1.acl.as_ref().unwrap().len(), 1);
    assert_eq!(loaded2.acl.as_ref().unwrap().len(), 2);
    assert!(loaded3.acl.is_none());

    assert_eq!(
        loaded1.acl.unwrap().get("alice@example.com"),
        Some(&"owner".to_string())
    );
    assert_eq!(
        loaded2.acl.unwrap().get("alice@example.com"),
        Some(&"read-only".to_string())
    );

    Ok(())
}

// ===== 5. Edge Cases =====

#[test]
fn test_session_deletion() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(Some(session_id), SessionStatus::Running, None, None);

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // Delete
    {
        let mut conn = db.get_conn()?;
        monoterminal_master::persistence::session::delete_session(&mut conn, &session_id)?;
    }

    // Verify gone
    let result = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)
    };

    assert!(result.is_err());
    Ok(())
}

#[test]
fn test_concurrent_access() -> Result<()> {
    let (_temp_dir, db) = setup_test_db()?;
    let session_id = Uuid::new_v4();
    let record = create_test_record(
        Some(session_id),
        SessionStatus::Running,
        Some("concurrent@example.com".to_string()),
        None,
    );

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let db_arc = std::sync::Arc::new(db);
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let db_clone = db_arc.clone();
            let sid = session_id;

            std::thread::spawn(move || {
                for _ in 0..10 {
                    let conn = db_clone.get_conn().expect("Failed to get conn");
                    let result =
                        monoterminal_master::persistence::session::load_session(&conn, &sid);
                    assert!(result.is_ok(), "Thread {} failed to load", i);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    Ok(())
}
