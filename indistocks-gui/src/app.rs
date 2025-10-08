use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed, record_recently_viewed, get_downloaded_files_for_symbol, validate_download_records, get_symbols_with_downloads, get_bhavcopy_date_range, get_bhavcopy_files, search_nse_symbols};
use std::sync::{Arc, Mutex};
use crate::ui::{top_nav, sidebar, main_content, settings};
use chrono::NaiveDate;
use std::sync::mpsc::Receiver;
use indistocks_db::BhavCopyMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Home,
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
    // Search caching
    pub last_search_query: String,
    pub search_results: Vec<String>,
    pub symbols_with_downloads: Vec<String>,
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
        let symbols_with_downloads = get_symbols_with_downloads(&*conn).unwrap_or_default();
        let bhavcopy_date_range = get_bhavcopy_date_range(&*conn).unwrap_or(None);
        println!("Found {} symbols with downloads at startup", symbols_with_downloads.len());

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
            last_search_query: String::new(),
            search_results: Vec::new(),
            symbols_with_downloads,
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

        match get_downloaded_files_for_symbol(&*self.db_conn.lock().unwrap(), symbol) {
            Ok(files) => {
                println!("Found {} downloaded files for {}", files.len(), symbol);
                for file_path in files {
                    println!("Loading file: {}", file_path);
                    // Load CSV and parse
                    if let Ok(mut rdr) = csv::Reader::from_path(&file_path) {
                        let mut count = 0;
                        let mut is_first_row = true;
                        for result in rdr.records() {
                            if let Ok(record) = result {
                                if is_first_row {
                                    // Skip header row
                                    is_first_row = false;
                                    continue;
                                }
                                // Date is column 2, Close Price is column 8 (0-indexed)
                                if let (Some(date_str), Some(close_str)) = (record.get(2), record.get(8)) {
                                    if let (Ok(date), Ok(close)) = (
                                        NaiveDate::parse_from_str(date_str.trim(), "%d-%b-%Y"), // NSE format, trim spaces
                                        close_str.trim().parse::<f64>()
                                    ) {
                                        self.plot_data.push((date, close));
                                        count += 1;
                                    }
                                }
                            }
                        }
                        println!("Loaded {} data points from {}", count, file_path);
                    } else {
                        println!("Failed to read CSV file: {}", file_path);
                    }
                }
            }
            Err(e) => {
                println!("Error getting downloaded files for {}: {}", symbol, e);
            }
        }

        // If no per-symbol data, load from bhavcopy
        if self.plot_data.is_empty() {
            match get_bhavcopy_files(&*self.db_conn.lock().unwrap()) {
                Ok(bhavcopy_files) => {
                    println!("Loading from {} bhavcopy files for {}", bhavcopy_files.len(), symbol);
                    for file_path in bhavcopy_files {
                        if let Ok(mut rdr) = csv::Reader::from_path(&file_path) {
                            let mut is_first_row = true;
                            for result in rdr.records() {
                                if let Ok(record) = result {
                                    if is_first_row {
                                        is_first_row = false;
                                        continue;
                                    }
                                    if let (Some(sym), Some(date_str), Some(close_str)) = (record.get(7), record.get(0), record.get(17)) { // TckrSymb, TradDt, ClsPric
                                        if sym.trim() == symbol {
                                            if let (Ok(date), Ok(close)) = (
                                                NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d"),
                                                close_str.trim().parse::<f64>()
                                            ) {
                                                self.plot_data.push((date, close));
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("Failed to read bhavcopy CSV file: {}", file_path);
                        }
                    }
                }
                Err(e) => {
                    println!("Error getting bhavcopy files: {}", e);
                }
            }
        }

        // Sort by date
        self.plot_data.sort_by_key(|(date, _)| *date);
        println!("Total data points loaded: {}", self.plot_data.len());
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
                View::Settings => settings::render(ui, self),
                View::Logs => {
                    ui.heading("Logs");
                    ui.label("Logs view - Coming soon");
                }
            }
        });
    }
}
