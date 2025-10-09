use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use chrono::{Utc, NaiveDate, Datelike};
use reqwest::blocking::Client;
use std::time::Duration;
use std::thread;
use zip;
use csv;





#[derive(Debug)]
pub struct DownloadRecord {
    pub id: i64,
    pub symbol: Option<String>,
    pub from_date: i64,
    pub to_date: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub downloaded_at: i64,
}

pub fn get_downloads_dir() -> PathBuf {
    let proj_dirs = ProjectDirs::from("", "", "Indistocks")
        .expect("Unable to determine config directory");
    let downloads_dir = proj_dirs.config_dir().join("downloads");
    fs::create_dir_all(&downloads_dir).expect("Unable to create downloads directory");
    downloads_dir
}

fn create_http_client() -> Client {
    Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
        .timeout(Duration::from_secs(15))
        .cookie_store(true)
        .gzip(true)
        .build()
        .expect("Failed to create HTTP client")
}

fn rate_limit_delay() {
    thread::sleep(Duration::from_millis(350)); // ~3 requests per second
}





pub fn download_historical_data(symbol: &str, from_date: NaiveDate, to_date: NaiveDate) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();
    let mut downloaded_files = Vec::new();

    // NSE API may have limits, chunk dates if needed, but try full range first
    let from_str = from_date.format("%d-%m-%Y").to_string();
    let to_str = to_date.format("%d-%m-%Y").to_string();

    let url = format!(
        "https://www.nseindia.com/api/historicalOR/generateSecurityWiseHistoricalData?from={}&to={}&symbol={}&type=priceVolumeDeliverable&series=ALL&csv=true",
        from_str, to_str, symbol
    );

    rate_limit_delay();

    let response = client
        .get(&url)
        .header("Referer", "https://www.nseindia.com/get-quotes/equity?symbol=HDFCBANK")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("DNT", "1")
        .header("Connection", "keep-alive")
        .header("Upgrade-Insecure-Requests", "1")
        .header("Sec-Fetch-Dest", "document")
        .header("Sec-Fetch-Mode", "navigate")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Cache-Control", "max-age=0")
        .send()?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for {}", response.status(), url).into());
    }

    // Check if response is CSV
    let content_type = response.headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("text/csv") && !content_type.contains("application/octet-stream") {
        return Err("Unexpected content type, expected CSV".into());
    }

    // Save the CSV directly
    let year_dir = downloads_dir.join(from_date.year().to_string());
    let month_dir = year_dir.join(format!("{:02}", from_date.month()));
    fs::create_dir_all(&month_dir)?;

    let file_name = format!("historical_{}_{}_{}.csv", symbol, from_str, to_str);
    let file_path = month_dir.join(&file_name);

    let bytes = response.bytes()?;
    let csv_content = String::from_utf8_lossy(&bytes);

    // Validate that it's CSV data
    let lines: Vec<&str> = csv_content.lines().collect();
    if lines.len() < 2 || !lines[0].contains("Date") {
        return Err("Response does not appear to be valid CSV data".into());
    }

    // Clean headers by trimming spaces
    let cleaned_content = csv_content.lines()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                // Clean header row
                line.split(',')
                    .map(|field| field.trim().trim_matches('"'))
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut file = fs::File::create(&file_path)?;
    std::io::copy(&mut cleaned_content.as_bytes(), &mut file)?;

    downloaded_files.push(file_name);

    Ok(downloaded_files)
}

