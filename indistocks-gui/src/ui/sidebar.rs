use crate::app::IndistocksApp;

pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    ui.vertical(|ui| {
        ui.add_space(10.0);

        ui.heading("Recently Viewed");

        ui.add_space(10.0);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let items: Vec<_> = app.recently_viewed.iter().map(|item| (item.symbol.clone(), item.name.clone())).collect();
            for (symbol, name) in items {
                ui.add_space(5.0);

                if ui.button(&symbol).clicked() {
                    app.load_plot_data(&symbol);
                }

                if let Some(name) = name {
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
