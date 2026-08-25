-- Persistence Layer Schema
-- Phase 4: Splits/Tabs Layout Persistence (ADR-018, task-74 Day 4)

-- Layout persistence table (one layout per user)
CREATE TABLE IF NOT EXISTS layouts (
    user_id TEXT NOT NULL PRIMARY KEY,
    layout_data BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for timestamp queries (optional, for future analytics)
CREATE INDEX IF NOT EXISTS idx_layouts_updated_at ON layouts(updated_at);
