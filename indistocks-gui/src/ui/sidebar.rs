use crate::app::IndistocksApp;

pub fn render(ui: &mut egui::Ui, app: &IndistocksApp) {
    ui.vertical(|ui| {
        ui.add_space(10.0);

        ui.heading("Recently Viewed");

        ui.add_space(10.0);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &app.recently_viewed {
                ui.add_space(5.0);

                if ui.button(&item.symbol).clicked() {
                    // Future: navigate to stock detail view
                }

                if let Some(name) = &item.name {
                    ui.label(
                        egui::RichText::new(name)
                            .small()
                            .color(egui::Color32::GRAY)
                    );
                }

                ui.add_space(5.0);
            }
        });
    });
}
