# ADR-012: Persistence Layer Design

**Status:** Draft — Pending Phase 1 Gate  
**Date:** 2026-08-17  
**Deciders:** principal-architect, rust-engineer-storage  
**SRS Reference:** §4.1, §7.2 (SQLite Persistence)  
**Phase:** Phase 2 (P2P + Persistence)

---

## Context

Phase 2 introduces persistent storage for:
- **Session state:** Preserve sessions across daemon restarts (no lost work on reboot)
- **Terminal scrollback:** Fetch history from disk (100k+ lines, gigabytes of logs)
- **Configuration:** User preferences, keybindings, color schemes
- **Audit logs:** Session creation, attachment, input events (compliance requirement)

**Current State (Phase 1):**
- **In-memory only:** Sessions lost on daemon restart
- **Limited scrollback:** Last 10k lines (ring buffer in RAM)
- **No audit trail:** No record of who accessed which session

**Phase 2 Requirements (SRS §4.1, §7.2):**
- SQLite database for all persistent state
- zstd compression for scrollback (reduce disk usage by 60-80%)
- Schema migrations (forward-compatible evolution)
- 100 concurrent sessions tested (read/write performance)

---

## Decision

Implement **SQLite-based persistence layer** with the following schema design:

---

## 1. Database Organization

### 1.1 File Layout

**Dual-mode paths (per ADR-005):**

| Mode | Base Path | Database File |
|------|-----------|---------------|
| **Service** | `%ProgramData%\MONOTERMINAL\` | `data\monoterminal.db` |
| **Console** | `%LOCALAPPDATA%\monoterminal\` | `data\monoterminal.db` |

**Example paths:**
- Service: `C:\ProgramData\MONOTERMINAL\data\monoterminal.db`
- Console: `C:\Users\Alice\AppData\Local\monoterminal\data\monoterminal.db`

**Write-Ahead Log (WAL) mode:**
```sql
-- Enable WAL for better concurrency
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;  -- Balance durability vs performance
```

**Rationale:**
- ✅ **WAL mode:** Readers don't block writers (Phase 2 requirement: 100 concurrent sessions)
- ✅ **NORMAL sync:** ~10x faster writes vs FULL, acceptable data loss window (<1 second on crash)
- ✅ **Single file:** Simpler backup/restore (just copy `monoterminal.db`)

---

### 1.2 Schema Version Management

**Schema version table:**
```sql
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL,  -- ISO 8601 timestamp
    description TEXT NOT NULL
);

-- Bootstrap: Insert initial version
INSERT INTO schema_migrations (version, applied_at, description)
VALUES (1, datetime('now'), 'Initial Phase 2 schema');
```

**Migration strategy:**
- **Forward-only migrations:** v1 → v2 → v3 (never rollback)
- **Idempotent scripts:** Each migration checks `schema_migrations` before applying
- **Startup validation:** Daemon verifies schema version matches expected, applies pending migrations

**Example migration (v1 → v2):**
```rust
// migrations/002_add_session_tags.sql
BEGIN;

-- Check current version
SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1;
-- If version >= 2, skip this migration

-- Add new column (backward compatible)
ALTER TABLE sessions ADD COLUMN tags TEXT;  -- JSON array of tags

-- Record migration
INSERT INTO schema_migrations (version, applied_at, description)
VALUES (2, datetime('now'), 'Add session tags');

COMMIT;
```

**Migration tool:**
```bash
# Apply pending migrations
monoterminal db migrate

# Show current schema version
monoterminal db version
```

---

## 2. Core Schema Design

### 2.1 Sessions Table

**Primary table for session metadata:**

```sql
CREATE TABLE sessions (
    -- Primary key
    session_id TEXT PRIMARY KEY,  -- UUID v4
    
    -- Session lifecycle
    created_at TEXT NOT NULL,     -- ISO 8601 timestamp
    last_accessed_at TEXT NOT NULL,
    status TEXT NOT NULL,         -- 'RUNNING' | 'DETACHED' | 'TERMINATED'
    
    -- PTY configuration
    shell_path TEXT NOT NULL,     -- e.g., 'C:\Windows\System32\cmd.exe'
    working_dir TEXT NOT NULL,
    env_vars TEXT,                -- JSON object of environment variables
    
    -- Terminal dimensions
    rows INTEGER NOT NULL,
    cols INTEGER NOT NULL,
    
    -- Ownership & metadata
    owner_user_id TEXT,           -- From JWT 'sub' claim (nullable for Phase 1 compat)
    acl TEXT,                     -- JSON-encoded ACL: {"user@example.com": "editor", "bob@example.com": "viewer"} (Phase 2+)
    metadata TEXT                 -- JSON object: {"name": "build-server", "tags": ["work", "linux"]}
);

