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
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    email TEXT NOT NULL UNIQUE,
                    password_hash TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS linked_computers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL REFERENCES users(id),
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
