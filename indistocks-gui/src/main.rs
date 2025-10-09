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

    // Download 5 days of data using the same function as GUI
    println!("3. Downloading 5 days of BhavCopy data...");
    use std::sync::{Arc, Mutex, mpsc};
    use indistocks_db::{BhavCopyMessage, download_bhavcopy_with_limit};

    let conn_arc = Arc::new(Mutex::new(conn));
    let (tx, rx) = mpsc::channel();

    // Spawn download in a thread (same as GUI)
    let conn_clone = conn_arc.clone();
    std::thread::spawn(move || {
        let result = download_bhavcopy_with_limit(&conn_clone, &tx, Some(5));
        let _ = tx.send(BhavCopyMessage::Done(result.map_err(|e| e.to_string())));
    });

    // Process messages (same as GUI would)
    loop {
        match rx.recv() {
            Ok(BhavCopyMessage::Progress(msg)) => {
                println!("   {}", msg);
            }
            Ok(BhavCopyMessage::DateRangeUpdated(_min, _max)) => {
                // Date range updated, GUI would update display here
            }
            Ok(BhavCopyMessage::Done(result)) => {
                match result {
                    Ok(()) => println!("   ✓ Download completed successfully"),
                    Err(e) => println!("   ✗ Download error: {}", e),
                }
                break;
            }
            Err(_) => {
                println!("   ✗ Channel disconnected");
                break;
            }
        }
    }

    // Give a moment for any pending operations to complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Count how many files were actually downloaded
    let conn = conn_arc.lock().unwrap();
    let downloaded_count: i64 = conn.query_row("SELECT COUNT(DISTINCT date) FROM bhavcopy_data", [], |row| row.get(0))?;
    println!("   ✓ Downloaded {} days of data\n", downloaded_count);

    // Query data for the test symbol
    println!("4. Querying data for symbol '{}'...", symbol);

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
