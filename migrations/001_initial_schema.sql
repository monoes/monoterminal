-- Migration 001: Initial Phase 2 schema
-- ADR-012: Persistence Layer Design
-- Date: 2026-08-17

BEGIN;

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL,  -- ISO 8601 timestamp
    description TEXT NOT NULL
);

-- Sessions table (§2.1)
CREATE TABLE IF NOT EXISTS sessions (
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
    acl TEXT,                     -- JSON-encoded ACL: {"user@example.com": "editor", "bob@example.com": "viewer"}
    metadata TEXT                 -- JSON object: {"name": "build-server", "tags": ["work", "linux"]}
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_sessions_owner ON sessions(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_last_accessed ON sessions(last_accessed_at);

-- Scrollback storage (§2.2)
CREATE TABLE IF NOT EXISTS scrollback (
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
CREATE INDEX IF NOT EXISTS idx_scrollback_sequence ON scrollback(session_id, sequence_number);

-- Configuration table (§2.3)
CREATE TABLE IF NOT EXISTS configuration (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,      -- JSON-encoded value
    updated_at TEXT NOT NULL
);

-- Audit logs table (§2.4)
CREATE TABLE IF NOT EXISTS audit_logs (
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

-- Indexes for audit queries
CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_session ON audit_logs(session_id);
CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_logs(user_id);

-- Record this migration
INSERT INTO schema_migrations (version, applied_at, description)
VALUES (1, datetime('now'), 'Initial Phase 2 schema');

COMMIT;
