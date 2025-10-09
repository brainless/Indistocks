use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed, record_recently_viewed, validate_download_records, get_bhavcopy_date_range, search_nse_symbols, StockData, get_stock_data_in_range};
use std::sync::{Arc, Mutex};
use crate::ui::{top_nav, sidebar, main_content, settings};
use chrono::NaiveDate;
use std::sync::mpsc::Receiver;
use indistocks_db::BhavCopyMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Home,
    Stocks,
    Settings,
    Logs,
}

pub struct IndistocksApp {
    pub current_view: View,
    pub db_conn: Arc<Mutex<Connection>>,
    pub recently_viewed: Vec<RecentlyViewed>,
    pub search_query: String,
    pub settings_error_symbols: Vec<String>,
    // BhavCopy Download
    pub bhavcopy_progress: String,
    pub bhavcopy_status: String,
    pub is_downloading_bhavcopy: bool,
    pub bhavcopy_receiver: Option<Receiver<BhavCopyMessage>>,
    pub bhavcopy_date_range: Option<(chrono::NaiveDate, chrono::NaiveDate)>,
    // NSE List Download
    pub is_downloading_nse_list: bool,
    pub nse_list_status: String,
    pub nse_list_receiver: Option<Receiver<crate::ui::settings::NseListMessage>>,
    // Plotting
    pub selected_symbol: Option<String>,
    pub plot_data: Vec<(NaiveDate, f64)>, // date, close price
    pub plot_loaded_range: Option<(NaiveDate, NaiveDate)>, // Track what data is currently loaded
    pub plot_earliest_available: Option<NaiveDate>, // Earliest date available in DB for current symbol
    pub plot_loading_in_progress: bool, // Prevent concurrent loads
    // Search caching
    pub last_search_query: String,
    pub search_results: Vec<String>,
    // Stocks page
    pub stocks_price_from: String,
    pub stocks_price_to: String,
    pub stocks_range_type: RangeType,
    pub stocks_cached_data: Vec<StockData>,
    pub stocks_last_price_from: String,
    pub stocks_last_price_to: String,
    pub stocks_last_range_type: RangeType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RangeType {
    Last5Days,
    Last30Days,
    Last52Weeks,
}

impl IndistocksApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, db_conn: Connection) -> Self {
        // Validate download records against existing files
        if let Err(e) = validate_download_records(&db_conn) {
            eprintln!("Failed to validate download records: {}", e);
        }

        // Load symbols with downloads once at startup
        let db_conn_arc = Arc::new(Mutex::new(db_conn));
        let conn = db_conn_arc.lock().unwrap();
        let bhavcopy_date_range = get_bhavcopy_date_range(&*conn).unwrap_or(None);