-- Indexes for common queries
CREATE INDEX idx_sessions_status ON sessions(status);
CREATE INDEX idx_sessions_owner ON sessions(owner_user_id);
CREATE INDEX idx_sessions_last_accessed ON sessions(last_accessed_at);
```

**Design decisions:**

**Q1: Why TEXT for session_id instead of INTEGER?**
- ✅ **UUID portability:** Clients generate UUIDs (no server round-trip needed)
- ✅ **Merge-friendly:** No ID conflicts if multiple masters sync (future Phase 4+)
- ❌ **Trade-off:** 36 bytes (TEXT) vs 8 bytes (INTEGER), but readable in logs

**Q2: Why TEXT for timestamps instead of INTEGER (Unix epoch)?**
- ✅ **SQLite native:** `datetime('now')` generates ISO 8601, human-readable in queries
- ✅ **Timezone-aware:** Stores UTC, avoids local timezone bugs
- ❌ **Trade-off:** 20 bytes (TEXT) vs 8 bytes (INTEGER), but better debugging

**Q3: Should env_vars be separate table (normalized)?**
- ❌ **Rejected:** Env vars rarely queried individually, JSON blob is simpler
- ✅ **JSON blob:** Easier to serialize from Rust HashMap, avoid N+1 query problem

**Q4: Why separate acl column instead of in metadata JSON?**
- ✅ **Security-critical:** ACL should be explicit, not hidden in generic metadata blob
- ✅ **Query performance:** Filtering sessions by user permission easier with dedicated column
- ✅ **Schema clarity:** RBAC is core feature (Phase 2+), deserves explicit column
- ❌ **Trade-off:** One more column, but better than overloading metadata

---

### 2.2 Scrollback Storage

**Challenge:** 100k lines × 100 sessions = 10M rows, gigabytes of storage

**Two-tier strategy:**

**Tier 1: Recent scrollback (hot storage, in-memory)**
- Last 10k lines per session (Phase 1 ring buffer, unchanged)
- SQLite backed for persistence, but loaded into RAM on daemon start

**Tier 2: Historical scrollback (cold storage, disk-only)**
- Lines older than 10k (compressed, queried on-demand)

**Schema:**

```sql
CREATE TABLE scrollback (
    -- Composite primary key
    session_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    
    -- Line content (zstd compressed if data_compressed = true)
    data BLOB NOT NULL,           -- Raw bytes (UTF-8 or zstd-compressed UTF-8)
    data_compressed BOOLEAN NOT NULL DEFAULT 0,
    
    -- Metadata
    timestamp_ms INTEGER NOT NULL,  -- Unix timestamp (milliseconds)
    sequence_number INTEGER NOT NULL,  -- Matches Envelope.sequence_number
    
    PRIMARY KEY (session_id, line_number)
);

-- Index for scrollback pagination
CREATE INDEX idx_scrollback_sequence ON scrollback(session_id, sequence_number);
```

**Compression strategy (zstd):**

```rust
use zstd::stream::encode_all;

