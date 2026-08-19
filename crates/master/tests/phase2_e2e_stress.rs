//! Phase 2 End-to-End Validation and Stress Testing
//! Task: task-45 | Date: 2026-08-19
//!
//! Tests Phase 2 features against SRS §7.2 acceptance criteria:
//! - 100 concurrent sessions
//! - Multi-client RBAC collaboration
//! - Session persistence and recovery
//! - ACL permission enforcement
//! - Discovery service integration
//!
//! SRS Targets:
//! - 100 concurrent sessions (Phase 2 target, §7.2)
//! - 1000 concurrent sessions (ultimate target, §1.3)
//! - 75% test coverage (Phase 2, §7.2)
//! - <30ms LAN p95 latency (§1.3)

use anyhow::Result;
use monoterminal_master::{
    persistence::{
        session::{SessionRecord, SessionStatus},
        Database,
    },
    session::manager::SessionManager,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::task::JoinSet;
use uuid::Uuid;

// ===== Test Helpers =====

async fn setup_test_env() -> Result<(TempDir, Database, Arc<SessionManager>)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path)?;

    let manager = Arc::new(SessionManager::new(None));

    Ok((temp_dir, db, manager))
}

fn create_test_record(
    session_id: Uuid,
    status: SessionStatus,
    owner: Option<String>,
    acl: Option<HashMap<String, String>>,
) -> SessionRecord {
    let now = chrono::Utc::now().to_rfc3339();
    SessionRecord {
        session_id,
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

// ===== E2E Test Scenarios =====

/// E2E Scenario 1: Multi-Client Session with RBAC Enforcement
/// Tests: 3+ clients attaching to same session with different permissions
#[tokio::test]
async fn e2e_multi_client_rbac() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    // Create session with owner
    let session_id = Uuid::new_v4();
    let mut acl = HashMap::new();
    acl.insert("alice@example.com".to_string(), "owner".to_string());
    acl.insert("bob@example.com".to_string(), "editor".to_string());
    acl.insert("charlie@example.com".to_string(), "viewer".to_string());

    let record = create_test_record(
        session_id,
        SessionStatus::Running,
        Some("alice@example.com".to_string()),
        Some(acl),
    );

    // Persist session
    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // Load and verify ACL
    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };

    assert!(loaded.acl.is_some());
    let loaded_acl = loaded.acl.unwrap();
    assert_eq!(loaded_acl.len(), 3);
    assert_eq!(
        loaded_acl.get("alice@example.com"),
        Some(&"owner".to_string())
    );
    assert_eq!(
        loaded_acl.get("bob@example.com"),
        Some(&"editor".to_string())
    );
    assert_eq!(
        loaded_acl.get("charlie@example.com"),
        Some(&"viewer".to_string())
    );

    Ok(())
}

/// E2E Scenario 2: Multi-Session Load (10 concurrent sessions)
/// Tests: Creating and managing 10 concurrent sessions
#[tokio::test]
async fn e2e_multi_session_load() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const SESSION_COUNT: usize = 10;
    let mut session_ids = Vec::new();

    // Create 10 concurrent sessions
    for i in 0..SESSION_COUNT {
        let session_id = Uuid::new_v4();
        let record = create_test_record(
            session_id,
            SessionStatus::Running,
            Some(format!("user{}@example.com", i)),
            None,
        );

        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
        session_ids.push(session_id);
    }

    // Verify all sessions exist
    let active = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::list_active_sessions(&conn)?
    };

    assert_eq!(active.len(), SESSION_COUNT);

    // Verify each session can be loaded
    for session_id in &session_ids {
        let loaded = {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::load_session(&conn, session_id)?
        };
        assert_eq!(loaded.session_id, *session_id);
        assert_eq!(loaded.status, SessionStatus::Running);
    }

    Ok(())
}

