use rusqlite::{Connection, Result, params};
use chrono::{Utc, Datelike};
use crate::models::NseDownload;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub struct NseSymbol {
    pub id: i64,
    pub symbol: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecentlyViewed {
    pub symbol: String,
    pub name: Option<String>,
}

pub fn save_nse_symbols(conn: &Connection, symbols: Vec<String>) -> Result<(usize, Vec<String>)> {
    let now = Utc::now().timestamp();
    let mut saved_count = 0;
    let mut errors = Vec::new();

    for symbol in symbols {
        let trimmed = symbol.trim().to_uppercase();

        // Validate symbol format (alphanumeric and underscore only)
        if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') || trimmed.is_empty() {
            errors.push(trimmed);
            continue;
        }

        match conn.execute(
            "INSERT INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?2)
             ON CONFLICT(symbol) DO UPDATE SET updated_at = ?2",
            params![trimmed, now],
        ) {
            Ok(_) => saved_count += 1,
            Err(_) => errors.push(trimmed),
        }
    }

    Ok((saved_count, errors))
}

pub fn save_nse_symbols_with_names(conn: &Connection, symbols: Vec<(String, String)>) -> Result<(usize, Vec<String>)> {
    let now = Utc::now().timestamp();
    let mut saved_count = 0;
    let mut errors = Vec::new();

    for (symbol, name) in symbols {
        let trimmed = symbol.trim().to_uppercase();
        let trimmed_name = name.trim().to_string();

        // Validate symbol format (alphanumeric and underscore only)
        if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') || trimmed.is_empty() {
            errors.push(trimmed);
            continue;
        }

        match conn.execute(
            "INSERT INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(symbol) DO UPDATE SET name = excluded.name, updated_at = ?3",
            params![trimmed, trimmed_name, now],
        ) {
            Ok(_) => saved_count += 1,
            Err(_) => errors.push(trimmed),
        }
    }

    Ok((saved_count, errors))
}

pub fn get_nse_symbols(conn: &Connection) -> Result<Vec<String>> {
    get_nse_symbols_paginated(conn, None, None)
}

pub fn get_nse_symbols_paginated(conn: &Connection, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<String>> {
    let mut query = "SELECT symbol FROM nse_symbols ORDER BY symbol".to_string();
    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit));
        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }
    }
    let mut stmt = conn.prepare(&query)?;
    let symbols = stmt.query_map([], |row| row.get(0))?.collect::<Result<Vec<String>>>()?;
    Ok(symbols)
}

pub fn search_nse_symbols(conn: &Connection, query: &str, limit: usize) -> Result<Vec<String>> {
    let sql = "SELECT symbol FROM nse_symbols WHERE symbol LIKE ? OR name LIKE ? ORDER BY symbol LIMIT ?";
    let pattern = format!("%{}%", query.to_uppercase());
    let mut stmt = conn.prepare(sql)?;
    let symbols = stmt.query_map(params![pattern, pattern, limit], |row| row.get(0))?.collect::<Result<Vec<String>>>()?;
    Ok(symbols)
}

pub fn get_downloaded_files_for_symbol(conn: &Connection, symbol: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM nse_downloads
         WHERE symbol = ?
         ORDER BY from_date"
    )?;
    let files = stmt.query_map([symbol], |row| row.get(0))?.collect::<Result<Vec<String>>>()?;
    Ok(files)
}

pub fn get_bhavcopy_files(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM nse_downloads
         WHERE symbol IS NULL AND status = 'completed'
         ORDER BY from_date"
    )?;
    let files = stmt.query_map([], |row| row.get(0))?.collect::<Result<Vec<String>>>()?;
    Ok(files)
}

pub fn get_symbols_with_downloads(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT symbol FROM nse_downloads
         WHERE symbol IS NOT NULL
         ORDER BY symbol"
    )?;
    let symbols = stmt.query_map([], |row| row.get(0))?.collect::<Result<Vec<String>>>()?;
    Ok(symbols)
}

