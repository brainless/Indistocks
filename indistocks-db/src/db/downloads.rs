use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use directories::ProjectDirs;
use chrono::{Utc, NaiveDate, Datelike};
use reqwest::blocking::Client;
use std::time::Duration;
use std::thread;
use zip::ZipArchive;



#[derive(Debug, Clone, PartialEq)]
pub enum DownloadType {
    EquityBhavcopy,
    DeliveryBhavcopy,
    IndicesBhavcopy,
    Historical,
}

impl std::fmt::Display for DownloadType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadType::EquityBhavcopy => write!(f, "Equity Bhavcopy"),
            DownloadType::DeliveryBhavcopy => write!(f, "Delivery Bhavcopy"),
            DownloadType::IndicesBhavcopy => write!(f, "Indices Bhavcopy"),
            DownloadType::Historical => write!(f, "Historical Data"),
        }
    }
}

#[derive(Debug)]
pub struct DownloadRecord {
    pub id: i64,
    pub download_type: String,
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

fn download_file_with_retry(client: &Client, url: &str, file_path: &PathBuf, max_retries: usize) -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 0..max_retries {
        rate_limit_delay();

        let response = client.get(url).send()?;

        if !response.status().is_success() {
            if attempt == max_retries - 1 {
                return Err(format!("HTTP {}: {}", response.status(), url).into());
            }
            continue;
        }

        let content_type = response.headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        if content_type.contains("text/html") {
            return Err("File unavailable (HTML response)".into());
        }

        let mut file = fs::File::create(file_path)?;
        let content = response.bytes()?;
        std::io::copy(&mut content.as_ref(), &mut file)?;

        return Ok(());
    }
    Err("Max retries exceeded".into())
}

fn extract_zip(file_path: &PathBuf, extract_to: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::open(file_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = extract_to.join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

pub fn download_equity_bhavcopy(from_date: NaiveDate, to_date: NaiveDate) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();
    let mut downloaded_files = Vec::new();

    let mut current_date = from_date;
    while current_date <= to_date {
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let date_str = format!("{:02}{:02}{:04}", day, month % 100, year);
        let archive_url = "https://nsearchives.nseindia.com";

        // Check if new format (after 8 July 2024)
        let cutoff = NaiveDate::from_ymd_opt(2024, 7, 8).unwrap();
        let url = if current_date > cutoff {
            format!("{}/content/cm/BhavCopy_NSE_CM_0_0_0_{}_F_0000.csv.zip", archive_url, date_str)
        } else {
            let month_name = match month {
                1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR", 5 => "MAY", 6 => "JUN",
                7 => "JUL", 8 => "AUG", 9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
                _ => "JAN",
            };
            format!("{}/content/historical/EQUITIES/{}/{}/cm{}bhav.csv.zip", archive_url, year, month_name, date_str)
        };

        let year_dir = downloads_dir.join(year.to_string());
        let month_dir = year_dir.join(format!("{:02}", month));
        fs::create_dir_all(&month_dir)?;

        let zip_file_name = format!("equity_bhavcopy_{}.csv.zip", date_str);
        let zip_path = month_dir.join(&zip_file_name);

        match download_file_with_retry(&client, &url, &zip_path, 3) {
            Ok(_) => {
                // Extract ZIP
                let extract_dir = month_dir.join("extracted");
                fs::create_dir_all(&extract_dir)?;
                if let Err(e) = extract_zip(&zip_path, &extract_dir) {
                    eprintln!("Failed to extract {}: {}", zip_file_name, e);
                    continue;
                }

                // Find the CSV file
                let csv_files: Vec<_> = fs::read_dir(&extract_dir)?
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("csv"))
                    .collect();

                if let Some(csv_entry) = csv_files.first() {
                    let csv_path = csv_entry.path();
                    let final_csv_name = format!("equity_bhavcopy_{}.csv", date_str);
                    let final_csv_path = month_dir.join(&final_csv_name);
                    fs::rename(&csv_path, &final_csv_path)?;

                    downloaded_files.push(final_csv_name);
                }

                // Clean up
                fs::remove_file(&zip_path)?;
                fs::remove_dir_all(&extract_dir)?;
            }
            Err(e) => {
                eprintln!("Failed to download {}: {}", date_str, e);
            }
        }

        current_date = current_date.succ_opt().unwrap();
    }

    Ok(downloaded_files)
}

