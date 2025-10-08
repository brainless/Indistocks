use crate::app::{IndistocksApp, View};
use indistocks_db::save_nse_symbols;

pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        // Header with back/close navigation
        ui.horizontal(|ui| {
            ui.heading("Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").on_hover_text("Close").clicked() {
                    app.current_view = View::Home;
                    app.settings_success_message = None;
                    app.settings_error_symbols.clear();
                }
            });
        });

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(20.0);

        // Link to Logs page
        ui.horizontal(|ui| {
            ui.label("View application");
            if ui.link("Logs").clicked() {
                app.current_view = View::Logs;
            }
        });

        ui.add_space(30.0);

        // NSE Stocks section
        ui.heading("NSE Stocks");
        ui.add_space(10.0);

        ui.label("Enter NSE stock symbols (comma-separated):");
        ui.add_space(5.0);

        let _textarea_response = ui.add_sized(
            [600.0, 150.0],
            egui::TextEdit::multiline(&mut app.settings_nse_symbols)
                .hint_text("e.g., RELIANCE, TCS, INFY, HDFCBANK")
        );

        ui.add_space(10.0);

        // Save button
        if ui.button("Save").clicked() {
            let symbols: Vec<String> = app.settings_nse_symbols
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            match save_nse_symbols(&app.db_conn, symbols) {
                Ok((count, errors)) => {
                    app.settings_error_symbols = errors;
                    app.settings_success_message = Some(format!("Saved {} symbols successfully", count));
                    app.refresh_recently_viewed();
                }
                Err(e) => {
                    app.settings_success_message = Some(format!("Error saving symbols: {}", e));
                    app.settings_error_symbols.clear();
                }
            }
        }

        ui.add_space(10.0);

        // Show success message
        if let Some(ref msg) = app.settings_success_message {
            ui.label(
                egui::RichText::new(msg)
                    .color(egui::Color32::from_rgb(0, 150, 0))
            );
        }

        // Show error symbols
        if !app.settings_error_symbols.is_empty() {
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Symbols with errors:")
                    .color(egui::Color32::from_rgb(200, 0, 0))
            );
            ui.label(
                egui::RichText::new(app.settings_error_symbols.join(", "))
                    .color(egui::Color32::from_rgb(200, 0, 0))
                    .small()
            );
        }

        ui.add_space(20.0);
    });
}
