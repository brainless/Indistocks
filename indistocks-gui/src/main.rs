mod app;
mod ui;

use app::IndistocksApp;
use indistocks_db::{init_db, populate_demo_data, clear_bhavcopy_data};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "indistocks")]
#[command(about = "Indian Stock Market Analysis Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run in test mode: download 5 days of data and query a stock
    Test {
        /// Symbol to test (e.g., RELIANCE, TCS, HDFCBANK)
        #[arg(short, long, default_value = "RELIANCE")]
        symbol: String,
    },
}

fn test_mode(symbol: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== INDISTOCKS TEST MODE ===");
    println!("Testing with symbol: {}\n", symbol);

    // Initialize database
    println!("1. Initializing database...");
    let conn = init_db()?;
    println!("   ✓ Database initialized\n");

    // Clear existing bhavcopy data
    println!("2. Clearing existing BhavCopy data...");
    clear_bhavcopy_data(&conn)?;
    println!("   ✓ Data cleared\n");

    // Download 5 days of data
    println!("3. Downloading 5 days of BhavCopy data...");
    use chrono::{Utc, Datelike, Duration};
    use std::fs;
    use indistocks_db::get_downloads_dir;
    use reqwest::blocking::Client;
    use std::sync::{Arc, Mutex};
    use csv::Reader;

    let conn_arc = Arc::new(Mutex::new(conn));
    let downloads_dir = get_downloads_dir();
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
        .timeout(std::time::Duration::from_secs(15))
        .cookie_store(true)
        .gzip(true)
        .build()?;

    let mut current_date = Utc::now().date_naive() - Duration::days(1);
    let mut downloaded_count = 0;
    let target_downloads = 5;

    while downloaded_count < target_downloads && downloaded_count < 30 {
        let date_str = current_date.format("%Y%m%d").to_string();
        let year = current_date.year();
        let month = current_date.month();
        let day = current_date.day();

        let url = format!("https://nsearchives.nseindia.com/content/cm/BhavCopy_NSE_CM_0_0_0_{}_F_0000.csv.zip", date_str);

        std::thread::sleep(std::time::Duration::from_millis(350));

        println!("   Attempting to download: {} ({})", current_date.format("%Y-%m-%d"), url);

        let response = client.get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; rv:109.0) Gecko/20100101 Firefox/118.0")
            .header("Referer", "https://www.nseindia.com/")
            .send();

        match response {
            Ok(resp) if resp.status().is_success() => {
                // Create directory
                let year_dir = downloads_dir.join(year.to_string());
                let month_dir = year_dir.join(format!("{:02}", month));
                fs::create_dir_all(&month_dir)?;

                let zip_path = month_dir.join(format!("bhavcopy_{}.zip", date_str));
                let csv_path = month_dir.join(format!("bhavcopy_{}.csv", date_str));

                // Download ZIP
                let bytes = resp.bytes()?;
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
                    current_date = current_date - Duration::days(1);
                    continue;
                }

                // Save CSV
                fs::write(&csv_path, &csv_data)?;
                fs::remove_file(&zip_path)?;

                // Parse and insert data
                let ts = current_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
                let conn = conn_arc.lock().unwrap();

                let mut rdr = Reader::from_path(&csv_path)?;
                let headers = rdr.headers()?.clone();
                println!("   CSV Headers: {:?}", headers);

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
                    let sym = record.get(symbol_idx).unwrap_or("").trim().to_uppercase();
                    if sym.is_empty() { continue; }
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
                    rows.push((sym, series, ts, open, high, low, close, last, prev_close, volume, turnover, trades, isin));
                }

                for chunk in rows.chunks(100) {
                    if chunk.is_empty() { continue; }
                    let placeholders: Vec<String> = chunk.iter().map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string()).collect();
                    let query = format!("INSERT OR IGNORE INTO bhavcopy_data (symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin) VALUES {}", placeholders.join(", "));
                    let params: Vec<&dyn rusqlite::ToSql> = chunk.iter().flat_map(|(symbol, series, date, open, high, low, close, last, prev_close, volume, turnover, trades, isin)|
                        vec![symbol as &dyn rusqlite::ToSql, series as &dyn rusqlite::ToSql, date as &dyn rusqlite::ToSql,
                             open as &dyn rusqlite::ToSql, high as &dyn rusqlite::ToSql, low as &dyn rusqlite::ToSql,
                             close as &dyn rusqlite::ToSql, last as &dyn rusqlite::ToSql, prev_close as &dyn rusqlite::ToSql,
                             volume as &dyn rusqlite::ToSql, turnover as &dyn rusqlite::ToSql, trades as &dyn rusqlite::ToSql,
                             isin as &dyn rusqlite::ToSql]).collect();
                    conn.execute(&query, rusqlite::params_from_iter(params))?;
                }

                fs::remove_file(&csv_path)?;

                downloaded_count += 1;
                println!("   ✓ Downloaded and processed: {}", current_date.format("%Y-%m-%d"));
            }
            _ => {
                println!("   ✗ Not available: {}", current_date.format("%Y-%m-%d"));
            }
        }

        current_date = current_date - Duration::days(1);
    }

    println!("   ✓ Downloaded {} days of data\n", downloaded_count);

    // Query data for the test symbol
    println!("4. Querying data for symbol '{}'...", symbol);
    let conn = conn_arc.lock().unwrap();

    let total_rows: i64 = conn.query_row("SELECT COUNT(*) FROM bhavcopy_data", [], |row| row.get(0))?;
    println!("   Total rows in bhavcopy_data: {}", total_rows);

    let symbol_rows: i64 = conn.query_row("SELECT COUNT(*) FROM bhavcopy_data WHERE symbol = ?", [symbol], |row| row.get(0))?;
    println!("   Rows for symbol '{}': {}", symbol, symbol_rows);

    let mut series_stmt = conn.prepare("SELECT DISTINCT series FROM bhavcopy_data WHERE symbol = ?")?;
    let series_list: Vec<String> = series_stmt.query_map([symbol], |row| row.get(0))?.collect::<Result<Vec<_>, _>>()?;
    println!("   Series available for '{}': {:?}", symbol, series_list);

    let eq_rows: i64 = conn.query_row("SELECT COUNT(*) FROM bhavcopy_data WHERE symbol = ? AND series = 'EQ'", [symbol], |row| row.get(0))?;
    println!("   EQ series rows for '{}': {}", symbol, eq_rows);

    println!("\n5. Sample data for '{}':", symbol);
    let mut stmt = conn.prepare("SELECT date, open, high, low, close, volume, series FROM bhavcopy_data WHERE symbol = ? ORDER BY date DESC LIMIT 10")?;
    let rows = stmt.query_map([symbol], |row| {
        let ts: i64 = row.get(0)?;
        let date = chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.naive_utc().date()).unwrap_or_default();
        let open: f64 = row.get(1)?;
        let high: f64 = row.get(2)?;
        let low: f64 = row.get(3)?;
        let close: f64 = row.get(4)?;
        let volume: i64 = row.get(5)?;
        let series: String = row.get(6)?;
        Ok((date, open, high, low, close, volume, series))
    })?;

    println!("   Date       | Series | Open    | High    | Low     | Close   | Volume");
    println!("   -----------|--------|---------|---------|---------|---------|----------");
    for row in rows {
        let (date, open, high, low, close, volume, series) = row?;
        println!("   {} | {}     | {:7.2} | {:7.2} | {:7.2} | {:7.2} | {}",
                 date, series, open, high, low, close, volume);
    }

    println!("\n6. Sample symbols from database:");
    let mut sample_stmt = conn.prepare("SELECT DISTINCT symbol FROM bhavcopy_data WHERE series = 'EQ' LIMIT 10")?;
    let sample_symbols: Vec<String> = sample_stmt.query_map([], |row| row.get(0))?.collect::<Result<Vec<_>, _>>()?;
    println!("   {:?}", sample_symbols);

    println!("\n=== TEST COMPLETE ===");
    println!("Summary:");
    println!("  - Downloaded {} days of data", downloaded_count);
    println!("  - Total rows in database: {}", total_rows);
    println!("  - Rows for '{}': {}", symbol, symbol_rows);
    println!("  - EQ series rows for '{}': {}", symbol, eq_rows);

    if eq_rows > 0 {
        println!("\n✓ TEST PASSED: Data successfully downloaded and queryable!");
    } else {
        println!("\n✗ TEST FAILED: No EQ series data found for '{}'", symbol);
        println!("  Available series: {:?}", series_list);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Test { symbol }) => {
            test_mode(&symbol)?;
            Ok(())
        }
        None => {
            // Initialize database
            let conn = init_db().expect("Failed to initialize database");

            // Populate demo data (only if empty)
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM nse_symbols", [], |row| row.get(0))
                .unwrap_or(0);

            if count == 0 {
                populate_demo_data(&conn).expect("Failed to populate demo data");
            }

            // Configure window options
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([1200.0, 800.0])
                    .with_min_inner_size([800.0, 600.0]),
                ..Default::default()
            };

            // Run the app
            eframe::run_native(
                "Indistocks",
                options,
                Box::new(|cc| Ok(Box::new(IndistocksApp::new(cc, conn)))),
            )?;
            Ok(())
        }
    }
}