pub fn save_download_record(conn: &Connection, symbol: Option<&str>, from_date: i64, to_date: i64, file_path: &str, status: &str, error_message: Option<&str>) -> Result<i64, Box<dyn std::error::Error>> {
    let now = Utc::now().timestamp();
    let file_size = fs::metadata(file_path).ok().map(|m| m.len() as i64);

    conn.execute(
        "INSERT INTO nse_downloads (symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            symbol,
            from_date,
            to_date,
            file_path,
            file_size,
            status,
            error_message,
            now
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

pub fn get_download_records(conn: &Connection) -> Result<Vec<DownloadRecord>, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at
         FROM nse_downloads ORDER BY downloaded_at DESC LIMIT 50"
    )?;

    let records = stmt.query_map([], |row| {
        Ok(DownloadRecord {
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
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(records)
}

pub fn download_bhavcopy(db_conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, tx: &std::sync::mpsc::Sender<crate::BhavCopyMessage>) -> Result<(), Box<dyn std::error::Error>> {
    download_bhavcopy_with_limit(db_conn, tx, None)
}

pub fn download_bhavcopy_with_date_range(db_conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, tx: &std::sync::mpsc::Sender<crate::BhavCopyMessage>, start_date: NaiveDate, end_date: NaiveDate, max_files: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();

    let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
        "Downloading BhavCopy data from {} to {}",
        end_date.format("%Y-%m-%d"),
        start_date.format("%Y-%m-%d")
    )));

    let mut current_date = start_date;
    let mut downloaded_count = 0;
    let mut consecutive_error_days = 0;
    let mut attempts = 0;
    let max_consecutive_error_days = 10; // Stop if we get 10 consecutive days of errors

    while current_date >= end_date {
        // Check if we've reached the download limit
        if let Some(limit) = max_files {
            if downloaded_count >= limit {
                println!("Reached download limit of {} files", limit);
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!("Reached download limit of {} files", limit)));
                break;
            }
        }

        // Stop if too many consecutive days with errors
        if consecutive_error_days >= max_consecutive_error_days {
            let msg = format!("Stopping after {} consecutive days with no data available", max_consecutive_error_days);
            println!("{}", msg);
            let _ = tx.send(crate::BhavCopyMessage::Progress(msg));
            break;
        }

        attempts += 1;
        let date_str = current_date.format("%Y%m%d").to_string();
        let year = current_date.year();
        let month = current_date.month();

        let url = format!("https://nsearchives.nseindia.com/content/cm/BhavCopy_NSE_CM_0_0_0_{}_F_0000.csv.zip", date_str);

        rate_limit_delay();

        println!("Downloading: {}", url);
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Downloading {} (attempt {}, {} downloaded, {} consecutive error days)",
            current_date.format("%Y-%m-%d"),
            attempts,
            downloaded_count,
            consecutive_error_days
        )));

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
            .header("Referer", "https://www.nseindia.com/get-quotes/equity?symbol=HDFCBANK")
            .send();

        let response = match response {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                println!("   HTTP error {}: {}", resp.status(), current_date.format("%Y-%m-%d"));
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
                    "   HTTP error {} for {}",
                    resp.status(),
                    current_date.format("%Y-%m-%d")
                )));
                consecutive_error_days += 1;
                current_date = current_date - chrono::Duration::days(1);
                continue;
            }
            Err(e) => {
                println!("   Network error: {} for {}", e, current_date.format("%Y-%m-%d"));
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
                    "   Network error: {} for {}",
                    e,
                    current_date.format("%Y-%m-%d")
                )));
                consecutive_error_days += 1;
                current_date = current_date - chrono::Duration::days(1);
                continue;
            }
        };

        // Create directory
        let year_dir = downloads_dir.join(year.to_string());
        let month_dir = year_dir.join(format!("{:02}", month));
        fs::create_dir_all(&month_dir)?;

        let zip_path = month_dir.join(format!("bhavcopy_{}.zip", date_str));
        let csv_path = month_dir.join(format!("bhavcopy_{}.csv", date_str));

        // Download ZIP
        let bytes = response.bytes()?;
        fs::write(&zip_path, &bytes)?;

        // Extract ZIP
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip_path)?)?;
        let mut file = archive.by_index(0)?;
        let mut csv_data = Vec::new();
        std::io::copy(&mut file, &mut csv_data)?;

        // Validate CSV
        let csv_str = String::from_utf8_lossy(&csv_data);
        let lines: Vec<&str> = csv_str.lines().collect();
        if lines.len() < 2 || !lines[0].contains("TradDt") {
            println!("   Invalid CSV for {}", current_date.format("%Y-%m-%d"));
            fs::remove_file(&zip_path)?;
            consecutive_error_days += 1;
            current_date = current_date - chrono::Duration::days(1);
            continue;
        }

        // Save CSV
        fs::write(&csv_path, &csv_data)?;
        fs::remove_file(&zip_path)?; // Remove ZIP after extraction

        // Record in DB
        let ts = current_date.and_hms_opt(0,0,0).unwrap().and_utc().timestamp();
        {
            let conn = db_conn.lock().unwrap();
            save_download_record(&*conn, None, ts, ts, &csv_path.to_string_lossy(), "completed", None)?;
        }

        // Parse CSV and insert into bhavcopy_data
        println!("Processing: {}", csv_path.display());
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Processing {} data into database...",
            current_date.format("%Y-%m-%d")
        )));
        {
            let conn = db_conn.lock().unwrap();
            let mut rdr = csv::ReaderBuilder::new()
                .flexible(true)
                .from_path(&csv_path)?;

            let headers = rdr.headers()?.clone();

            let symbol_idx = headers.iter().position(|h| h == "TckrSymb").unwrap_or(1);
            let series_idx = headers.iter().position(|h| h == "SctySrs").unwrap_or(2);
            let open_idx = headers.iter().position(|h| h == "OpnPric").unwrap_or(4);
            let high_idx = headers.iter().position(|h| h == "HghPric").unwrap_or(5);
            let low_idx = headers.iter().position(|h| h == "LwPric").unwrap_or(6);
            let close_idx = headers.iter().position(|h| h == "ClsPric").unwrap_or(7);
            let last_idx = headers.iter().position(|h| h == "LastPric").unwrap_or(8);
            let prev_close_idx = headers.iter().position(|h| h == "PrvsClsgPric").unwrap_or(9);
            let volume_idx = headers.iter().position(|h| h == "TtlTradgVol").unwrap_or(10);
            let turnover_idx = headers.iter().position(|h| h == "TtlTrfVal").unwrap_or(11);
            let trades_idx = headers.iter().position(|h| h == "TtlNbOfTxsExctd").unwrap_or(12);
            let isin_idx = headers.iter().position(|h| h == "ISIN").unwrap_or(13);

            let mut rows: Vec<(String, String, i64, f64, f64, f64, f64, f64, f64, i64, f64, i64, String)> = Vec::new();
            for result in rdr.records() {
                let record = result?;
                if record.len() <= symbol_idx { continue; }
                let symbol = record.get(symbol_idx).unwrap_or("").trim().to_uppercase();
                if symbol.is_empty() { continue; }
                let series = record.get(series_idx).unwrap_or("").trim().to_string();
                let open: f64 = record.get(open_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let high: f64 = record.get(high_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let low: f64 = record.get(low_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let close: f64 = record.get(close_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let last: f64 = record.get(last_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let prev_close: f64 = record.get(prev_close_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let volume: i64 = record.get(volume_idx).unwrap_or("0").trim().parse().unwrap_or(0);
                let turnover: f64 = record.get(turnover_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let trades: i64 = record.get(trades_idx).unwrap_or("0").trim().parse().unwrap_or(0);
                let isin = record.get(isin_idx).unwrap_or("").trim().to_string();
                rows.push((symbol, series, ts, open, high, low, close, last, prev_close, volume, turnover, trades, isin));
            }
            for chunk in rows.chunks(100) {
                if chunk.is_empty() { continue; }
                let placeholders: Vec<String> = chunk.iter().map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string()).collect();
                let query = format!("INSERT OR IGNORE INTO bhavcopy_data (symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin) VALUES {}", placeholders.join(", "));
                let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().flat_map(|(symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin)| vec![symbol as &dyn rusqlite::ToSql, series as &dyn rusqlite::ToSql, date as &dyn rusqlite::ToSql, open as &dyn rusqlite::ToSql, high as &dyn rusqlite::ToSql, low as &dyn rusqlite::ToSql, close as &dyn rusqlite::ToSql, last as &dyn rusqlite::ToSql, prev_close as &dyn rusqlite::ToSql, volume as &dyn rusqlite::ToSql, turnover as &dyn rusqlite::ToSql, trades as &dyn rusqlite::ToSql, isin as &dyn rusqlite::ToSql]).collect();
                conn.execute(&query, rusqlite::params_from_iter(params))?;
            }
        }

        println!("Finished: {}", csv_path.display());

        // Delete CSV file after processing
        fs::remove_file(&csv_path)?;

        // Success! Reset consecutive error day counter
        consecutive_error_days = 0;
        downloaded_count += 1;
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Completed {} ({} files processed)",
            current_date.format("%Y-%m-%d"),
            downloaded_count
        )));

        // Send updated date range
        {
            let conn = db_conn.lock().unwrap();
            if let Ok(Some((min_date, max_date))) = get_bhavcopy_date_range(&*conn) {
                let _ = tx.send(crate::BhavCopyMessage::DateRangeUpdated(min_date, max_date));
            }
        }

        current_date = current_date - chrono::Duration::days(1);
    }

    Ok(())
}

pub fn download_bhavcopy_with_limit(db_conn: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, tx: &std::sync::mpsc::Sender<crate::BhavCopyMessage>, max_files: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();

    // Get the earliest date in bhavcopy_data to download older data
    let earliest_data_date: Option<i64> = {
        let conn = db_conn.lock().unwrap();
        conn.query_row(
            "SELECT MIN(date) FROM bhavcopy_data",
            [],
            |row| row.get(0),
        ).unwrap_or(None)
    };

    let start_date = if let Some(ts) = earliest_data_date {
        // If we have data, start from the day before the earliest date
        chrono::DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.naive_utc().date() - chrono::Duration::days(1))
            .unwrap_or_else(|| chrono::Utc::now().date_naive() - chrono::Duration::days(1))
    } else {
        // No data yet, start from yesterday
        chrono::Utc::now().date_naive() - chrono::Duration::days(1)
    };

    let end_date = start_date - chrono::Duration::days(365); // 12 months back

    let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
        "Downloading BhavCopy data from {} to {}",
        end_date.format("%Y-%m-%d"),
        start_date.format("%Y-%m-%d")
    )));

    let mut current_date = start_date;
    let mut downloaded_count = 0;
    let mut consecutive_error_days = 0;
    let mut attempts = 0;
    let max_consecutive_error_days = 10; // Stop if we get 10 consecutive days of errors

    while current_date >= end_date {
        // Check if we've reached the download limit
        if let Some(limit) = max_files {
            if downloaded_count >= limit {
                println!("Reached download limit of {} files", limit);
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!("Reached download limit of {} files", limit)));
                break;
            }
        }

        // Stop if too many consecutive days with errors
        if consecutive_error_days >= max_consecutive_error_days {
            let msg = format!("Stopping after {} consecutive days with no data available", max_consecutive_error_days);
            println!("{}", msg);
            let _ = tx.send(crate::BhavCopyMessage::Progress(msg));
            break;
        }

        attempts += 1;
        let date_str = current_date.format("%Y%m%d").to_string();
        let year = current_date.year();
        let month = current_date.month();

        // NSE switched to new format for 2024 onwards
        // Old format URLs no longer work, even for dates before the switch
        let url = format!("https://nsearchives.nseindia.com/content/cm/BhavCopy_NSE_CM_0_0_0_{}_F_0000.csv.zip", date_str);

        rate_limit_delay();

        println!("Downloading: {}", url);
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Downloading {} (attempt {}, {} downloaded, {} consecutive error days)",
            current_date.format("%Y-%m-%d"),
            attempts,
            downloaded_count,
            consecutive_error_days
        )));

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
            .header("Referer", "https://www.nseindia.com/get-quotes/equity?symbol=HDFCBANK")
            .send();

        let response = match response {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                println!("   HTTP error {}: {}", resp.status(), current_date.format("%Y-%m-%d"));
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
                    "   HTTP error {} for {}",
                    resp.status(),
                    current_date.format("%Y-%m-%d")
                )));
                consecutive_error_days += 1;
                current_date = current_date - chrono::Duration::days(1);
                continue;
            }
            Err(e) => {
                println!("   Network error: {} for {}", e, current_date.format("%Y-%m-%d"));
                let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
                    "   Network error: {} for {}",
                    e,
                    current_date.format("%Y-%m-%d")
                )));
                consecutive_error_days += 1;
                current_date = current_date - chrono::Duration::days(1);
                continue;
            }
        };

        // Create directory
        let year_dir = downloads_dir.join(year.to_string());
        let month_dir = year_dir.join(format!("{:02}", month));
        fs::create_dir_all(&month_dir)?;

        let zip_path = month_dir.join(format!("bhavcopy_{}.zip", date_str));
        let csv_path = month_dir.join(format!("bhavcopy_{}.csv", date_str));

        // Download ZIP
        let bytes = response.bytes()?;
        fs::write(&zip_path, &bytes)?;

        // Extract ZIP
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip_path)?)?;
        let mut file = archive.by_index(0)?;
        let mut csv_data = Vec::new();
        std::io::copy(&mut file, &mut csv_data)?;

        // Validate CSV
        let csv_str = String::from_utf8_lossy(&csv_data);
        let lines: Vec<&str> = csv_str.lines().collect();
        if lines.len() < 2 || !lines[0].contains("TradDt") {
            println!("   Invalid CSV for {}", current_date.format("%Y-%m-%d"));
            fs::remove_file(&zip_path)?;
            consecutive_error_days += 1;
            current_date = current_date - chrono::Duration::days(1);
            continue;
        }

        // Save CSV
        fs::write(&csv_path, &csv_data)?;
        fs::remove_file(&zip_path)?; // Remove ZIP after extraction

        // Record in DB
        let ts = current_date.and_hms_opt(0,0,0).unwrap().and_utc().timestamp();
        {
            let conn = db_conn.lock().unwrap();
            save_download_record(&*conn, None, ts, ts, &csv_path.to_string_lossy(), "completed", None)?;
        }

        // Parse CSV and insert into bhavcopy_data
        println!("Processing: {}", csv_path.display());
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Processing {} data into database...",
            current_date.format("%Y-%m-%d")
        )));
        {
            let conn = db_conn.lock().unwrap();
            // Configure CSV reader to be flexible about field counts
            // Some NSE files (e.g., 2024-06-19, 2024-06-20) have trailing commas in headers
            let mut rdr = csv::ReaderBuilder::new()
                .flexible(true)
                .from_path(&csv_path)?;

            // Get headers to determine column mapping
            let headers = rdr.headers()?.clone();

            // Find column indices
            let symbol_idx = headers.iter().position(|h| h == "TckrSymb").unwrap_or(1);
            let series_idx = headers.iter().position(|h| h == "SctySrs").unwrap_or(2);
            let open_idx = headers.iter().position(|h| h == "OpnPric").unwrap_or(4);
            let high_idx = headers.iter().position(|h| h == "HghPric").unwrap_or(5);
            let low_idx = headers.iter().position(|h| h == "LwPric").unwrap_or(6);
            let close_idx = headers.iter().position(|h| h == "ClsPric").unwrap_or(7);
            let last_idx = headers.iter().position(|h| h == "LastPric").unwrap_or(8);
            let prev_close_idx = headers.iter().position(|h| h == "PrvsClsgPric").unwrap_or(9);
            let volume_idx = headers.iter().position(|h| h == "TtlTradgVol").unwrap_or(10);
            let turnover_idx = headers.iter().position(|h| h == "TtlTrfVal").unwrap_or(11);
            let trades_idx = headers.iter().position(|h| h == "TtlNbOfTxsExctd").unwrap_or(12);
            let isin_idx = headers.iter().position(|h| h == "ISIN").unwrap_or(13);

            let mut rows: Vec<(String, String, i64, f64, f64, f64, f64, f64, f64, i64, f64, i64, String)> = Vec::new();
            for result in rdr.records() {
                let record = result?;
                if record.len() <= symbol_idx { continue; }
                let symbol = record.get(symbol_idx).unwrap_or("").trim().to_uppercase();
                if symbol.is_empty() { continue; }
                let series = record.get(series_idx).unwrap_or("").trim().to_string();
                let open: f64 = record.get(open_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let high: f64 = record.get(high_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let low: f64 = record.get(low_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let close: f64 = record.get(close_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let last: f64 = record.get(last_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let prev_close: f64 = record.get(prev_close_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let volume: i64 = record.get(volume_idx).unwrap_or("0").trim().parse().unwrap_or(0);
                let turnover: f64 = record.get(turnover_idx).unwrap_or("0").trim().parse().unwrap_or(0.0);
                let trades: i64 = record.get(trades_idx).unwrap_or("0").trim().parse().unwrap_or(0);
                let isin = record.get(isin_idx).unwrap_or("").trim().to_string();
                rows.push((symbol, series, ts, open, high, low, close, last, prev_close, volume, turnover, trades, isin));
            }
            for chunk in rows.chunks(100) {
                if chunk.is_empty() { continue; }
                let placeholders: Vec<String> = chunk.iter().map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string()).collect();
                let query = format!("INSERT OR IGNORE INTO bhavcopy_data (symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin) VALUES {}", placeholders.join(", "));
                let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().flat_map(|(symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin)| vec![symbol as &dyn rusqlite::ToSql, series as &dyn rusqlite::ToSql, date as &dyn rusqlite::ToSql, open as &dyn rusqlite::ToSql, high as &dyn rusqlite::ToSql, low as &dyn rusqlite::ToSql, close as &dyn rusqlite::ToSql, last as &dyn rusqlite::ToSql, prev_close as &dyn rusqlite::ToSql, volume as &dyn rusqlite::ToSql, turnover as &dyn rusqlite::ToSql, trades as &dyn rusqlite::ToSql, isin as &dyn rusqlite::ToSql]).collect();
                conn.execute(&query, rusqlite::params_from_iter(params))?;
            }
        }

        println!("Finished: {}", csv_path.display());

        // Delete CSV file after processing
        fs::remove_file(&csv_path)?;

        // Success! Reset consecutive error day counter
        consecutive_error_days = 0;
        downloaded_count += 1;
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!(
            "Completed {} ({} files processed)",
            current_date.format("%Y-%m-%d"),
            downloaded_count
        )));

        // Send updated date range
        {
            let conn = db_conn.lock().unwrap();
            if let Ok(Some((min_date, max_date))) = get_bhavcopy_date_range(&*conn) {
                let _ = tx.send(crate::BhavCopyMessage::DateRangeUpdated(min_date, max_date));
            }
        }

        current_date = current_date - chrono::Duration::days(1);
    }

    Ok(())
}

pub fn get_bhavcopy_date_range(conn: &Connection) -> Result<Option<(chrono::NaiveDate, chrono::NaiveDate)>, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT MIN(date), MAX(date) FROM bhavcopy_data")?;
    let mut rows = stmt.query_map([], |row| {
        let min_ts: Option<i64> = row.get(0)?;
        let max_ts: Option<i64> = row.get(1)?;
        Ok((min_ts, max_ts))
    })?;

    if let Some(row) = rows.next() {
        let (min_ts, max_ts) = row?;
        if let (Some(min_ts), Some(max_ts)) = (min_ts, max_ts) {
            let min_date = chrono::DateTime::from_timestamp(min_ts, 0)
                .map(|dt| dt.naive_utc().date());
            let max_date = chrono::DateTime::from_timestamp(max_ts, 0)
                .map(|dt| dt.naive_utc().date());
            if let (Some(min), Some(max)) = (min_date, max_date) {
                return Ok(Some((min, max)));
            }
        }
    }
    Ok(None)
}

pub fn clear_bhavcopy_data(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute("DELETE FROM bhavcopy_data", [])?;
    conn.execute("DELETE FROM nse_downloads WHERE symbol IS NULL", [])?;
    Ok(())
}