pub fn store_scrollback_line(
    conn: &Connection,
    session_id: &str,
    line_number: u64,
    data: &[u8],
    sequence_number: u64,
) -> Result<()> {
    let (blob, compressed) = if data.len() > 512 {
        // Compress if >512 bytes (typical line is 80-200 bytes)
        (encode_all(data, 1)?, true)  // zstd level 1
    } else {
        (data.to_vec(), false)
    };
    
    conn.execute(
        "INSERT INTO scrollback (session_id, line_number, data, data_compressed, timestamp_ms, sequence_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![session_id, line_number, blob, compressed, now_millis(), sequence_number],
    )?;
    
    Ok(())
}
```

**Compression ratio (expected):**
- **Logs with timestamps:** 60-70% reduction (4KB → 1.2-1.6KB)
- **Source code (build output):** 70-80% reduction (repetitive compiler output)
- **Plain text:** 50-60% reduction (less repetition)

**Scrollback fetch (pagination):**

```rust
pub fn fetch_scrollback_range(
    conn: &Connection,
    session_id: &str,
    start_line: u64,
    limit: usize,
) -> Result<Vec<Line>> {
    let mut stmt = conn.prepare(
        "SELECT line_number, data, data_compressed, timestamp_ms
         FROM scrollback
         WHERE session_id = ?1 AND line_number >= ?2
         ORDER BY line_number ASC
         LIMIT ?3"
    )?;
    
    let lines = stmt.query_map(params![session_id, start_line, limit], |row| {
        let data_compressed: bool = row.get(2)?;
        let blob: Vec<u8> = row.get(1)?;
        
        let data = if data_compressed {
            zstd::stream::decode_all(&blob[..])?
        } else {
            blob
        };
        
        Ok(Line {
            line_number: row.get(0)?,
            data: String::from_utf8(data)?,
            timestamp_ms: row.get(3)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;
    
    Ok(lines)
}
```

**Performance target (SRS §7.2):**
- Fetch 1000 lines: <100ms p95 (including zstd decompression)
- Write 1 line: <5ms p95 (append-only, no compression on write path)

---

### 2.3 Configuration Table

**User preferences (keybindings, themes, settings):**

```sql
CREATE TABLE configuration (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,      -- JSON-encoded value
    updated_at TEXT NOT NULL
);

-- Example entries
INSERT INTO configuration (key, value, updated_at)
VALUES 
    ('theme', '{"name": "dracula", "colors": {...}}', datetime('now')),
    ('keybindings', '{"copy": "Ctrl+Shift+C", "paste": "Ctrl+Shift+V"}', datetime('now')),
    ('default_shell', '"C:\\Windows\\System32\\powershell.exe"', datetime('now'));
```

**Why flat key-value instead of structured columns:**
- ✅ **Schema-less:** Add new config keys without migrations
- ✅ **Simple API:** `get_config(key)`, `set_config(key, value)` (no complex queries)
- ❌ **Trade-off:** Can't query inside JSON (but config is rarely queried, mostly loaded at startup)

---

### 2.4 Audit Logs Table

**Compliance requirement (SRS §5):**

```sql
CREATE TABLE audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    
    -- Event details
    event_type TEXT NOT NULL,     -- 'SESSION_CREATE' | 'SESSION_ATTACH' | 'INPUT' | 'SESSION_TERMINATE'
    session_id TEXT NOT NULL,
    user_id TEXT,                 -- From JWT 'sub' claim (nullable for local-only Phase 1)
    
    -- Context
    client_id TEXT,               -- From ClientInfo (Phase 2)
    ip_address TEXT,              -- Client IP (for network audit)
    
    -- Payload (optional, depends on event_type)
    payload TEXT                  -- JSON-encoded event-specific data
);

-- Index for audit queries
CREATE INDEX idx_audit_timestamp ON audit_logs(timestamp);
CREATE INDEX idx_audit_session ON audit_logs(session_id);
CREATE INDEX idx_audit_user ON audit_logs(user_id);
```

**Example audit events:**

```rust
pub enum AuditEvent {
    SessionCreate { session_id: String, shell_path: String },
    SessionAttach { session_id: String, client_id: String },
    Input { session_id: String, data_length: usize },  // NOT raw input (privacy)
    SessionTerminate { session_id: String, exit_code: i32 },
}

pub fn log_audit_event(conn: &Connection, event: AuditEvent, user_id: Option<&str>) -> Result<()> {
    let (event_type, session_id, payload) = match event {
        AuditEvent::SessionCreate { session_id, shell_path } => {
            ("SESSION_CREATE", session_id, json!({"shell_path": shell_path}))
        }
        AuditEvent::SessionAttach { session_id, client_id } => {
            ("SESSION_ATTACH", session_id, json!({"client_id": client_id}))
        }
        // ... other events
    };
    
    conn.execute(
        "INSERT INTO audit_logs (event_type, session_id, user_id, payload)
         VALUES (?1, ?2, ?3, ?4)",
        params![event_type, session_id, user_id, payload.to_string()],
    )?;
    
    Ok(())
}
```

**Privacy-preserving:**
- ✅ **Log input length, NOT raw input** (prevents password leaks in audit logs)
- ✅ **Timestamp + event type sufficient** for compliance (who accessed what, when)

**Retention policy:**
- **Phase 2:** Keep all logs indefinitely (defer to Phase 4 cleanup policy)
- **Phase 4:** Configurable retention (90 days default, auto-delete older entries)

---

## 3. Database Lifecycle

### 3.1 Initialization (First Run)

```rust
use rusqlite::Connection;

pub fn init_database(db_path: &Path) -> Result<Connection> {
    // Create parent directory if missing
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Open database (creates file if missing)
    let conn = Connection::open(db_path)?;
    
    // Enable WAL mode
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;"
    )?;
    
    // Create schema if not exists
    create_schema_if_missing(&conn)?;
    
    // Apply pending migrations
    apply_pending_migrations(&conn)?;
    
    Ok(conn)
}

fn create_schema_if_missing(conn: &Connection) -> Result<()> {
    // Check if schema_migrations table exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
        [],
        |row| row.get(0),
    )?;
    
    if !table_exists {
        // Fresh database: create all tables
        conn.execute_batch(include_str!("../../migrations/001_initial_schema.sql"))?;
    }
    
    Ok(())
}
```

---

### 3.2 Connection Pooling

**Multi-threaded access (Phase 2 requirement: 100 concurrent sessions):**

```rust
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let manager = SqliteConnectionManager::file(db_path)
            .with_init(|conn| {
                conn.execute_batch(
                    "PRAGMA journal_mode = WAL;
                     PRAGMA synchronous = NORMAL;
                     PRAGMA foreign_keys = ON;"
                )?;
                Ok(())
            });
        
        let pool = Pool::builder()
            .max_size(20)  // Max 20 concurrent connections
            .build(manager)?;
        
        Ok(Database { pool })
    }
    
    pub fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        Ok(self.pool.get()?)
    }
}
```

**Pool sizing:**
- **20 connections max:** SQLite WAL mode supports multiple readers + 1 writer
- **Bounded queue:** Tokio tasks wait for connection availability (backpressure)

---

### 3.3 Backup Strategy

**Automatic backups (Phase 2):**

```rust
use rusqlite::backup::Backup;

