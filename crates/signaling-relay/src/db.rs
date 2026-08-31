use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
    db_path: PathBuf,
}

impl Database {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).context("failed to create database directory")?;
            }
        }

        let manager = SqliteConnectionManager::file(&db_path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;",
            )?;
            Ok(())
        });

        let pool = Pool::builder()
            .max_size(10)
            .connection_timeout(Duration::from_secs(30))
            .build(manager)
            .context("failed to create connection pool")?;

        {
            let conn = pool.get().context("failed to get connection from pool")?;

            // Pre-OAuth installs have an integer-keyed `users` table with a
            // local password hash. Identity now comes from monoes.me
            // (TEXT ids), so those rows and their FKs are no longer
            // meaningful — rebuild rather than migrate in place.
            let schema_version: i64 =
                conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if schema_version < 1 {
                conn.execute_batch(
                    "PRAGMA foreign_keys = OFF;
                     DROP TABLE IF EXISTS linked_computers;
                     DROP TABLE IF EXISTS users;
                     PRAGMA user_version = 1;
                     PRAGMA foreign_keys = ON;",
                )
                .context("failed to migrate to OAuth-backed schema")?;
            }

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    email TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS linked_computers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id TEXT NOT NULL REFERENCES users(id),
                    peer_id TEXT NOT NULL UNIQUE,
                    name TEXT NOT NULL,
                    linked_at INTEGER NOT NULL,
                    last_seen_at INTEGER
                );

                CREATE TABLE IF NOT EXISTS pairing_codes (
                    code TEXT PRIMARY KEY,
                    peer_id TEXT NOT NULL,
                    expires_at INTEGER NOT NULL,
                    consumed INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS oauth_states (
                    state TEXT PRIMARY KEY,
                    code_verifier TEXT NOT NULL,
                    return_to TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            )
            .context("failed to initialize schema")?;
        }

        Ok(Database { pool, db_path })
    }

    pub fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .context("failed to get connection from pool")
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }
}
