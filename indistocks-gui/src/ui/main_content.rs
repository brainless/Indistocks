use crate::app::IndistocksApp;
use indistocks_db::{get_nse_symbols_paginated, get_symbols_with_downloads};

pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    if let Some(symbol) = &app.selected_symbol {
        ui.heading(format!("Historical Data for {}", symbol));
        ui.add_space(10.0);

        if app.plot_data.is_empty() {
            ui.label("No downloaded data available for this symbol.");
        } else {
            // Plot the data
            let plot = egui_plot::Plot::new("price_plot")
                .height(400.0)
                .legend(egui_plot::Legend::default());

            plot.show(ui, |plot_ui| {
                let points: egui_plot::PlotPoints = app.plot_data
                    .iter()
                    .map(|(date, price)| {
                        let x = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;
                        [x, *price]
                    })
                    .collect();
                let line = egui_plot::Line::new(points)
                    .name("Close Price");
                plot_ui.line(line);
            });
        }

        if ui.button("Back").clicked() {
            app.selected_symbol = None;
            app.plot_data.clear();
        }
    } else if !app.search_query.is_empty() {
        // Show search results
        ui.heading("Search Results");
        ui.add_space(10.0);

        let query = app.search_query.to_uppercase();
        let mut all_symbols = Vec::new();

        // Get saved symbols
        if let Ok(saved_symbols) = get_nse_symbols_paginated(&app.db_conn, Some(50), None) {
            println!("Found {} saved symbols", saved_symbols.len());
            all_symbols.extend(saved_symbols);
        } else {
            println!("Failed to get saved symbols");
        }

        // Get symbols with downloads
        if let Ok(download_symbols) = get_symbols_with_downloads(&app.db_conn) {
            println!("Found {} symbols with downloads: {:?}", download_symbols.len(), download_symbols);
            for symbol in download_symbols {
                if !all_symbols.contains(&symbol) {
                    all_symbols.push(symbol);
                }
            }
        } else {
            println!("Failed to get symbols with downloads");
        }

        println!("Total unique symbols for search: {}", all_symbols.len());

        let filtered: Vec<_> = all_symbols.into_iter()
            .filter(|s| s.contains(&query))
            .take(20)
            .collect();

        println!("Search query: {}, found {} matching symbols: {:?}", query, filtered.len(), filtered);

        for symbol in filtered {
            if ui.button(&symbol).clicked() {
                app.load_plot_data(&symbol);
            }
        }
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.heading("Welcome to Indistocks");
            ui.add_space(20.0);
            ui.label("Select a stock from the sidebar or search above");
        });
    }
}