pub fn backup_database(src_path: &Path, dest_path: &Path) -> Result<()> {
    let src_conn = Connection::open(src_path)?;
    let mut dest_conn = Connection::open(dest_path)?;
    
    // SQLite online backup (doesn't lock database)
    let backup = Backup::new(&src_conn, &mut dest_conn)?;
    backup.run_to_completion(5, Duration::from_millis(250), None)?;
    
    Ok(())
}

// Schedule daily backup (via tokio interval)
pub async fn schedule_backups(db: Database, backup_dir: PathBuf) {
    let mut interval = tokio::time::interval(Duration::from_secs(86400)); // 24 hours
    
    loop {
        interval.tick().await;
        
        let backup_path = backup_dir.join(format!("monoterminal-{}.db", chrono::Utc::now().format("%Y%m%d")));
        
        if let Err(e) = backup_database(&db.path, &backup_path) {
            tracing::error!("Backup failed: {}", e);
        } else {
            tracing::info!("Backup created: {:?}", backup_path);
        }
        
        // Keep last 7 backups, delete older
        cleanup_old_backups(&backup_dir, 7)?;
    }
}
```

**Backup retention:**
- **Daily backups:** Keep last 7 days
- **Manual export:** User can trigger `monoterminal db export --file backup.db`

---

## 4. Performance Optimization

### 4.1 Write Batching

**Problem:** Writing 1 line at a time = 1 transaction per line (slow for high-throughput sessions)

**Solution:** Batch writes per session

```rust
use tokio::sync::mpsc;

pub struct ScrollbackWriter {
    tx: mpsc::Sender<ScrollbackLine>,
}