// Similar functions for other types would go here
// For brevity, I'll implement placeholders

pub fn download_delivery_bhavcopy(from_date: NaiveDate, to_date: NaiveDate) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();
    let mut downloaded_files = Vec::new();

    let mut current_date = from_date;
    while current_date <= to_date {
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let date_str = format!("{:02}{:02}{:04}", day, month % 100, year);
        let archive_url = "https://nsearchives.nseindia.com";
        let url = format!("{}/products/content/sec_bhavdata_full_{}.csv", archive_url, date_str);

        let year_dir = downloads_dir.join(year.to_string());
        let month_dir = year_dir.join(format!("{:02}", month));
        fs::create_dir_all(&month_dir)?;

        let file_name = format!("delivery_bhavcopy_{}.csv", date_str);
        let file_path = month_dir.join(&file_name);

        match download_file_with_retry(&client, &url, &file_path, 3) {
            Ok(_) => {
                downloaded_files.push(file_name);
            }
            Err(e) => {
                eprintln!("Failed to download {}: {}", date_str, e);
            }
        }

        current_date = current_date.succ_opt().unwrap();
    }

    Ok(downloaded_files)
}

pub fn download_indices_bhavcopy(from_date: NaiveDate, to_date: NaiveDate) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = create_http_client();
    let downloads_dir = get_downloads_dir();
    let mut downloaded_files = Vec::new();

    let mut current_date = from_date;
    while current_date <= to_date {
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let date_str = format!("{:02}{:02}{:04}", day, month % 100, year);
        let archive_url = "https://nsearchives.nseindia.com";
        let url = format!("{}/content/indices/ind_close_all_{}.csv", archive_url, date_str);

        let year_dir = downloads_dir.join(year.to_string());
        let month_dir = year_dir.join(format!("{:02}", month));
        fs::create_dir_all(&month_dir)?;

        let file_name = format!("indices_bhavcopy_{}.csv", date_str);
        let file_path = month_dir.join(&file_name);

        match download_file_with_retry(&client, &url, &file_path, 3) {
            Ok(_) => {
                downloaded_files.push(file_name);
            }
            Err(e) => {
                eprintln!("Failed to download {}: {}", date_str, e);
            }
        }

        current_date = current_date.succ_opt().unwrap();
    }

    Ok(downloaded_files)
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

    let mut file = fs::File::create(&file_path)?;
    let bytes = response.bytes()?;
    std::io::copy(&mut bytes.as_ref(), &mut file)?;

    downloaded_files.push(file_name);

    Ok(downloaded_files)
}

pub fn save_download_record(conn: &Connection, download_type: &str, symbol: Option<&str>, from_date: i64, to_date: i64, file_path: &str, status: &str, error_message: Option<&str>) -> Result<i64, Box<dyn std::error::Error>> {
    let now = Utc::now().timestamp();
    let file_size = fs::metadata(file_path).ok().map(|m| m.len() as i64);

    conn.execute(
        "INSERT INTO nse_downloads (download_type, symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            download_type,
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
        "SELECT id, download_type, symbol, from_date, to_date, file_path, file_size, status, error_message, downloaded_at
         FROM nse_downloads ORDER BY downloaded_at DESC LIMIT 50"
    )?;

    let records = stmt.query_map([], |row| {
        Ok(DownloadRecord {
            id: row.get(0)?,
            download_type: row.get(1)?,
            symbol: row.get(2)?,
            from_date: row.get(3)?,
            to_date: row.get(4)?,
            file_path: row.get(5)?,
            file_size: row.get(6)?,
            status: row.get(7)?,
            error_message: row.get(8)?,
            downloaded_at: row.get(9)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(records)
}