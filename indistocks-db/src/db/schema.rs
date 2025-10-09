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

        CREATE UNIQUE INDEX IF NOT EXISTS idx_recently_viewed_symbol_id
        ON recently_viewed(symbol_id);

        CREATE INDEX IF NOT EXISTS idx_recently_viewed_time
        ON recently_viewed(viewed_at DESC);

        CREATE TABLE IF NOT EXISTS nse_downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT,
            from_date INTEGER NOT NULL,
            to_date INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_size INTEGER,
            status TEXT NOT NULL,
            error_message TEXT,
            downloaded_at INTEGER NOT NULL,
            UNIQUE(symbol, from_date, to_date)
        );

        CREATE INDEX IF NOT EXISTS idx_nse_downloads_downloaded_at
        ON nse_downloads(downloaded_at DESC);

        CREATE TABLE IF NOT EXISTS bhavcopy_data (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL,
            series TEXT,
            date INTEGER NOT NULL,
            open REAL,
            high REAL,
            low REAL,
            close REAL,
            last REAL,
            prev_close REAL,
            volume INTEGER,
            turnover REAL,
            trades INTEGER,
            isin TEXT,
            UNIQUE(symbol, date)
        );

        CREATE INDEX IF NOT EXISTS idx_bhavcopy_data_symbol_date
        ON bhavcopy_data(symbol, date);
        "
    )?;

    Ok(conn)
}
