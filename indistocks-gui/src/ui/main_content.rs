use crate::app::IndistocksApp;

pub fn render(ui: &mut egui::Ui, _app: &IndistocksApp) {
    ui.vertical_centered(|ui| {
        ui.add_space(50.0);
        ui.heading("Welcome to Indistocks");
        ui.add_space(20.0);
        ui.label("Select a stock from the sidebar or search above");
    });
}