pub fn validate_download_records(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    use crate::get_downloads_dir;

    // Get all download records
    let mut stmt = conn.prepare("SELECT id, file_path FROM nse_downloads")?;
    let records: Vec<(i64, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_>>()?;

    let mut existing_paths = std::collections::HashSet::new();

    for (id, file_path) in records {
        if std::path::Path::new(&file_path).exists() {
            existing_paths.insert(file_path);
        } else {
            // File missing, remove record
            conn.execute("DELETE FROM nse_downloads WHERE id = ?", [id])?;
        }
    }

    // Now scan for files not in DB
    let downloads_dir = get_downloads_dir();
    if downloads_dir.exists() {
        scan_dir_for_missing_records(conn, &downloads_dir, &existing_paths)?;
    }

    Ok(())
}

fn scan_dir_for_missing_records(conn: &Connection, dir: &std::path::Path, existing_paths: &std::collections::HashSet<String>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_dir_for_missing_records(conn, &path, existing_paths)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("csv") {
            let path_str = path.to_string_lossy().to_string();
            if !existing_paths.contains(&path_str) {
                // Parse filename and add record
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if let Some(parts) = file_name.strip_prefix("historical_").and_then(|s| s.strip_suffix(".csv")) {
                        let parts: Vec<&str> = parts.split('_').collect();
                        if parts.len() >= 3 {
                            let symbol = parts[0].to_string();
                            let from_str = format!("{}-{}-{}", &parts[1][0..2], &parts[1][2..4], &parts[1][4..]);
                            let to_str = if parts.len() > 2 {
                                format!("{}-{}-{}", &parts[2][0..2], &parts[2][2..4], &parts[2][4..])
                            } else {
                                from_str.clone()
                            };

                            let from_ts = chrono::NaiveDate::parse_from_str(&from_str, "%d-%m-%Y")
                                .map(|d| d.and_hms_opt(0,0,0).unwrap().and_utc().timestamp())
                                .unwrap_or(0);
                            let to_ts = chrono::NaiveDate::parse_from_str(&to_str, "%d-%m-%Y")
                                .map(|d| d.and_hms_opt(0,0,0).unwrap().and_utc().timestamp())
                                .unwrap_or(0);
                            let file_size = path.metadata().ok().map(|m| m.len() as i64);

                            println!("Adding missing record for file: {}", path_str);
                            conn.execute(
                                "INSERT INTO nse_downloads (download_type, symbol, from_date, to_date, file_path, file_size, status, downloaded_at)
                                 VALUES (?, ?, ?, ?, ?, ?, 'completed', ?)",
                                rusqlite::params![
                                    "historical",
                                    symbol,
                                    from_ts,
                                    to_ts,
                                    path_str,
                                    file_size,
                                    chrono::Utc::now().timestamp()
                                ],
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn record_recently_viewed(conn: &Connection, symbol: &str) -> Result<()> {
    let now = Utc::now().timestamp();

    // First, ensure the symbol exists in nse_symbols
    let symbol_id: i64 = conn.query_row(
        "SELECT id FROM nse_symbols WHERE symbol = ?1",
        params![symbol],
        |row| row.get(0),
    ).unwrap_or_else(|_| {
        // Insert if not exists
        conn.execute(
            "INSERT INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?2)",
            params![symbol, now],
        ).unwrap();
        conn.last_insert_rowid()
    });

    // Insert or update recently_viewed
    conn.execute(
        "INSERT INTO recently_viewed (symbol_id, viewed_at)
         VALUES (?1, ?2)
         ON CONFLICT(symbol_id) DO UPDATE SET viewed_at = excluded.viewed_at",
        params![symbol_id, now],
    )?;

    Ok(())
}

pub fn get_recently_viewed(conn: &Connection, limit: usize) -> Result<Vec<RecentlyViewed>> {
    let mut stmt = conn.prepare(
        "SELECT ns.symbol, ns.name
         FROM recently_viewed rv
         JOIN nse_symbols ns ON rv.symbol_id = ns.id
         ORDER BY rv.viewed_at DESC
         LIMIT ?1"
    )?;

    let items = stmt.query_map(params![limit], |row| {
        Ok(RecentlyViewed {
            symbol: row.get(0)?,
            name: row.get(1)?,
        })
    })?;

    items.collect()
}

// For demo purposes, populate some random recently viewed items
pub fn populate_demo_data(conn: &Connection) -> Result<()> {
    let now = Utc::now().timestamp();

    // Add some demo symbols
    let demo_symbols = vec![
        "RELIANCE", "TCS", "HDFCBANK", "INFY", "ICICIBANK",
        "HINDUNILVR", "ITC", "SBIN", "BHARTIARTL", "KOTAKBANK",
        "LT", "AXISBANK", "ASIANPAINT", "MARUTI", "TITAN",
        "SUNPHARMA", "BAJFINANCE", "HCLTECH", "WIPRO", "ULTRACEMCO"
    ];

    for symbol in &demo_symbols {
        conn.execute(
            "INSERT OR IGNORE INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?2)",
            params![symbol, now],
        )?;
    }

    // Add some to recently viewed
    for (i, symbol) in demo_symbols.iter().take(10).enumerate() {
        let symbol_id: i64 = conn.query_row(
            "SELECT id FROM nse_symbols WHERE symbol = ?1",
            params![symbol],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO recently_viewed (symbol_id, viewed_at)
             VALUES (?1, ?2)",
            params![symbol_id, now - (i as i64 * 3600)],
        )?;
    }

    Ok(())
}

pub fn save_nse_download(conn: &Connection, download: &NseDownload) -> Result<i64> {
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO nse_downloads
         (symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(symbol, from_date, to_date)
         DO UPDATE SET file_path = excluded.file_path, file_size = excluded.file_size,
                      status = excluded.status, error_message = excluded.error_message,
                      downloaded_at = excluded.downloaded_at",
        params![
            download.symbol,
            download.from_date,
            download.to_date,
            download.file_path,
            download.file_size,
            download.status,
            download.error_message,
            now
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn get_nse_downloads(conn: &Connection, limit: usize) -> Result<Vec<NseDownload>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at
         FROM nse_downloads
         ORDER BY downloaded_at DESC
         LIMIT ?1"
    )?;

    let items = stmt.query_map(params![limit], |row| {
        Ok(NseDownload {
            id: row.get(0)?,
            symbol: row.get(1)?,
            from_date: row.get(2)?,
            to_date: row.get(3)?,
            file_path: row.get(4)?,
            file_size: row.get(5)?,
            status: row.get(6)?,
            error_message: row.get(7)?,
            downloaded_at: row.get(8)?,
        })
    })?;

    items.collect()
}

pub fn get_downloads_directory() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", "Indistocks")
        .expect("Unable to determine config directory");
    let config_dir = proj_dirs.config_dir();
    let downloads_dir = config_dir.join("downloads");
    fs::create_dir_all(&downloads_dir).expect("Unable to create downloads directory");
    downloads_dir
}

pub fn get_date_directory_path(date: chrono::NaiveDate) -> PathBuf {
    let downloads_dir = get_downloads_directory();
    let year_dir = downloads_dir.join(date.year().to_string());
    let month_dir = year_dir.join(format!("{:02}", date.month()));
    fs::create_dir_all(&month_dir).expect("Unable to create date directory");
    month_dir
}

#[derive(Debug, Clone)]
pub struct StockData {
    pub symbol: String,
    pub name: Option<String>,
    pub ltp: f64,
    pub change_percent: f64,
    pub volume: i64,
    pub range_low: f64,
    pub range_high: f64,
}

/// Get stock price data for a specific symbol within a date range
/// Returns Vec of (date, close_price) tuples ordered by date
pub fn get_stock_data_in_range(
    conn: &Connection,
    symbol: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate
) -> Result<Vec<(chrono::NaiveDate, f64)>> {
    let start_ts = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
    let end_ts = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp();

    let mut stmt = conn.prepare(
        "SELECT date, close FROM bhavcopy_data
         WHERE symbol = ? AND series = 'EQ'
         AND date >= ? AND date <= ?
         ORDER BY date"
    )?;

    let rows = stmt.query_map(params![symbol, start_ts, end_ts], |row| {
        let ts: i64 = row.get(0)?;
        let date = chrono::DateTime::from_timestamp(ts, 0)
            .unwrap()
            .naive_utc()
            .date();
        let close: f64 = row.get(1)?;
        Ok((date, close))
    })?;

    rows.collect()
}

pub fn get_all_stocks_with_metrics(conn: &Connection, price_from: Option<f64>, price_to: Option<f64>, range_days: i64) -> Result<Vec<StockData>> {
    // Get the latest date we have data for
    let latest_date: Option<i64> = conn.query_row(
        "SELECT MAX(date) FROM bhavcopy_data WHERE series = 'EQ'",
        [],
        |row| row.get(0)
    ).ok();

    let latest_date = match latest_date {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    // Calculate range start based on days
    let range_start = latest_date - (range_days * 24 * 60 * 60);

    // Build query with optional price filters
    let mut query = String::from(
        "SELECT
            ns.symbol,
            ns.name,
            latest.close as ltp,
            latest.prev_close,
            CASE
                WHEN latest.prev_close > 0 THEN ((latest.close - latest.prev_close) / latest.prev_close * 100.0)
                ELSE 0
            END as change_percent,
            latest.volume,
            range_stats.range_low,
            range_stats.range_high
        FROM nse_symbols ns
        INNER JOIN (
            SELECT symbol, close, prev_close, volume
            FROM bhavcopy_data
            WHERE date = ? AND series = 'EQ'
        ) latest ON ns.symbol = latest.symbol
        INNER JOIN (
            SELECT
                symbol,
                MIN(low) as range_low,
                MAX(high) as range_high
            FROM bhavcopy_data
            WHERE date >= ? AND date <= ? AND series = 'EQ'
            GROUP BY symbol
        ) range_stats ON ns.symbol = range_stats.symbol
        WHERE 1=1"
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(latest_date),
        Box::new(range_start),
        Box::new(latest_date),
    ];

    if let Some(from) = price_from {
        query.push_str(" AND latest.close >= ?");
        params.push(Box::new(from));
    }

    if let Some(to) = price_to {
        query.push_str(" AND latest.close <= ?");
        params.push(Box::new(to));
    }

    query.push_str(" ORDER BY ns.symbol");

    let mut stmt = conn.prepare(&query)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let stocks = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(StockData {
            symbol: row.get(0)?,
            name: row.get(1)?,
            ltp: row.get(2)?,
            change_percent: row.get(4)?,
            volume: row.get(5)?,
            range_low: row.get(6)?,
            range_high: row.get(7)?,
        })
    })?;

    stocks.collect()
}


