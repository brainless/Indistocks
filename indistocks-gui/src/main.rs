mod app;
mod ui;

use app::IndistocksApp;
use indistocks_db::{init_db, populate_demo_data};

fn main() -> Result<(), eframe::Error> {
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
    )
}