impl ScrollbackWriter {
    pub fn new(db: Database) -> Self {
        let (tx, mut rx) = mpsc::channel(1000);
        
        // Background writer task
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            
            loop {
                tokio::select! {
                    Some(line) = rx.recv() => {
                        batch.push(line);
                        
                        // Flush if batch full
                        if batch.len() >= 100 {
                            flush_batch(&db, &batch).await;
                            batch.clear();
                        }
                    }
                    _ = interval.tick() => {
                        // Flush every 100ms (even if batch not full)
                        if !batch.is_empty() {
                            flush_batch(&db, &batch).await;
                            batch.clear();
                        }
                    }
                }
            }
        });
        
        ScrollbackWriter { tx }
    }
    
    pub async fn write(&self, line: ScrollbackLine) -> Result<()> {
        self.tx.send(line).await?;
        Ok(())
    }
}

async fn flush_batch(db: &Database, batch: &[ScrollbackLine]) -> Result<()> {
    let conn = db.get_conn()?;
    
    let tx = conn.transaction()?;
    for line in batch {
        tx.execute(
            "INSERT INTO scrollback (...) VALUES (...)",
            params![...],
        )?;
    }
    tx.commit()?;
    
    Ok(())
}
```

**Batching parameters:**
- **Batch size:** 100 lines (balance latency vs throughput)
- **Flush interval:** 100ms (ensure writes hit disk within 100ms)

**Performance impact:**
- Without batching: ~500 writes/sec (1 transaction per write)
- With batching: ~10,000 writes/sec (100 lines per transaction)

---

### 4.2 Hot vs Cold Storage Tiering

**Problem:** Loading 100k lines into RAM = gigabytes of memory

**Solution:** Keep only recent 10k lines in memory (hot tier), query disk for older lines (cold tier)

```rust
pub struct SessionScrollback {
    session_id: String,
    hot_buffer: RingBuffer<Line>,  // Last 10k lines (in RAM)
    db: Database,
}

impl SessionScrollback {
    pub async fn fetch_range(&self, start_line: u64, limit: usize) -> Result<Vec<Line>> {
        let current_line = self.hot_buffer.latest_line_number();
        
        if start_line + limit as u64 > current_line - 10_000 {
            // Range in hot buffer: serve from RAM
            Ok(self.hot_buffer.get_range(start_line, limit))
        } else {
            // Range in cold storage: query SQLite
            let conn = self.db.get_conn()?;
            fetch_scrollback_range(&conn, &self.session_id, start_line, limit)
        }
    }
}
```

**Memory usage (100 sessions × 10k lines × 200 bytes/line):**
- Hot tier: ~200 MB (acceptable for 100 sessions)
- Cold tier: Disk only (queries on-demand)

---

## 5. Migration Strategy

### 5.1 Backward Compatibility

**Goal:** Phase 2 daemon must read Phase 1 in-memory state (no DB) if upgrading

**Upgrade path:**

```rust
pub fn migrate_from_phase1(sessions: Vec<Session>, db: &Database) -> Result<()> {
    let conn = db.get_conn()?;
    let tx = conn.transaction()?;
    
    for session in sessions {
        // Insert session metadata
        tx.execute(
            "INSERT INTO sessions (session_id, created_at, last_accessed_at, status, shell_path, working_dir, rows, cols)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id,
                session.created_at.to_rfc3339(),
                session.last_accessed_at.to_rfc3339(),
                "RUNNING",
                session.shell_path,
                session.working_dir,
                session.rows,
                session.cols,
            ],
        )?;
        
        // Insert scrollback (hot buffer only, don't persist entire history)
        for (line_num, line) in session.scrollback.iter().enumerate() {
            store_scrollback_line(&tx, &session.id, line_num as u64, line.data.as_bytes(), line.sequence)?;
        }
    }
    
    tx.commit()?;
    
    tracing::info!("Migrated {} sessions from Phase 1 to Phase 2 database", sessions.len());
    Ok(())
}
```

**Phase 1 → Phase 2 upgrade:**
1. Daemon shuts down (Phase 1 in-memory state still in RAM)
2. Phase 2 binary starts, detects missing DB
3. Prompts user: "Migrate in-memory sessions to persistent storage? (Y/n)"
4. If yes: Writes RAM state to SQLite, preserves sessions
5. If no: Starts fresh (loses sessions, acceptable for Phase 2 beta)

---

### 5.2 Schema Evolution (Phase 2 → Phase 3+)

**Additive-only changes:**

```sql
-- Phase 3: Add session tags
ALTER TABLE sessions ADD COLUMN tags TEXT;  -- JSON array
UPDATE sessions SET tags = '[]' WHERE tags IS NULL;

