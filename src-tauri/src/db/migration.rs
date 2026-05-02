pub const SCHEMA_V1: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS share_links (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    share_code      TEXT    NOT NULL,
    receive_code    TEXT    NOT NULL DEFAULT '',
    title           TEXT    NOT NULL DEFAULT '',
    share_user_id   TEXT    NOT NULL DEFAULT '',
    share_user_name TEXT    NOT NULL DEFAULT '',
    total_file_count INTEGER NOT NULL DEFAULT 0,
    total_size      INTEGER NOT NULL DEFAULT 0,
    status          TEXT    NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    last_parsed_at  TEXT,
    added_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(share_code, receive_code)
);

CREATE TABLE IF NOT EXISTS files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    share_link_id   INTEGER NOT NULL,
    file_id         TEXT    NOT NULL DEFAULT '',
    parent_id       TEXT    NOT NULL DEFAULT '0',
    name            TEXT    NOT NULL,
    size            INTEGER NOT NULL DEFAULT 0,
    sha1            TEXT    NOT NULL DEFAULT '',
    is_dir          INTEGER NOT NULL DEFAULT 0,
    file_type       TEXT    NOT NULL DEFAULT 'other',
    full_path       TEXT    NOT NULL DEFAULT '',
    depth           INTEGER NOT NULL DEFAULT 0,
    thumbnail_url   TEXT    NOT NULL DEFAULT '',
    raw_json        TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (share_link_id) REFERENCES share_links(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_files_share_link_id ON files(share_link_id);
CREATE INDEX IF NOT EXISTS idx_files_file_type ON files(file_type);
CREATE INDEX IF NOT EXISTS idx_files_is_dir ON files(is_dir);
CREATE INDEX IF NOT EXISTS idx_files_size ON files(size);
CREATE INDEX IF NOT EXISTS idx_files_name ON files(name);
CREATE INDEX IF NOT EXISTS idx_files_file_id ON files(file_id);
CREATE INDEX IF NOT EXISTS idx_files_parent_id ON files(parent_id);
CREATE INDEX IF NOT EXISTS idx_files_sha1 ON files(sha1);
CREATE INDEX IF NOT EXISTS idx_share_links_status ON share_links(status);

CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    name,
    full_path,
    content='files',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, name, full_path) VALUES (new.id, new.name, new.full_path);
END;

CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, name, full_path) VALUES('delete', old.id, old.name, old.full_path);
END;

CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, name, full_path) VALUES('delete', old.id, old.name, old.full_path);
    INSERT INTO files_fts(rowid, name, full_path) VALUES (new.id, new.name, new.full_path);
END;

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('rate_limit_rps', '2'),
    ('page_size', '1150'),
    ('theme', 'light'),
    ('language', 'zh-CN');
"#;

pub fn run_migrations(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let current_version: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .ok();

    if current_version.is_none() {
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('schema_version', '1')",
            [],
        )?;
    }

    Ok(())
}