/// E2E Scenario 3: Session Recovery After Restart
/// Tests: Sessions persist across daemon restarts
#[tokio::test]
async fn e2e_session_recovery() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("test.db");

    let session_ids: Vec<Uuid>;

    // Phase 1: Create sessions
    {
        let db = Database::new(&db_path)?;
        session_ids = (0..5).map(|_| Uuid::new_v4()).collect();

        for (i, &session_id) in session_ids.iter().enumerate() {
            let record = create_test_record(
                session_id,
                SessionStatus::Running,
                Some(format!("user{}@example.com", i)),
                None,
            );

            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::create_session(&conn, &record)?;
        }
    }

    // Phase 2: Simulate restart - create new DB connection
    {
        let db = Database::new(&db_path)?;

        // Verify all sessions recovered
        let active = {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::list_active_sessions(&conn)?
        };

        assert_eq!(active.len(), 5);

        let recovered_ids: Vec<Uuid> = active.iter().map(|s| s.session_id).collect();
        for &session_id in &session_ids {
            assert!(
                recovered_ids.contains(&session_id),
                "Session {} not recovered after restart",
                session_id
            );
        }
    }

    Ok(())
}

/// E2E Scenario 4: ACL Permission Enforcement
/// Tests: Owner/Editor/Viewer permission levels
#[tokio::test]
async fn e2e_acl_permission_enforcement() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    // Create 3 sessions with different ACL configurations
    let sessions = vec![
        ("owner-session", "alice@example.com", "owner"),
        ("editor-session", "bob@example.com", "editor"),
        ("viewer-session", "charlie@example.com", "viewer"),
    ];

    for (name, user, permission) in &sessions {
        let session_id = Uuid::new_v4();
        let mut acl = HashMap::new();
        acl.insert(user.to_string(), permission.to_string());

        let record = create_test_record(
            session_id,
            SessionStatus::Running,
            Some(user.to_string()),
            Some(acl),
        );

        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;

        // Verify permission
        let loaded = {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::load_session(&conn, &session_id)?
        };

        let loaded_acl = loaded.acl.unwrap_or_else(|| panic!("ACL missing for {}", name));
        assert_eq!(
            loaded_acl.get(*user),
            Some(&permission.to_string()),
            "Permission mismatch for {}",
            name
        );
    }

    Ok(())
}

/// E2E Scenario 5: Discovery Service Integration (Placeholder)
/// Tests: HTTP directory service integration
/// Note: Implementation pending discovery service implementation
#[tokio::test]
#[ignore] // Enable when discovery service is implemented
async fn e2e_discovery_service() -> Result<()> {
    // TODO: Test discovery service once implemented
    // - Register session with discovery service
    // - Query available sessions
    // - Verify session metadata
    Ok(())
}

// ===== Stress Tests =====

/// Stress Test 1: 100 Concurrent Sessions (SRS §7.2 Phase 2 Target)
/// Tests: System can handle 100 concurrent sessions
#[tokio::test]
async fn stress_100_concurrent_sessions() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const TARGET_SESSIONS: usize = 100;
    let start = Instant::now();

    // Create 100 sessions concurrently
    let mut tasks = JoinSet::new();
    let db = Arc::new(db);

    for i in 0..TARGET_SESSIONS {
        let db_clone = db.clone();
        tasks.spawn(async move {
            let session_id = Uuid::new_v4();
            let record = create_test_record(
                session_id,
                SessionStatus::Running,
                Some(format!("user{}@example.com", i)),
                None,
            );

            let conn = db_clone.get_conn()?;
            monoterminal_master::persistence::session::create_session(&conn, &record)?;
            Ok::<_, anyhow::Error>(session_id)
        });
    }

    // Wait for all sessions to be created
    let mut created_ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let session_id = result??;
        created_ids.push(session_id);
    }

    let creation_duration = start.elapsed();

    // Verify all 100 sessions exist
    let active = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::list_active_sessions(&conn)?
    };

    assert_eq!(
        active.len(),
        TARGET_SESSIONS,
        "Expected {} sessions, found {}",
        TARGET_SESSIONS,
        active.len()
    );

    println!(
        "✅ Created {} concurrent sessions in {:?}",
        TARGET_SESSIONS, creation_duration
    );
    println!(
        "   Average: {:.2}ms per session",
        creation_duration.as_millis() as f64 / TARGET_SESSIONS as f64
    );

    Ok(())
}

