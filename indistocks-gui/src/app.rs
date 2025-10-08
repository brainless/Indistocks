use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed, record_recently_viewed, get_downloaded_files_for_symbol, validate_download_records, get_symbols_with_downloads, get_nse_symbols_paginated, download_historical_data, save_download_record};
use std::sync::mpsc;
use crate::ui::{top_nav, sidebar, main_content, settings};
use chrono::NaiveDate;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Home,
    Settings,
    Logs,
}

pub struct IndistocksApp {
    pub current_view: View,
    pub db_conn: Connection,
    pub recently_viewed: Vec<RecentlyViewed>,
    pub search_query: String,
    pub settings_error_symbols: Vec<String>,
    // BhavCopy Download
    pub bhavcopy_progress: String,
    pub bhavcopy_status: String,
    pub is_downloading_bhavcopy: bool,
    pub bhavcopy_receiver: Option<Receiver<crate::ui::settings::BhavCopyMessage>>,
    // NSE List Download
    pub is_downloading_nse_list: bool,
    pub nse_list_status: String,
    pub nse_list_receiver: Option<Receiver<crate::ui::settings::NseListMessage>>,
    pub auto_download_receiver: Option<mpsc::Receiver<Vec<String>>>,
    pub downloading_symbol: Option<String>,
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

        let recently_viewed = get_recently_viewed(&db_conn, 20).unwrap_or_default();

        // Load symbols with downloads once at startup
        let symbols_with_downloads = get_symbols_with_downloads(&db_conn).unwrap_or_default();
        println!("Found {} symbols with downloads at startup", symbols_with_downloads.len());

        Self {
            current_view: View::Home,
            db_conn,
            recently_viewed: get_recently_viewed(&db_conn, 20).unwrap_or_default(),
            search_query: String::new(),
            settings_error_symbols: Vec::new(),
            bhavcopy_progress: String::new(),
            bhavcopy_status: String::new(),
            is_downloading_bhavcopy: false,
            bhavcopy_receiver: None,
            is_downloading_nse_list: false,
            nse_list_status: String::new(),
            nse_list_receiver: None,
            auto_download_receiver: None,
            downloading_symbol: None,
            selected_symbol: None,
            plot_data: Vec::new(),
            last_search_query: String::new(),
            search_results: Vec::new(),
            symbols_with_downloads,
        }
    }

    pub fn refresh_recently_viewed(&mut self) {
        self.recently_viewed = get_recently_viewed(&self.db_conn, 20).unwrap_or_default();
    }

    pub fn update_search_results(&mut self) {
        if self.search_query == self.last_search_query {
            return; // No change, skip update
        }

        self.last_search_query = self.search_query.clone();
        self.search_results.clear();

        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_uppercase();
        let mut all_symbols = Vec::new();

        // Get saved symbols
        if let Ok(saved_symbols) = get_nse_symbols_paginated(&self.db_conn, Some(50), None) {
            all_symbols.extend(saved_symbols);
        }

        // Add symbols with downloads (already loaded)
        for symbol in &self.symbols_with_downloads {
            if !all_symbols.contains(symbol) {
                all_symbols.push(symbol.clone());
            }
        }

        self.search_results = all_symbols.into_iter()
            .filter(|s| s.contains(&query))
            .take(20)
            .collect();

        println!("Search query: '{}', found {} matching symbols", query, self.search_results.len());
    }

    pub fn load_plot_data(&mut self, symbol: &str) {
        println!("Loading plot data for symbol: {}", symbol);
        self.selected_symbol = Some(symbol.to_string());

        // Record as recently viewed
        if let Err(e) = record_recently_viewed(&self.db_conn, symbol) {
            eprintln!("Failed to record recently viewed: {}", e);
        }
        self.refresh_recently_viewed();

        self.plot_data.clear();

        match get_downloaded_files_for_symbol(&self.db_conn, symbol) {
            Ok(files) => {
                self.downloading_symbol = None;
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
            }
            Err(e) => {
                println!("Error getting downloaded files for {}: {}", symbol, e);
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

        // Check for auto download completion
        if let Some(rx) = &self.auto_download_receiver {
            if let Ok(files) = rx.try_recv() {
                let now = chrono::Utc::now().date_naive();
                let one_year_ago = now - chrono::Duration::days(365);
                let from_ts = one_year_ago.and_hms_opt(0,0,0).unwrap().and_utc().timestamp();
                let to_ts = now.and_hms_opt(0,0,0).unwrap().and_utc().timestamp();
                for file_path in &files {
                    if let Err(e) = save_download_record(&self.db_conn, self.downloading_symbol.as_deref(), from_ts, to_ts, file_path, "completed", None) {
                        eprintln!("Failed to save download record: {}", e);
                    }
                }
                self.auto_download_receiver = None;
                self.downloading_symbol = None;
                // Reload plot if current symbol
                if let Some(ref sym) = self.selected_symbol.clone() {
                    self.load_plot_data(&sym);
                }
            }
        }

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
