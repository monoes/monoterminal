//! Performance benchmarks for persistence layer
//! ADR-012 Performance Targets:
//! - Single insert: 10k/s
//! - Batched insert: 100k/s
//! - Indexed SELECT: <1ms
//! - Scrollback fetch 1000 lines: <100ms p95 (including decompression)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use monoterminal_master::persistence::*;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

fn setup_test_db() -> (TempDir, Database) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("bench.db");
    let db = Database::new(&db_path).unwrap();
    (temp_dir, db)
}

fn bench_session_create(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();

    c.bench_function("session_create", |b| {
        b.iter(|| {
            let conn = db.get_conn().unwrap();
            let session_id = Uuid::new_v4();

            let record = session::SessionRecord {
                session_id,
                created_at: "2026-08-19T10:00:00Z".to_string(),
                last_accessed_at: "2026-08-19T10:00:00Z".to_string(),
                status: session::SessionStatus::Running,
                shell_path: "cmd.exe".to_string(),
                working_dir: PathBuf::from("C:\\Users\\Test"),
                env_vars: None,
                rows: 24,
                cols: 80,
                owner_user_id: None,
                acl: None,
                metadata: None,
            };

            session::create_session(&conn, &record).unwrap();
        });
    });
}

fn bench_session_load(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();

    // Pre-create a session
    let session_id = Uuid::new_v4();
    {
        let conn = db.get_conn().unwrap();
        let record = session::SessionRecord {
            session_id,
            created_at: "2026-08-19T10:00:00Z".to_string(),
            last_accessed_at: "2026-08-19T10:00:00Z".to_string(),
            status: session::SessionStatus::Running,
            shell_path: "cmd.exe".to_string(),
            working_dir: PathBuf::from("C:\\"),
            env_vars: None,
            rows: 24,
            cols: 80,
            owner_user_id: None,
            acl: None,
            metadata: None,
        };
        session::create_session(&conn, &record).unwrap();
    }

    c.bench_function("session_load", |b| {
        b.iter(|| {
            let conn = db.get_conn().unwrap();
            let loaded = session::load_session(&conn, &session_id).unwrap();
            black_box(loaded);
        });
    });
}

fn bench_scrollback_single_insert(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();

    c.bench_function("scrollback_single_insert", |b| {
        b.iter(|| {
            // Use unique session_id + line_number per iteration to avoid UNIQUE constraint violations
            let session_id = Uuid::new_v4();
            let conn = db.get_conn().unwrap();
            let line = scrollback::ScrollbackLine {
                session_id,
                line_number: 0,
                data: b"INFO: Processing file.txt\n".to_vec(),
                timestamp_ms: scrollback::now_millis(),
                sequence_number: 0,
            };
            scrollback::store_line(&conn, &line).unwrap();
        });
    });
}

fn bench_scrollback_batch_insert(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();

    let mut group = c.benchmark_group("scrollback_batch_insert");

    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    // Use unique session_id per iteration to avoid UNIQUE constraint violations
                    let session_id = Uuid::new_v4();
                    let mut conn = db.get_conn().unwrap();
                    let lines: Vec<_> = (0..batch_size)
                        .map(|i| {
                            let line = scrollback::ScrollbackLine {
                                session_id,
                                line_number: i as u64,
                                data: format!("INFO: Processing file_{}.txt\n", i).into_bytes(),
                                timestamp_ms: scrollback::now_millis(),
                                sequence_number: i as u64,
                            };
                            line
                        })
                        .collect();

                    scrollback::store_lines_batch(&mut conn, &lines).unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_scrollback_fetch(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();
    let session_id = Uuid::new_v4();

    // Pre-populate with 10k lines
    {
        let mut conn = db.get_conn().unwrap();
        let lines: Vec<_> = (0..10000)
            .map(|i| scrollback::ScrollbackLine {
                session_id,
                line_number: i,
                data: format!("INFO: Log line {}\n", i).into_bytes(),
                timestamp_ms: scrollback::now_millis(),
                sequence_number: i,
            })
            .collect();

        // Insert in batches of 1000
        for chunk in lines.chunks(1000) {
            scrollback::store_lines_batch(&mut conn, chunk).unwrap();
        }
    }

    c.bench_function("scrollback_fetch_1000", |b| {
        b.iter(|| {
            let conn = db.get_conn().unwrap();
            let lines = scrollback::fetch_range(&conn, &session_id, 0, 1000).unwrap();
            black_box(lines);
        });
    });
}

fn bench_scrollback_compression(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();
    let session_id = Uuid::new_v4();

    // Large repetitive data (should compress well)
    let large_data = b"INFO: Build succeeded\n".repeat(100);

    c.bench_function("scrollback_store_compressed", |b| {
        b.iter(|| {
            // Use unique session_id per iteration to avoid UNIQUE constraint violations
            let session_id = Uuid::new_v4();
            let conn = db.get_conn().unwrap();
            let line = scrollback::ScrollbackLine {
                session_id,
                line_number: 0,
                data: large_data.clone(),
                timestamp_ms: scrollback::now_millis(),
                sequence_number: 0,
            };
            scrollback::store_line(&conn, &line).unwrap();
        });
    });

    // Pre-populate for decompression benchmark
    {
        let conn = db.get_conn().unwrap();
        for i in 0..100 {
            let line = scrollback::ScrollbackLine {
                session_id,
                line_number: i,
                data: large_data.clone(),
                timestamp_ms: scrollback::now_millis(),
                sequence_number: i,
            };
            scrollback::store_line(&conn, &line).unwrap();
        }
    }

    c.bench_function("scrollback_fetch_decompress_100", |b| {
        b.iter(|| {
            let conn = db.get_conn().unwrap();
            let lines = scrollback::fetch_range(&conn, &session_id, 0, 100).unwrap();
            black_box(lines);
        });
    });
}

fn bench_audit_log(c: &mut Criterion) {
    let (_temp_dir, db) = setup_test_db();

    c.bench_function("audit_log_create", |b| {
        b.iter(|| {
            let conn = db.get_conn().unwrap();
            let session_id = Uuid::new_v4();

            let event = audit::AuditEvent::SessionCreate {
                session_id,
                shell_path: "cmd.exe".to_string(),
            };

            audit::log_audit_event(&conn, event, Some("alice@example.com"), None, None).unwrap();
        });
    });
}

fn bench_backup(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let src_db = temp_dir.path().join("source.db");
    let backup_db = temp_dir.path().join("backup.db");

    // Create a database with some data
    {
        let db = Database::new(&src_db).unwrap();
        let conn = db.get_conn().unwrap();

        // Add 100 sessions
        for i in 0..100 {
            let session_id = Uuid::new_v4();
            let record = session::SessionRecord {
                session_id,
                created_at: format!("2026-08-19T10:{:02}:00Z", i % 60),
                last_accessed_at: format!("2026-08-19T10:{:02}:00Z", i % 60),
                status: session::SessionStatus::Running,
                shell_path: "cmd.exe".to_string(),
                working_dir: PathBuf::from("C:\\"),
                env_vars: None,
                rows: 24,
                cols: 80,
                owner_user_id: None,
                acl: None,
                metadata: None,
            };
            session::create_session(&conn, &record).unwrap();
        }
    }

    c.bench_function("backup_database", |b| {
        b.iter(|| {
            backup::backup_database(&src_db, &backup_db).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_session_create,
    bench_session_load,
    bench_scrollback_single_insert,
    bench_scrollback_batch_insert,
    bench_scrollback_fetch,
    bench_scrollback_compression,
    bench_audit_log,
    bench_backup,
);

criterion_main!(benches);