-- Phase 3: Add compression ratio tracking
ALTER TABLE scrollback ADD COLUMN compression_ratio REAL DEFAULT 1.0;
```

**Non-breaking:** Old daemons ignore new columns (SQLite default: NULL)

**Breaking changes (require migration script):**
- Rare (only if restructuring core schema)
- Example: Split `sessions` table into `sessions` + `session_metadata` (normalization)
- Requires: `migrations/NNN_break_session_table.sql` with explicit data copy

---

## 6. Testing Strategy

### 6.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_session_persist_and_restore() {
        let db = Database::new_temp()?;
        
        // Create session
        let session = Session::new("cmd.exe", "C:\\Users\\Alice");
        persist_session(&db, &session)?;
        
        // Restart daemon (simulate)
        drop(session);
        
        // Restore session
        let restored = load_session(&db, &session.id)?;
        assert_eq!(restored.shell_path, "cmd.exe");
    }
    
    #[tokio::test]
    async fn test_scrollback_compression() {
        let db = Database::new_temp()?;
        
        // Insert 1000 lines of scrollback
        let data = "INFO: Build succeeded\n".repeat(100);
        for i in 0..1000 {
            store_scrollback_line(&db, "session-1", i, data.as_bytes(), i)?;
        }
        
        // Verify compression ratio
        let stats = get_compression_stats(&db, "session-1")?;
        assert!(stats.compression_ratio < 0.5);  // >50% reduction
    }
}
```

---

### 6.2 Performance Benchmarks

```rust
#[bench]
fn bench_scrollback_write(b: &mut Bencher) {
    let db = Database::new_temp().unwrap();
    let line = "INFO: Processing file.txt\n";
    
    b.iter(|| {
        for i in 0..100 {
            store_scrollback_line(&db, "session-1", i, line.as_bytes(), i).unwrap();
        }
    });
}

#[bench]
fn bench_scrollback_read(b: &mut Bencher) {
    let db = setup_db_with_10k_lines();
    
    b.iter(|| {
        fetch_scrollback_range(&db, "session-1", 0, 1000).unwrap();
    });
}
```

**Performance targets (SRS §7.2):**
- Write 100 lines: <50ms p95 (batched)
- Read 1000 lines: <100ms p95 (including decompression)
- DB size: <1GB for 100 sessions × 100k lines (with compression)

---

## Consequences

### Positive
- ✅ Sessions survive daemon restarts (no lost work)
- ✅ Unlimited scrollback history (gigabytes of logs, queryable)
- ✅ zstd compression reduces disk usage by 60-80%
- ✅ Audit logs for compliance (who accessed what, when)

### Negative
- ⚠️ SQLite introduces disk I/O latency (vs Phase 1 in-memory)
- ⚠️ Schema migrations add complexity (must test upgrade paths)
- ⚠️ Backup strategy required (auto-backup adds background task)

### Neutral
- WAL mode requires 3 files: `monoterminal.db`, `monoterminal.db-wal`, `monoterminal.db-shm`
- Connection pooling adds state management (r2d2 Pool)

---

## References

- **ADR-005:** Daemon Lifecycle (dual-mode paths, %ProgramData% vs %LOCALAPPDATA%)
- **SRS §4.1:** SQLite persistence
- **SRS §7.2:** Phase 2 acceptance (100 concurrent sessions)
- **rusqlite:** https://docs.rs/rusqlite
- **r2d2-sqlite:** https://docs.rs/r2d2-sqlite
- **zstd:** https://docs.rs/zstd

---

## Follow-up Actions

1. ⏳ **Pending Phase 1 gate passage** (Friday 5/7 threshold)
2. ⏳ **Approve ADR-012** (eng-director, rust-engineer-storage review)
3. ⏳ **Implement schema** (rust-engineer-storage, Week 1-2)
4. ⏳ **Benchmark write/read performance** (criterion.rs, Week 3)
5. ⏳ **Fuzz test schema migrations** (cargo-fuzz, Week 4)

---

**Next:** ADR-013 (Multi-Session Architecture)
