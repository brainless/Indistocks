use crate::app::{IndistocksApp, View};
use indistocks_db::{save_nse_symbols_with_names, download_bhavcopy, get_bhavcopy_date_range, clear_bhavcopy_data, BhavCopyMessage};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;



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
    // Refresh BhavCopy date range only once when Settings view is opened (if not already set)
    if app.bhavcopy_date_range.is_none() && !app.is_downloading_bhavcopy {
        app.bhavcopy_date_range = get_bhavcopy_date_range(&*app.db_conn.lock().unwrap()).unwrap_or(None);
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(20.0);

        // Header with back/close navigation
        ui.horizontal(|ui| {
            ui.heading("Settings");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").on_hover_text("Close").clicked() {
                    app.current_view = View::Home;
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
                             let result = save_nse_symbols_with_names(&*app.db_conn.lock().unwrap(), symbols);
                             match result {
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

        ui.horizontal(|ui| {
            // Download BhavCopy button
            if ui.button("Download BhavCopy").clicked() && !app.is_downloading_bhavcopy {
                app.is_downloading_bhavcopy = true;
                app.bhavcopy_progress = "Starting download...".to_string();
                app.bhavcopy_status = String::new();

                let (tx, rx) = mpsc::channel();
                app.bhavcopy_receiver = Some(rx);

                let db_conn = app.db_conn.clone();
                thread::spawn(move || {
                    let result = download_bhavcopy(&db_conn, &tx);
                    let _ = tx.send(BhavCopyMessage::Done(result.map_err(|e| e.to_string())));
                });
            }

            // Clear BhavCopy data button
            if ui.button("Clear BhavCopy Data").clicked() && !app.is_downloading_bhavcopy {
                match clear_bhavcopy_data(&*app.db_conn.lock().unwrap()) {
                    Ok(()) => {
                        app.bhavcopy_status = "BhavCopy data cleared successfully".to_string();
                        app.bhavcopy_date_range = None;
                    }
                    Err(e) => {
                        app.bhavcopy_status = format!("Error clearing data: {}", e);
                    }
                }
            }
        });

        ui.add_space(10.0);

        // Show date range of existing BhavCopy downloads
        ui.label("Existing BhavCopy Downloads:");
        match app.bhavcopy_date_range {
            Some((start, end)) => {
                ui.label(format!("From {} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")));
            }
            _ => {
                ui.label("No downloads yet");
            }
        }

        // Check for bhavcopy messages - process all available messages
        if let Some(ref rx) = app.bhavcopy_receiver {
            loop {
                match rx.try_recv() {
                    Ok(message) => {
                        match message {
                            BhavCopyMessage::Progress(progress) => {
                                app.bhavcopy_progress = progress;
                            }
                            BhavCopyMessage::DateRangeUpdated(min_date, max_date) => {
                                app.bhavcopy_date_range = Some((min_date, max_date));
                            }
                            BhavCopyMessage::Done(result) => {
                                app.is_downloading_bhavcopy = false;
                                app.bhavcopy_receiver = None;
                                match result {
                                    Ok(()) => {
                                        app.bhavcopy_status = "BhavCopy download completed successfully".to_string();
                                        // Update date range
                                        app.bhavcopy_date_range = get_bhavcopy_date_range(&*app.db_conn.lock().unwrap()).unwrap_or(None);
                                    }
                                    Err(e) => {
                                        app.bhavcopy_status = format!("Error: {}", e);
                                    }
                                }
                                break;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        // No more messages
                        break;
                    }
                    Err(TryRecvError::Disconnected) => {
                        app.is_downloading_bhavcopy = false;
                        app.bhavcopy_receiver = None;
                        app.bhavcopy_status = "Download thread disconnected".to_string();
                        break;
                    }
                }
            }
        }

        ui.add_space(10.0);

        // Progress and Status
        if !app.bhavcopy_progress.is_empty() {
            ui.label(&app.bhavcopy_progress);
        }
        if !app.bhavcopy_status.is_empty() {
            ui.label(&app.bhavcopy_status);
        }

        ui.add_space(20.0);
    });
}