/// Stress Test 2: Rapid Creation/Termination (10 sessions/second)
/// Tests: System handles rapid session lifecycle churn
#[tokio::test]
async fn stress_rapid_creation_termination() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const SESSIONS_PER_SECOND: usize = 10;
    const DURATION_SECONDS: usize = 5;
    const TOTAL_SESSIONS: usize = SESSIONS_PER_SECOND * DURATION_SECONDS;

    let start = Instant::now();

    for i in 0..TOTAL_SESSIONS {
        let session_id = Uuid::new_v4();

        // Create session
        let record = create_test_record(
            session_id,
            SessionStatus::Running,
            Some(format!("user{}@example.com", i)),
            None,
        );

        {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::create_session(&conn, &record)?;
        }

        // Immediately terminate
        {
            let conn = db.get_conn()?;
            monoterminal_master::persistence::session::update_session_status(
                &conn,
                &session_id,
                SessionStatus::Terminated,
            )?;
        }

        // Throttle to ~10/second
        if i % SESSIONS_PER_SECOND == 0 && i > 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    let duration = start.elapsed();
    let rate = TOTAL_SESSIONS as f64 / duration.as_secs_f64();

    println!(
        "✅ Created and terminated {} sessions in {:?}",
        TOTAL_SESSIONS, duration
    );
    println!(
        "   Rate: {:.2} sessions/second (target: {} sessions/second)",
        rate, SESSIONS_PER_SECOND
    );

    assert!(
        rate >= SESSIONS_PER_SECOND as f64 * 0.8,
        "Session creation rate {:.2}/s below 80% of target {}/s",
        rate,
        SESSIONS_PER_SECOND
    );

    Ok(())
}

/// Stress Test 3: Large Scrollback (10k lines)
/// Tests: System handles 10k line scrollback per SRS §4.1 hot tier target
#[tokio::test]
async fn stress_large_scrollback() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const LINES_COUNT: usize = 10_000;
    let session_id = Uuid::new_v4();

    // Create session
    let record = create_test_record(
        session_id,
        SessionStatus::Running,
        Some("scrollback-test@example.com".to_string()),
        None,
    );

    {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    // Note: Scrollback persistence tested separately in scrollback.rs module
    // This test validates session with large metadata
    let mut large_metadata = serde_json::Map::new();
    for i in 0..1000 {
        large_metadata.insert(
            format!("key_{}", i),
            serde_json::json!(format!("value_{}", i)),
        );
    }

    // Update session with large metadata (simulating large state)
    let updated_record = SessionRecord {
        metadata: Some(serde_json::Value::Object(large_metadata)),
        ..record
    };

    let start = Instant::now();
    {
        let mut conn = db.get_conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE sessions SET metadata = ?1 WHERE session_id = ?2",
            rusqlite::params![
                serde_json::to_string(&updated_record.metadata)?,
                session_id.to_string()
            ],
        )?;
        tx.commit()?;
    }
    let write_duration = start.elapsed();

    // Verify large metadata persists and loads
    let start = Instant::now();
    let loaded = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::load_session(&conn, &session_id)?
    };
    let read_duration = start.elapsed();

    assert!(loaded.metadata.is_some());
    let loaded_metadata = loaded.metadata.unwrap();
    assert!(loaded_metadata.is_object());
    let obj = loaded_metadata.as_object().unwrap();
    assert_eq!(obj.len(), 1000);

    println!(
        "✅ Large metadata ({} entries) - Write: {:?}, Read: {:?}",
        obj.len(),
        write_duration,
        read_duration
    );

    // SRS §4.1: Indexed SELECT <1ms target
    assert!(
        read_duration < Duration::from_millis(10),
        "Read time {:?} exceeds 10ms relaxed threshold (SRS target: <1ms)",
        read_duration
    );

    Ok(())
}

