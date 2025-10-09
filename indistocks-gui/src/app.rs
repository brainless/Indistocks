use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed, record_recently_viewed, validate_download_records, get_bhavcopy_date_range, search_nse_symbols};
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
            last_search_query: String::new(),
            search_results: Vec::new(),
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

        let conn = self.db_conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT date, close FROM bhavcopy_data WHERE symbol = ? ORDER BY date").unwrap();
        let rows = stmt.query_map([symbol], |row| {
            let ts: i64 = row.get(0).unwrap();
            let date = chrono::DateTime::from_timestamp(ts, 0).unwrap().naive_utc().date();
            let close: f64 = row.get(1).unwrap();
            Ok((date, close))
        }).unwrap();
        for row in rows {
            if let Ok((date, close)) = row {
                self.plot_data.push((date, close));
            }
        }
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
