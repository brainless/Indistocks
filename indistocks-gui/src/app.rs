use indistocks_db::{Connection, RecentlyViewed, get_recently_viewed};
use crate::ui::{top_nav, sidebar, main_content, settings};

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
}

impl IndistocksApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, db_conn: Connection) -> Self {
        let recently_viewed = get_recently_viewed(&db_conn, 20).unwrap_or_default();

        Self {
            current_view: View::Home,
            db_conn,
            recently_viewed,
            search_query: String::new(),
            settings_nse_symbols: String::new(),
            settings_error_symbols: Vec::new(),
            settings_success_message: None,
        }
    }

    pub fn refresh_recently_viewed(&mut self) {
        self.recently_viewed = get_recently_viewed(&self.db_conn, 20).unwrap_or_default();
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