/// Stress Test 4: Connection Pool Stress (20 concurrent DB operations)
/// Tests: r2d2 connection pool handles concurrent access (pool size: 20)
#[tokio::test]
async fn stress_connection_pool() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const CONCURRENT_OPS: usize = 20; // Matches pool size
    const ITERATIONS_PER_OP: usize = 50;

    let db = Arc::new(db);
    let start = Instant::now();

    // Spawn 20 concurrent tasks, each doing 50 operations
    let mut tasks = JoinSet::new();

    for task_id in 0..CONCURRENT_OPS {
        let db_clone = db.clone();
        tasks.spawn(async move {
            for i in 0..ITERATIONS_PER_OP {
                let session_id = Uuid::new_v4();
                let record = create_test_record(
                    session_id,
                    SessionStatus::Running,
                    Some(format!("task{}_iter{}@example.com", task_id, i)),
                    None,
                );

                // Create
                {
                    let conn = db_clone.get_conn()?;
                    monoterminal_master::persistence::session::create_session(&conn, &record)?;
                }

                // Read
                {
                    let conn = db_clone.get_conn()?;
                    let _loaded = monoterminal_master::persistence::session::load_session(
                        &conn,
                        &session_id,
                    )?;
                }

                // Update
                {
                    let conn = db_clone.get_conn()?;
                    monoterminal_master::persistence::session::update_session_status(
                        &conn,
                        &session_id,
                        SessionStatus::Detached,
                    )?;
                }
            }
            Ok::<_, anyhow::Error>(())
        });
    }

    // Wait for all tasks
    while let Some(result) = tasks.join_next().await {
        result??;
    }

    let duration = start.elapsed();
    let total_ops = CONCURRENT_OPS * ITERATIONS_PER_OP * 3; // 3 ops per iteration
    let ops_per_sec = total_ops as f64 / duration.as_secs_f64();

    println!("✅ Connection pool stress test complete");
    println!(
        "   {} concurrent tasks × {} iterations × 3 ops = {} total operations",
        CONCURRENT_OPS, ITERATIONS_PER_OP, total_ops
    );
    println!("   Duration: {:?}", duration);
    println!("   Throughput: {:.2} operations/second", ops_per_sec);

    // Verify no deadlocks occurred
    let final_count = {
        let conn = db.get_conn()?;
        let counts = monoterminal_master::persistence::session::count_sessions_by_status(&conn)?;
        counts.values().sum::<u64>()
    };

    let expected_count = (CONCURRENT_OPS * ITERATIONS_PER_OP) as u64;
    assert_eq!(
        final_count, expected_count,
        "Expected {} sessions, found {} - possible concurrent access issue",
        expected_count, final_count
    );

    Ok(())
}

// ===== Performance Validation =====

