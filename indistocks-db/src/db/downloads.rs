use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use chrono::{Utc, NaiveDate, Datelike};
use reqwest::blocking::Client;
use std::time::Duration;
use std::thread;
use zip;
use csv::Reader;





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
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();

    // Get last downloaded date for bhavcopy (symbol IS NULL)
    let last_date: Option<i64> = {
        let conn = db_conn.lock().unwrap();
        conn.query_row(
            "SELECT MAX(to_date) FROM nse_downloads WHERE symbol IS NULL AND status = 'completed'",
            [],
            |row| row.get(0),
        ).unwrap_or(None)
    };

    let start_date = if let Some(ts) = last_date {
        chrono::DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.naive_utc().date())
            .unwrap_or_else(|| chrono::Utc::now().date_naive())
    } else {
        chrono::Utc::now().date_naive() - chrono::Duration::days(1) // yesterday
    };

    let end_date = start_date - chrono::Duration::days(365); // 12 months back

    let mut current_date = start_date;

    while current_date >= end_date {
        let date_str = current_date.format("%Y%m%d").to_string();
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let url = if current_date < chrono::NaiveDate::from_ymd_opt(2024, 7, 8).unwrap() {
            // Old format
            let month_name = match month {
                1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR", 5 => "MAY", 6 => "JUN",
                7 => "JUL", 8 => "AUG", 9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
                _ => "JAN",
            };
            format!("https://nsearchives.nseindia.com/content/historical/EQUITIES/{}/{}/cm{:02}{}bhav.csv.zip",
                    year, month_name, day, month_name)
        } else {
            // New format
            format!("https://nsearchives.nseindia.com/content/cm/BhavCopy_NSE_CM_0_0_0_{}_F_0000.csv.zip", date_str)
        };

        rate_limit_delay();

        println!("Downloading: {}", url);

        let response = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
            .header("Referer", "https://www.nseindia.com/get-quotes/equity?symbol=HDFCBANK")
            .send()?;

        if !response.status().is_success() {
            // Skip if not available (holiday or error)
            current_date = current_date - chrono::Duration::days(1);
            continue;
        }

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
            fs::remove_file(&zip_path)?;
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
        {
            let conn = db_conn.lock().unwrap();
            let mut rdr = Reader::from_path(&csv_path)?;
            let mut rows: Vec<(String, String, i64, f64, f64, f64, f64, f64, f64, i64, f64, i64, String)> = Vec::new();
            for result in rdr.records() {
                let record = result?;
                if record.len() < 13 { continue; }
                let symbol = record[0].trim().to_uppercase();
                let series = record[1].trim().to_string();
                let open: f64 = record[2].trim().parse().unwrap_or(0.0);
                let high: f64 = record[3].trim().parse().unwrap_or(0.0);
                let low: f64 = record[4].trim().parse().unwrap_or(0.0);
                let close: f64 = record[5].trim().parse().unwrap_or(0.0);
                let last: f64 = record[6].trim().parse().unwrap_or(0.0);
                let prev_close: f64 = record[7].trim().parse().unwrap_or(0.0);
                let volume: i64 = record[8].trim().parse().unwrap_or(0);
                let turnover: f64 = record[9].trim().parse().unwrap_or(0.0);
                let trades: i64 = record[11].trim().parse().unwrap_or(0); // TOTALTRADES is index 11 (0-based)
                let isin = record[12].trim().to_string();
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

        // Send progress
        let _ = tx.send(crate::BhavCopyMessage::Progress(format!("Downloaded {}", date_str)));

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