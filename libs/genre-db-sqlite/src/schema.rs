/// DDL statements for the genre database schema.

pub const CREATE_TABLES: &str = "
CREATE TABLE IF NOT EXISTS genres (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    name_lower  TEXT NOT NULL UNIQUE,
    source      TEXT NOT NULL,
    parent_id   INTEGER REFERENCES genres(id),
    description TEXT,
    path        TEXT
);
CREATE INDEX IF NOT EXISTS idx_genres_lower ON genres(name_lower);

CREATE TABLE IF NOT EXISTS genre_aliases (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    alias       TEXT NOT NULL,
    genre_id    INTEGER NOT NULL REFERENCES genres(id),
    alias_type  TEXT NOT NULL DEFAULT 'variant'
);
CREATE INDEX IF NOT EXISTS idx_genre_aliases_alias ON genre_aliases(alias);

CREATE TABLE IF NOT EXISTS descriptors (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL,
    name_lower       TEXT NOT NULL UNIQUE,
    category         TEXT NOT NULL,
    descriptor_type  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_descriptors_lower ON descriptors(name_lower);
";
