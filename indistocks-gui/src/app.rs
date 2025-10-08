use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed, DownloadType, get_downloaded_files_for_symbol, validate_download_records};
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
    pub settings_nse_symbols: String,
    pub settings_error_symbols: Vec<String>,
    pub settings_success_message: Option<String>,
    // NSE Downloads fields
    pub download_type: DownloadType,
    pub download_symbol: String,
    pub download_all_symbols: bool,
    pub download_from_date: String,
    pub download_to_date: String,
    pub download_progress: String,
    pub download_status: String,
    pub downloaded_files: Vec<String>,
    pub is_downloading: bool,
    pub download_receiver: Option<Receiver<crate::ui::settings::DownloadMessage>>,
    // Plotting
    pub selected_symbol: Option<String>,
    pub plot_data: Vec<(NaiveDate, f64)>, // date, close price
}

impl IndistocksApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, db_conn: Connection) -> Self {
        // Validate download records against existing files
        if let Err(e) = validate_download_records(&db_conn) {
            eprintln!("Failed to validate download records: {}", e);
        }

        let recently_viewed = get_recently_viewed(&db_conn, 20).unwrap_or_default();

        Self {
            current_view: View::Home,
            db_conn,
            recently_viewed,
            search_query: String::new(),
            settings_nse_symbols: String::new(),
            settings_error_symbols: Vec::new(),
            settings_success_message: None,
            download_type: DownloadType::EquityBhavcopy,
            download_symbol: String::new(),
            download_all_symbols: false,
            download_from_date: String::new(),
            download_to_date: String::new(),
            download_progress: String::new(),
            download_status: String::new(),
            downloaded_files: Vec::new(),
            is_downloading: false,
            download_receiver: None,
            selected_symbol: None,
            plot_data: Vec::new(),
        }
    }

    pub fn refresh_recently_viewed(&mut self) {
        self.recently_viewed = get_recently_viewed(&self.db_conn, 20).unwrap_or_default();
    }

    pub fn load_plot_data(&mut self, symbol: &str) {
        println!("Loading plot data for symbol: {}", symbol);
        self.selected_symbol = Some(symbol.to_string());
        self.plot_data.clear();

        match get_downloaded_files_for_symbol(&self.db_conn, symbol) {
            Ok(files) => {
                println!("Found {} downloaded files for {}", files.len(), symbol);
                for file_path in files {
                    println!("Loading file: {}", file_path);
                    // Load CSV and parse
                    if let Ok(mut rdr) = csv::Reader::from_path(&file_path) {
                        let mut count = 0;
                        for result in rdr.records() {
                            if let Ok(record) = result {
                                // Assume first column is date, fifth is close (0-indexed: 0=date, 4=close)
                                if let (Some(date_str), Some(close_str)) = (record.get(0), record.get(4)) {
                                    if let (Ok(date), Ok(close)) = (
                                        NaiveDate::parse_from_str(date_str, "%d-%b-%Y"), // NSE format
                                        close_str.parse::<f64>()
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
        // Sort by date
        self.plot_data.sort_by_key(|(date, _)| *date);
        println!("Total data points loaded: {}", self.plot_data.len());
    }
}

impl eframe::App for IndistocksApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
