use crate::app::{IndistocksApp, View};

pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    ui.horizontal(|ui| {
        ui.set_height(60.0);

        // Left spacing
        ui.add_space(10.0);

        // App title/logo
        ui.heading("Indistocks");

        ui.add_space(20.0);

        // Center search bar
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_sized(
                [400.0, 30.0],
                egui::TextEdit::singleline(&mut app.search_query)
                    .hint_text("Search stocks...")
            );
        });

        // Right side buttons
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0);

            // Settings button
            if ui.button("⚙").on_hover_text("Settings").clicked() {
                app.current_view = View::Settings;
            }

            ui.add_space(5.0);

            // Notifications button
            if ui.button("🔔").on_hover_text("Notifications").clicked() {
                // Future: show notifications
            }

            ui.add_space(10.0);

            // Stocks button
            if ui.button("Stocks").clicked() {
                app.current_view = View::Stocks;
            }
        });
    });

    ui.separator();
}
