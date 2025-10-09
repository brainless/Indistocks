use crate::app::{IndistocksApp, RangeType};
use indistocks_db::get_all_stocks_with_metrics;

pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    ui.heading("Stocks");
    ui.add_space(10.0);

    // Filters row
    ui.horizontal(|ui| {
        ui.label("Price From:");
        ui.add_sized(
            [100.0, 20.0],
            egui::TextEdit::singleline(&mut app.stocks_price_from)
                .hint_text("Min")
        );

        ui.add_space(10.0);

        ui.label("Price To:");
        ui.add_sized(
            [100.0, 20.0],
            egui::TextEdit::singleline(&mut app.stocks_price_to)
                .hint_text("Max")
        );

        ui.add_space(20.0);

        ui.label("Low/High Range:");
        egui::ComboBox::from_id_salt("range_type")
            .selected_text(range_type_label(app.stocks_range_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.stocks_range_type, RangeType::Last5Days, "Last 5 Days");
                ui.selectable_value(&mut app.stocks_range_type, RangeType::Last30Days, "Last 30 Days");
                ui.selectable_value(&mut app.stocks_range_type, RangeType::Last52Weeks, "Last 52 Weeks");
            });
    });

    ui.add_space(10.0);

    // Check if filters changed - only reload if they did
    let filters_changed = app.stocks_price_from != app.stocks_last_price_from
        || app.stocks_price_to != app.stocks_last_price_to
        || app.stocks_range_type != app.stocks_last_range_type;

    if filters_changed || app.stocks_cached_data.is_empty() {
        // Parse filters
        let price_from = app.stocks_price_from.parse::<f64>().ok();
        let price_to = app.stocks_price_to.parse::<f64>().ok();
        let range_days = match app.stocks_range_type {
            RangeType::Last5Days => 5,
            RangeType::Last30Days => 30,
            RangeType::Last52Weeks => 365,
        };

        // Fetch data
        let conn = app.db_conn.lock().unwrap();
        app.stocks_cached_data = get_all_stocks_with_metrics(&*conn, price_from, price_to, range_days).unwrap_or_default();
        drop(conn);

        // Update last filter values
        app.stocks_last_price_from = app.stocks_price_from.clone();
        app.stocks_last_price_to = app.stocks_price_to.clone();
        app.stocks_last_range_type = app.stocks_range_type;
    }

    if app.stocks_cached_data.is_empty() {
        ui.label("No stock data available. Please download BhavCopy data from Settings.");
        return;
    }

    // Render virtual scrolling table
    render_virtual_table(ui, app);
}

fn range_type_label(range_type: RangeType) -> &'static str {
    match range_type {
        RangeType::Last5Days => "Last 5 Days",
        RangeType::Last30Days => "Last 30 Days",
        RangeType::Last52Weeks => "Last 52 Weeks",
    }
}

fn render_virtual_table(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    use egui_extras::{TableBuilder, Column};

    let total_rows = app.stocks_cached_data.len();

    // Clone the data to avoid borrow checker issues
    let stocks_data = app.stocks_cached_data.clone();
    let range_type = app.stocks_range_type;

    let mut symbol_to_load: Option<String> = None;

    // Dynamic column headers based on range type
    let range_low_header = match range_type {
        RangeType::Last5Days => "5D Low",
        RangeType::Last30Days => "30D Low",
        RangeType::Last52Weeks => "52W Low",
    };
    let range_high_header = match range_type {
        RangeType::Last5Days => "5D High",
        RangeType::Last30Days => "30D High",
        RangeType::Last52Weeks => "52W High",
    };

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto().at_least(100.0))  // Symbol
        .column(Column::auto().at_least(200.0))  // Name
        .column(Column::auto().at_least(80.0))   // LTP
        .column(Column::auto().at_least(80.0))   // % Change
        .column(Column::auto().at_least(100.0))  // Volume
        .column(Column::auto().at_least(80.0))   // Range Low
        .column(Column::auto().at_least(80.0))   // Range High
        .header(30.0, |mut header| {
            header.col(|ui| {
                ui.strong("Symbol");
            });
            header.col(|ui| {
                ui.strong("Name");
            });
            header.col(|ui| {
                ui.strong("LTP");
            });
            header.col(|ui| {
                ui.strong("% Change");
            });
            header.col(|ui| {
                ui.strong("Volume");
            });
            header.col(|ui| {
                ui.strong(range_low_header);
            });
            header.col(|ui| {
                ui.strong(range_high_header);
            });
        })
        .body(|body| {
            // Virtual scrolling: only render visible rows + buffer
            body.rows(25.0, total_rows, |mut row| {
                let row_index = row.index();
                if let Some(stock) = stocks_data.get(row_index) {
                    let symbol = stock.symbol.clone();
                    row.col(|ui| {
                        if ui.button(&symbol).clicked() {
                            symbol_to_load = Some(symbol.clone());
                        }
                    });
                    row.col(|ui| {
                        ui.label(stock.name.as_deref().unwrap_or("N/A"));
                    });
                    row.col(|ui| {
                        ui.label(format!("{:.2}", stock.ltp));
                    });
                    row.col(|ui| {
                        let color = if stock.change_percent > 0.0 {
                            egui::Color32::GREEN
                        } else if stock.change_percent < 0.0 {
                            egui::Color32::RED
                        } else {
                            ui.style().visuals.text_color()
                        };
                        ui.colored_label(color, format!("{:+.2}%", stock.change_percent));
                    });
                    row.col(|ui| {
                        ui.label(format_volume(stock.volume));
                    });
                    row.col(|ui| {
                        ui.label(format!("{:.2}", stock.range_low));
                    });
                    row.col(|ui| {
                        ui.label(format!("{:.2}", stock.range_high));
                    });
                }
            });
        });

    // Load plot data after table rendering to avoid borrow issues
    if let Some(symbol) = symbol_to_load {
        app.load_plot_data(&symbol);
    }
}

fn format_volume(volume: i64) -> String {
    if volume >= 10_000_000 {
        format!("{:.1}M", volume as f64 / 1_000_000.0)
    } else if volume >= 100_000 {
        format!("{:.1}L", volume as f64 / 100_000.0)
    } else if volume >= 1_000 {
        format!("{:.1}K", volume as f64 / 1_000.0)
    } else {
        volume.to_string()
    }
}
