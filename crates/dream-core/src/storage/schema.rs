//! Схема SQLite (`launcher.db`). Хранит только метаданные — версия,
//! загрузчик, RAM, пути; секреты аккаунтов живут в `auth::vault`
//! (Windows Credential Manager), а не здесь.

use crate::error::{CoreError, Result};
use rusqlite::Connection;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id            TEXT PRIMARY KEY,   -- uuid профиля Minecraft
    username      TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('microsoft', 'offline')),
    is_active     INTEGER NOT NULL DEFAULT 0,
    expires_at    TEXT,               -- ISO-8601, только для microsoft
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS instances (
    id                TEXT PRIMARY KEY,   -- uuid
    slug              TEXT NOT NULL UNIQUE, -- имя каталога на диске
    name              TEXT NOT NULL,
    mc_version        TEXT NOT NULL,
    loader            TEXT NOT NULL CHECK (loader IN ('vanilla', 'fabric', 'quilt', 'forge', 'neoforge')),
    loader_version    TEXT,
    min_ram_mb        INTEGER NOT NULL DEFAULT 512,
    max_ram_mb        INTEGER NOT NULL DEFAULT 4096,
    java_path         TEXT,               -- NULL = автоопределение
    extra_jvm_args    TEXT NOT NULL DEFAULT '',
    icon              TEXT,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_played_at    TEXT,
    active_shader_id  TEXT               -- активный шейдер-пак
);

CREATE TABLE IF NOT EXISTS installed_content (
    instance_id   TEXT NOT NULL REFERENCES instances(id) ON DELETE CASCADE,
    project_id    TEXT NOT NULL,   -- Modrinth project id
    version_id    TEXT NOT NULL,   -- Modrinth version id
    filename      TEXT NOT NULL,
    sha1          TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    content_type  TEXT NOT NULL DEFAULT 'mod' CHECK (content_type IN ('mod', 'shader', 'resourcepack')),
    PRIMARY KEY (instance_id, project_id)
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn open(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent.display().to_string(), e))?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_all_expected_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
        let names: Vec<String> = stmt.query_map([], |row| row.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();

        for expected in ["accounts", "installed_content", "instances", "settings"] {
            assert!(names.iter().any(|n| n == expected), "отсутствует таблица {expected}");
        }
    }

    #[test]
    fn foreign_key_cascade_removes_content_with_instance() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        conn.execute(
            "INSERT INTO instances (id, slug, name, mc_version, loader) VALUES ('i1', 'demo', 'Demo', '1.21.1', 'fabric')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO installed_content (instance_id, project_id, version_id, filename, sha1, content_type) VALUES ('i1', 'p1', 'v1', 'mod.jar', 'abc', 'mod')",
            [],
        )
        .unwrap();

        conn.execute("DELETE FROM instances WHERE id = 'i1'", []).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM installed_content", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn open_creates_parent_directories_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested").join("launcher.db");

        let conn1 = open(&db_path).unwrap();
        drop(conn1);
        let conn2 = open(&db_path).unwrap(); // повторный вызов не должен падать на "table already exists"
        drop(conn2);

        assert!(db_path.exists());
    }
}
