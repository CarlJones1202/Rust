-- Core person record
CREATE TABLE IF NOT EXISTS persons (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    disambiguation TEXT,
    gender TEXT,
    ethnicity TEXT,
    country TEXT,
    height INTEGER,
    hair_color TEXT,
    eye_color TEXT,
    measurements TEXT,
    breast_type TEXT,
    career_start_year INTEGER,
    career_end_year INTEGER,
    bio TEXT,
    extra_data TEXT,
    stashdb_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Aliases (many-per-person)
CREATE TABLE IF NOT EXISTS person_aliases (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE CASCADE
);

-- Profile images (many-per-person, stored locally)
CREATE TABLE IF NOT EXISTS person_images (
    id TEXT PRIMARY KEY NOT NULL,
    person_id TEXT NOT NULL,
    hash TEXT NOT NULL,
    extension TEXT NOT NULL,
    width INTEGER,
    height INTEGER,
    is_primary INTEGER NOT NULL DEFAULT 0,
    source_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE CASCADE
);

-- Gallery <-> Person join table (many-to-many)
CREATE TABLE IF NOT EXISTS gallery_persons (
    gallery_id TEXT NOT NULL,
    person_id TEXT NOT NULL,
    PRIMARY KEY (gallery_id, person_id),
    FOREIGN KEY (gallery_id) REFERENCES galleries(id) ON DELETE CASCADE,
    FOREIGN KEY (person_id) REFERENCES persons(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_person_aliases_person ON person_aliases(person_id);
CREATE INDEX IF NOT EXISTS idx_person_images_person ON person_images(person_id);
CREATE INDEX IF NOT EXISTS idx_gallery_persons_gallery ON gallery_persons(gallery_id);
CREATE INDEX IF NOT EXISTS idx_gallery_persons_person ON gallery_persons(person_id);
CREATE INDEX IF NOT EXISTS idx_persons_stashdb ON persons(stashdb_id);