        Self {
            current_view: View::Home,
            db_conn: db_conn_arc.clone(),
            recently_viewed: get_recently_viewed(&*conn, 20).unwrap_or_default(),
            search_query: String::new(),
            settings_error_symbols: Vec::new(),
            bhavcopy_progress: String::new(),
            bhavcopy_status: String::new(),
            is_downloading_bhavcopy: false,
            bhavcopy_receiver: None,
            bhavcopy_date_range,
            is_downloading_nse_list: false,
            nse_list_status: String::new(),
            nse_list_receiver: None,
            selected_symbol: None,
            plot_data: Vec::new(),
            plot_loaded_range: None,
            plot_earliest_available: None,
            plot_loading_in_progress: false,
            last_search_query: String::new(),
            search_results: Vec::new(),
            stocks_price_from: String::new(),
            stocks_price_to: String::new(),
            stocks_range_type: RangeType::Last30Days,
            stocks_cached_data: Vec::new(),
            stocks_last_price_from: String::new(),
            stocks_last_price_to: String::new(),
            stocks_last_range_type: RangeType::Last30Days,
        }
    }

    pub fn refresh_recently_viewed(&mut self) {
        self.recently_viewed = get_recently_viewed(&*self.db_conn.lock().unwrap(), 20).unwrap_or_default();
    }

    pub fn update_search_results(&mut self) {
        if self.search_query == self.last_search_query {
            return; // No change, skip update
        }

        self.last_search_query = self.search_query.clone();

        self.search_results = search_nse_symbols(&*self.db_conn.lock().unwrap(), &self.search_query, 50).unwrap_or_default();
        println!("Search query: '{}', found {} matching symbols", self.search_query, self.search_results.len());
    }

    pub fn load_plot_data(&mut self, symbol: &str) {
        println!("Loading plot data for symbol: {}", symbol);
        self.selected_symbol = Some(symbol.to_string());

        // Record as recently viewed
        if let Err(e) = record_recently_viewed(&*self.db_conn.lock().unwrap(), symbol) {
            eprintln!("Failed to record recently viewed: {}", e);
        }
        self.refresh_recently_viewed();

        self.plot_data.clear();
        self.plot_loaded_range = None;
        self.plot_earliest_available = None;
        self.plot_loading_in_progress = false;

        let conn = self.db_conn.lock().unwrap();

        // Get the earliest and latest dates available for this symbol
        let earliest_date: Option<i64> = conn.query_row(
            "SELECT MIN(date) FROM bhavcopy_data WHERE symbol = ? AND series = 'EQ'",
            [symbol],
            |row| row.get(0)
        ).ok().flatten();

        let latest_date: Option<i64> = conn.query_row(
            "SELECT MAX(date) FROM bhavcopy_data WHERE symbol = ? AND series = 'EQ'",
            [symbol],
            |row| row.get(0)
        ).ok().flatten();

        if let (Some(earliest_ts), Some(latest_ts)) = (earliest_date, latest_date) {
            let earliest = chrono::DateTime::from_timestamp(earliest_ts, 0)
                .unwrap()
                .naive_utc()
                .date();
            let latest = chrono::DateTime::from_timestamp(latest_ts, 0)
                .unwrap()
                .naive_utc()
                .date();

            self.plot_earliest_available = Some(earliest);

            // Count total data points available
            let total_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM bhavcopy_data WHERE symbol = ? AND series = 'EQ'",
                [symbol],
                |row| row.get(0)
            ).unwrap_or(0);

            println!("Data available from {} to {} ({} days span, {} data points in DB)",
                earliest, latest, (latest - earliest).num_days(), total_count);

            // Load last 3 months of data initially
            let start = latest - chrono::Duration::days(90);
            let load_from = if start < earliest { earliest } else { start };

            match get_stock_data_in_range(&conn, symbol, load_from, latest) {
                Ok(data) => {
                    self.plot_data = data;
                    if !self.plot_data.is_empty() {
                        let actual_start = self.plot_data.first().unwrap().0;
                        let actual_end = self.plot_data.last().unwrap().0;
                        self.plot_loaded_range = Some((actual_start, actual_end));
                        println!("Loaded {} data points for {} (range: {} to {})",
                            self.plot_data.len(), symbol, actual_start, actual_end);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load plot data: {}", e);
                }
            }
        } else {
            println!("No data available for symbol: {}", symbol);
        }
    }

    /// Load additional data when user scrolls/drags to view earlier dates
    pub fn load_earlier_data(&mut self, symbol: &str, days_to_load: i64) {
        // Prevent concurrent loads
        if self.plot_loading_in_progress {
            return;
        }

        if let (Some((current_start, current_end)), Some(earliest_available)) =
            (self.plot_loaded_range, self.plot_earliest_available) {

            // Check if we've already loaded all available data
            if current_start <= earliest_available {
                println!("Already at earliest available date: {}", earliest_available);
                return;
            }

            self.plot_loading_in_progress = true;

            let new_start = current_start - chrono::Duration::days(days_to_load);
            let new_end = current_start - chrono::Duration::days(1);

            // Don't go before the earliest available date
            let load_from = if new_start < earliest_available {
                earliest_available
            } else {
                new_start
            };

            let conn = self.db_conn.lock().unwrap();
            match get_stock_data_in_range(&conn, symbol, load_from, new_end) {
                Ok(mut new_data) => {
                    if !new_data.is_empty() {
                        println!("Loading {} earlier data points (range: {} to {})",
                            new_data.len(), load_from, new_end);

                        // Prepend new data to existing data
                        new_data.extend(self.plot_data.drain(..));
                        self.plot_data = new_data;

                        // Update the loaded range
                        self.plot_loaded_range = Some((self.plot_data.first().unwrap().0, current_end));
                    } else {
                        println!("No earlier data available in range {} to {}", load_from, new_end);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load earlier data: {}", e);
                }
            }

            self.plot_loading_in_progress = false;
        }
    }
}

impl eframe::App for IndistocksApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update search results if needed
        self.update_search_results();



        // If there's a selected symbol or search query, switch to Home view
        if self.selected_symbol.is_some() || !self.search_query.is_empty() {
            self.current_view = View::Home;
        }

        // Top navigation
        egui::TopBottomPanel::top("top_nav").show(ctx, |ui| {
            top_nav::render(ui, self);
        });

        // Left sidebar
        egui::SidePanel::left("sidebar")
            .exact_width(250.0)
            .show(ctx, |ui| {
                sidebar::render(ui, self);
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_view {
                View::Home => main_content::render(ui, self),
                View::Stocks => crate::ui::stocks::render(ui, self),
                View::Settings => settings::render(ui, self),
                View::Logs => {
                    ui.heading("Logs");
                    ui.label("Logs view - Coming soon");
                }
            }
        });
    }
}
