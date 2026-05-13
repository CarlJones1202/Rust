CREATE TABLE IF NOT EXISTS requests (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS galleries (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY NOT NULL,
    gallery_id TEXT NOT NULL,
    hash TEXT NOT NULL,
    extension TEXT NOT NULL,
    original_filename TEXT,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (gallery_id) REFERENCES galleries(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS videos (
    id TEXT PRIMARY KEY NOT NULL,
    request_id TEXT NOT NULL,
    hash TEXT NOT NULL,
    extension TEXT NOT NULL,
    original_filename TEXT,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_galleries_request_id ON galleries(request_id);
CREATE INDEX IF NOT EXISTS idx_images_gallery_id ON images(gallery_id);
CREATE INDEX IF NOT EXISTS idx_images_hash ON images(hash);
CREATE INDEX IF NOT EXISTS idx_videos_request_id ON videos(request_id);
CREATE INDEX IF NOT EXISTS idx_videos_hash ON videos(hash);
