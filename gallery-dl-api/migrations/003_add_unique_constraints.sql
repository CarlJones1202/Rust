-- Add unique constraints to prevent duplicate media entries within the same gallery/request
-- This is crucial for idempotent resume support.

-- SQLite doesn't support ALTER TABLE ADD CONSTRAINT for UNIQUE, 
-- so we usually have to recreate the table, but since we are in dev and using IF NOT EXISTS 
-- for the initial tables, we can just create unique indices which act as constraints.

CREATE UNIQUE INDEX IF NOT EXISTS idx_images_gallery_hash ON images(gallery_id, hash);
CREATE UNIQUE INDEX IF NOT EXISTS idx_videos_request_hash ON videos(request_id, hash);