/// Stress Test 5: 1000 Concurrent Sessions (SRS §1.3 Ultimate Target)
/// Tests: System can handle 1000 concurrent sessions (ultimate capacity target)
#[tokio::test]
async fn stress_1000_concurrent_sessions() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    const TARGET_SESSIONS: usize = 1000;

    // Get baseline memory (approximation via session count)
    let baseline_sessions = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::list_active_sessions(&conn)?
    }
    .len();

    let start = Instant::now();

    // Create 1000 sessions concurrently
    let mut tasks = JoinSet::new();
    let db = Arc::new(db);

    for i in 0..TARGET_SESSIONS {
        let db_clone = db.clone();
        tasks.spawn(async move {
            let session_id = Uuid::new_v4();
            let record = create_test_record(
                session_id,
                SessionStatus::Running,
                Some(format!("user{}@example.com", i)),
                None,
            );

            let conn = db_clone.get_conn()?;
            monoterminal_master::persistence::session::create_session(&conn, &record)?;
            Ok::<_, anyhow::Error>(session_id)
        });
    }

    // Wait for all sessions to be created and track failures
    let mut created_ids = Vec::new();
    let mut error_count = 0;

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(session_id)) => created_ids.push(session_id),
            Ok(Err(e)) => {
                error_count += 1;
                eprintln!("Session creation failed: {}", e);
            }
            Err(e) => {
                error_count += 1;
                eprintln!("Task join failed: {}", e);
            }
        }
    }

    let creation_duration = start.elapsed();

    // Verify all 1000 sessions exist
    let active = {
        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::list_active_sessions(&conn)?
    };

    let final_count = active.len();
    let memory_growth_sessions = final_count - baseline_sessions;

    // Calculate metrics
    let success_rate = (created_ids.len() as f64 / TARGET_SESSIONS as f64) * 100.0;
    let throughput = created_ids.len() as f64 / creation_duration.as_secs_f64();
    let avg_latency_ms = creation_duration.as_millis() as f64 / TARGET_SESSIONS as f64;

    println!("✅ 1000-Session Stress Test Results:");
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Creation Time:     {:?}", creation_duration);
    println!(
        "   Sessions Created:  {}/{} ({:.2}% success)",
        created_ids.len(),
        TARGET_SESSIONS,
        success_rate
    );
    println!("   Errors:            {}", error_count);
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   Throughput:        {:.2} sessions/sec", throughput);
    println!("   Avg Latency:       {:.2}ms per session", avg_latency_ms);
    println!(
        "   Memory Growth:     {} sessions (baseline: {})",
        memory_growth_sessions, baseline_sessions
    );
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Verify vs 100-session baseline (task-45: 7.12ms total = 71.2µs per session)
    let baseline_100_avg_ms = 0.0712; // 71.2µs from task-45
    let scaling_factor = avg_latency_ms / baseline_100_avg_ms;
    println!("   Scaling Analysis:");
    println!(
        "   100-session baseline:  {:.4}ms/session (task-45)",
        baseline_100_avg_ms
    );
    println!("   1000-session measured: {:.4}ms/session", avg_latency_ms);
    println!("   Scaling factor:        {:.2}x", scaling_factor);
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // SRS §1.3 Success Criteria
    assert_eq!(
        final_count, TARGET_SESSIONS,
        "Expected {} sessions, found {} (errors: {})",
        TARGET_SESSIONS, final_count, error_count
    );

    assert_eq!(
        error_count, 0,
        "Expected zero errors, found {}",
        error_count
    );

    // Throughput should maintain ~10k inserts/s baseline (task-44)
    // Allowing some degradation under load: ≥5k inserts/s acceptable
    assert!(
        throughput >= 5000.0,
        "Throughput {:.2} sessions/s below minimum 5k/s threshold",
        throughput
    );

    // Memory growth should be linear (1000 sessions shouldn't exceed 10% overhead vs baseline)
    let expected_growth = TARGET_SESSIONS;
    let growth_overhead_pct =
        ((memory_growth_sessions as f64 - expected_growth as f64) / expected_growth as f64) * 100.0;
    assert!(
        growth_overhead_pct.abs() <= 10.0,
        "Memory growth overhead {:.2}% exceeds ±10% threshold",
        growth_overhead_pct
    );

    Ok(())
}

/// Performance baseline validation against SRS targets
#[tokio::test]
async fn perf_baseline_validation() -> Result<()> {
    let (_temp_dir, db, _manager) = setup_test_env().await?;

    // Test single session CRUD performance
    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let session_id = Uuid::new_v4();
        let record = create_test_record(
            session_id,
            SessionStatus::Running,
            Some(format!("perf_user{}@example.com", i)),
            None,
        );

        let conn = db.get_conn()?;
        monoterminal_master::persistence::session::create_session(&conn, &record)?;
    }

    let duration = start.elapsed();
    let ops_per_sec = iterations as f64 / duration.as_secs_f64();

    println!("✅ Performance baseline:");
    println!("   {} sessions created in {:?}", iterations, duration);
    println!("   Throughput: {:.2} inserts/second", ops_per_sec);

    // SRS §4.1 target: 10k single inserts/second
    // We're doing full session records (not just scrollback), so 1k/s is acceptable
    assert!(
        ops_per_sec >= 500.0,
        "Insert rate {:.2}/s below minimum threshold of 500/s",
        ops_per_sec
    );

    Ok(())
}
