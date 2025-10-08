use rusqlite::{Connection, Result};
use std::path::PathBuf;
use directories::ProjectDirs;

pub fn get_db_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", "Indistocks")
        .expect("Unable to determine config directory");
    let config_dir = proj_dirs.config_dir();
    std::fs::create_dir_all(config_dir).expect("Unable to create config directory");
    config_dir.join("db.sqlite3")
}

pub fn get_logs_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", "Indistocks")
        .expect("Unable to determine config directory");
    let config_dir = proj_dirs.config_dir();
    let logs_dir = config_dir.join("logs");
    std::fs::create_dir_all(&logs_dir).expect("Unable to create logs directory");
    logs_dir
}

pub fn init_db() -> Result<Connection> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;

    // Enable foreign key constraints
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Create tables
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS nse_symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL UNIQUE,
            name TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS bse_symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL UNIQUE,
            name TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS recently_viewed (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol_id INTEGER NOT NULL,
            viewed_at INTEGER NOT NULL,
            FOREIGN KEY (symbol_id) REFERENCES nse_symbols(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_recently_viewed_time
        ON recently_viewed(viewed_at DESC);
        "
    )?;

    Ok(conn)
}
