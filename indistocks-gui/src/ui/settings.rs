use crate::app::{IndistocksApp, View};
use indistocks_db::{save_nse_symbols_with_names, download_equity_bhavcopy, download_delivery_bhavcopy, download_indices_bhavcopy, download_historical_data, get_nse_symbols, DownloadType};
use chrono::NaiveDate;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

#[derive(Debug)]
pub enum DownloadMessage {
    Progress(String),
    Done(Result<Vec<String>, String>),
}

#[derive(Debug)]
pub enum NseListMessage {
    Done(Result<Vec<(String, String)>, String>),
}

fn download_nse_equity_list() -> Result<Vec<(String, String)>, String> {
    let url = "https://nsearchives.nseindia.com/content/equities/EQUITY_L.csv";
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("Failed to download: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }
    
    let content = response.text()
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut symbols = Vec::new();
    
    for result in rdr.records() {
        let record = result.map_err(|e| format!("CSV parse error: {}", e))?;
        if let (Some(symbol), Some(name)) = (record.get(0), record.get(1)) {
            if !symbol.trim().is_empty() && !name.trim().is_empty() {
                symbols.push((symbol.trim().to_string(), name.trim().to_string()));
            }
        }
    }
    
    Ok(symbols)
}

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

        ui.label("Download the official NSE equity list to populate the database:");
        ui.add_space(10.0);

        // Download button
        if ui.button("Download NSE Equity list").clicked() && !app.is_downloading_nse_list {
            app.is_downloading_nse_list = true;
            app.nse_list_status = "Downloading...".to_string();

            let (tx, rx) = mpsc::channel();
            app.nse_list_receiver = Some(rx);

            thread::spawn(move || {
                let result = download_nse_equity_list();
                let _ = tx.send(NseListMessage::Done(result));
            });
        }

        ui.add_space(10.0);

        // Show status
        if !app.nse_list_status.is_empty() {
            ui.label(&app.nse_list_status);
        }

        // Check for messages
        if let Some(ref rx) = app.nse_list_receiver {
            match rx.try_recv() {
                Ok(message) => {
                    match message {
                        NseListMessage::Done(result) => {
                            app.is_downloading_nse_list = false;
                            app.nse_list_receiver = None;
                            match result {
                                Ok(symbols) => {
                                    match save_nse_symbols_with_names(&app.db_conn, symbols) {
                                        Ok((count, errors)) => {
                                            app.nse_list_status = format!("Downloaded and saved {} symbols successfully", count);
                                            if !errors.is_empty() {
                                                app.nse_list_status.push_str(&format!(" ({} errors)", errors.len()));
                                            }
                                            app.refresh_recently_viewed();
                                        }
                                        Err(e) => {
                                            app.nse_list_status = format!("Error saving symbols: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    app.nse_list_status = format!("Error downloading: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    // Still downloading
                }
                Err(TryRecvError::Disconnected) => {
                    app.is_downloading_nse_list = false;
                    app.nse_list_receiver = None;
                    app.nse_list_status = "Download thread disconnected".to_string();
                }
            }
        }

        ui.add_space(30.0);

        // NSE Downloads section
        ui.heading("NSE Downloads");
        ui.add_space(10.0);

        // Download Type
        ui.label("Download Type:");
        egui::ComboBox::from_label("")
            .selected_text(format!("{}", app.download_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.download_type, DownloadType::EquityBhavcopy, "Equity Bhavcopy");
                ui.selectable_value(&mut app.download_type, DownloadType::DeliveryBhavcopy, "Delivery Bhavcopy");
                ui.selectable_value(&mut app.download_type, DownloadType::IndicesBhavcopy, "Indices Bhavcopy");
                ui.selectable_value(&mut app.download_type, DownloadType::Historical, "Historical Data");
            });

        ui.add_space(10.0);

        // Symbol input (only for Historical)
        if matches!(app.download_type, DownloadType::Historical) {
            ui.checkbox(&mut app.download_all_symbols, "Download for all saved NSE symbols");
            ui.add_space(5.0);

            if !app.download_all_symbols {
                ui.label("Symbol:");
                ui.text_edit_singleline(&mut app.download_symbol);
            }
            ui.add_space(10.0);
        }

        // Date inputs
        ui.horizontal(|ui| {
            ui.label("From Date (YYYY-MM-DD):");
            ui.text_edit_singleline(&mut app.download_from_date);
        });
        ui.horizontal(|ui| {
            ui.label("To Date (YYYY-MM-DD):");
            ui.text_edit_singleline(&mut app.download_to_date);
        });

        ui.add_space(10.0);

        // Download button
        if ui.button("Download").clicked() && !app.is_downloading {
            let from_date = NaiveDate::parse_from_str(&app.download_from_date, "%Y-%m-%d");
            let to_date = NaiveDate::parse_from_str(&app.download_to_date, "%Y-%m-%d");

            match (from_date, to_date) {
                (Ok(from_date), Ok(to_date)) => {
                    app.is_downloading = true;
                    app.download_progress = "Downloading...".to_string();
                    app.download_status = String::new();
                    app.downloaded_files.clear();

                    let download_type = app.download_type.clone();
                    let download_all_symbols = app.download_all_symbols;
                    let download_symbol = app.download_symbol.clone();

                    // Get symbols here if needed for all symbols download
                    let symbols = if matches!(download_type, DownloadType::Historical) && download_all_symbols {
                        get_nse_symbols(&app.db_conn).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    let (tx, rx) = mpsc::channel();
                    app.download_receiver = Some(rx);

                    thread::spawn(move || {
                        let result = match download_type {
                            DownloadType::EquityBhavcopy => {
                                let _ = tx.send(DownloadMessage::Progress("Downloading equity bhavcopy...".to_string()));
                                download_equity_bhavcopy(from_date, to_date)
                            }
                            DownloadType::DeliveryBhavcopy => {
                                let _ = tx.send(DownloadMessage::Progress("Downloading delivery bhavcopy...".to_string()));
                                download_delivery_bhavcopy(from_date, to_date)
                            }
                            DownloadType::IndicesBhavcopy => {
                                let _ = tx.send(DownloadMessage::Progress("Downloading indices bhavcopy...".to_string()));
                                download_indices_bhavcopy(from_date, to_date)
                            }
                            DownloadType::Historical => {
                                if download_all_symbols {
                                    let total = symbols.len();
                                    let mut all_files = Vec::new();
                                    let batch_size = 10;

                                    for (batch_idx, batch) in symbols.chunks(batch_size).enumerate() {
                                        let batch_start = batch_idx * batch_size + 1;
                                        let batch_end = (batch_start + batch.len() - 1).min(total);
                                        let progress = format!("Downloading batch {}-{} of {}", batch_start, batch_end, total);
                                        let _ = tx.send(DownloadMessage::Progress(progress));

                                        let mut handles = Vec::new();
                                        for symbol in batch {
                                            let symbol_clone = symbol.clone();
                                            let from = from_date;
                                            let to = to_date;
                                            let handle = thread::spawn(move || {
                                                download_historical_data(&symbol_clone, from, to).map_err(|e| e.to_string())
                                            });
                                            handles.push((symbol, handle));
                                        }

                                        for (symbol, handle) in handles {
                                            match handle.join() {
                                                Ok(result) => match result {
                                                    Ok(mut files) => all_files.append(&mut files),
                                                    Err(e) => eprintln!("Failed for {}: {}", symbol, e),
                                                },
                                                Err(_) => eprintln!("Thread panicked for {}", symbol),
                                            }
                                        }
                                    }
                                    Ok(all_files)
                                } else {
                                    let _ = tx.send(DownloadMessage::Progress(format!("Downloading for {}", download_symbol)));
                                    download_historical_data(&download_symbol, from_date, to_date)
                                }
                            }
                        };

                        let _ = tx.send(DownloadMessage::Done(result.map_err(|e| e.to_string())));
                    });
                }
                _ => {
                    app.download_status = "Invalid date format. Use YYYY-MM-DD".to_string();
                }
            }
        }

        // Check for download messages
        if let Some(ref rx) = app.download_receiver {
            match rx.try_recv() {
                Ok(message) => {
                    match message {
                        DownloadMessage::Progress(progress) => {
                            app.download_progress = progress;
                        }
                        DownloadMessage::Done(result) => {
                            app.is_downloading = false;
                            app.download_receiver = None;
                            match result {
                                Ok(files) => {
                                    app.download_status = format!("Downloaded {} files successfully", files.len());
                                    app.downloaded_files = files;
                                }
                                Err(e) => {
                                    app.download_status = format!("Error: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    // Still downloading
                }
                Err(TryRecvError::Disconnected) => {
                    app.is_downloading = false;
                    app.download_receiver = None;
                    app.download_status = "Download thread disconnected".to_string();
                }
            }
        }

        ui.add_space(10.0);

        // Progress and Status
        if !app.download_progress.is_empty() {
            ui.label(&app.download_progress);
        }
        if !app.download_status.is_empty() {
            ui.label(&app.download_status);
        }

        // Downloaded Files
        if !app.downloaded_files.is_empty() {
            ui.add_space(10.0);
            ui.label("Downloaded Files:");
            for file in &app.downloaded_files {
                ui.label(format!("• {}", file));
            }
        }

        ui.add_space(20.0);
    });
}
