-- Create video_progress table
CREATE TABLE IF NOT EXISTS video_progress (
    video_id TEXT PRIMARY KEY NOT NULL,
    position_seconds REAL NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE
);

-- Index for performance
CREATE INDEX IF NOT EXISTS idx_video_progress_updated_at ON video_progress(updated_at);

-- Add duration to videos table (this is handled in db.rs usually but kept here for completeness)
-- Note: SQLite doesn't have ADD COLUMN IF NOT EXISTS, so this might fail if already added.
-- We put it at the end so it doesn't block the CREATE TABLE above.
ALTER TABLE videos ADD COLUMN duration_seconds REAL;
