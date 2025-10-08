use crate::app::IndistocksApp;


pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    if let Some(symbol) = &app.selected_symbol {
        ui.heading(format!("Historical Data for {}", symbol));
        ui.add_space(10.0);

        if app.plot_data.is_empty() {
            if app.downloading_symbol.as_ref() == Some(symbol) {
                ui.label("Downloading recent data, please check this page shortly.");
            } else {
                ui.label("No downloaded data available for this symbol.");
            }
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

        let mut symbol_to_load = None;
        for symbol in &app.search_results {
            if ui.button(symbol).clicked() {
                symbol_to_load = Some(symbol.clone());
            }
        }

        if let Some(symbol) = symbol_to_load {
            app.load_plot_data(&symbol);
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
