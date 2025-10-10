use crate::app::{IndistocksApp, TimeRange};
use chrono::{Datelike, Duration, NaiveDate};


pub fn render(ui: &mut egui::Ui, app: &mut IndistocksApp) {
    if let Some(symbol) = &app.selected_symbol.clone() {
        ui.heading(format!("Historical Data for {}", symbol));
        ui.add_space(10.0);

        if app.plot_data.is_empty() {
            ui.label("No downloaded data available for this symbol.");
        } else {
            // Calculate date range
            let (min_date, max_date) = app.plot_data.iter().fold(
                (app.plot_data[0].0, app.plot_data[0].0),
                |(min, max), (date, _)| {
                    (min.min(*date), max.max(*date))
                }
            );
            let days_diff = (max_date - min_date).num_days();

            // Determine formatting based on date range
            let (x_fmt, should_filter_ticks) = get_date_format_and_filter(days_diff);
            let x_fmt_clone = x_fmt.clone();

            // Plot the data - use symbol and time range in ID to reset view when switching stocks or time ranges
            let mut plot = egui_plot::Plot::new(format!("price_plot_{}_{}", symbol, app.selected_time_range.label()))
                .height(600.0)
                .legend(egui_plot::Legend::default())
                .allow_zoom([true, false])  // Allow horizontal zoom only
                .allow_drag([true, false])  // Allow horizontal drag only
                .allow_scroll([true, false])  // Allow horizontal scroll for zooming only
                .x_axis_formatter(move |mark, _range| {
                    format_timestamp_to_date(mark.value, &x_fmt)
                })
                .label_formatter(move |_name, value| {
                    format!("Date: {}\nPrice: {:.2}",
                        format_timestamp_to_date(value.x, &x_fmt_clone),
                        value.y)
                });

            // Reset plot view if needed (when changing time range or loading new stock)
            if app.plot_needs_reset {
                plot = plot.reset();
                app.plot_needs_reset = false;
            }

            let response = plot.show(ui, |plot_ui| {
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

                // Add custom X-axis ticks if needed
                if should_filter_ticks {
                    add_custom_x_ticks(plot_ui, &app.plot_data, days_diff);
                }
            });

            // Only check for loading more data if user is actively interacting with the plot
            // This prevents automatic cascading loads when the plot first renders
            if response.response.dragged() || (response.response.hovered() && ui.input(|i| i.raw_scroll_delta.x != 0.0)) {
                let plot_bounds = response.transform;
                let plot_bounds_range = plot_bounds.bounds();

                // Get the visible X range (timestamps)
                let view_start_ts = plot_bounds_range.min()[0];
                let view_end_ts = plot_bounds_range.max()[0];

                // Get the earliest and latest loaded data timestamps
                if let (Some((earliest_date, _)), Some((latest_date, _))) =
                    (app.plot_data.first(), app.plot_data.last()) {

                    let earliest_ts = earliest_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;
                    let _latest_ts = latest_date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;

                    // Calculate visible range in days
                    let visible_range_days = (view_end_ts - view_start_ts) / (24.0 * 3600.0);

                    // If we're viewing within 20% of the visible range from the earliest loaded data, load more
                    let threshold = visible_range_days * 0.2 * 24.0 * 3600.0; // 20% of visible range in seconds

                    // Only attempt to load if:
                    // 1. We're viewing near the earliest loaded data
                    // 2. We're not already loading
                    // 3. We haven't reached the earliest available data
                    if view_start_ts < (earliest_ts + threshold) && !app.plot_loading_in_progress {
                        if let Some(earliest_available) = app.plot_earliest_available {
                            if earliest_date > &earliest_available {
                                println!("Loading earlier data: view_start={}, earliest={}, threshold={}",
                                    view_start_ts, earliest_ts, threshold);
                                // Load 90 more days of data
                                app.load_earlier_data(symbol, 90);
                            }
                        }
                    }
                }
            }
        }

        // Horizontal layout for Back button and time range buttons
        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                app.selected_symbol = None;
                app.plot_data.clear();
            }

            // Add spacing to push time range buttons to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Time range buttons (in reverse order because of right_to_left layout)
                let time_ranges = [
                    TimeRange::All,
                    TimeRange::FiveYears,
                    TimeRange::OneYear,
                    TimeRange::SixMonths,
                    TimeRange::ThreeMonths,
                    TimeRange::OneMonth,
                    TimeRange::FiveDays,
                ];

                for time_range in time_ranges.iter().rev() {
                    let is_selected = app.selected_time_range == *time_range;
                    let button = if is_selected {
                        egui::Button::new(time_range.label()).fill(ui.style().visuals.selection.bg_fill)
                    } else {
                        egui::Button::new(time_range.label())
                    };

                    if ui.add(button).clicked() {
                        app.change_time_range(*time_range);
                    }
                }
            });
        });
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

/// Determine the appropriate date format based on the time range
fn get_date_format_and_filter(days_diff: i64) -> (String, bool) {
    if days_diff <= 90 {
        // 3 months or less: Show DD/MM format
        ("%d/%m".to_string(), false)
    } else if days_diff <= 365 {
        // More than 3 months, up to 1 year: Show 01 and 15 of each month
        ("%d/%m".to_string(), true)
    } else {
        // More than 1 year: Show Month/Year
        ("%b/%Y".to_string(), true)
    }
}

/// Format a Unix timestamp to a date string
fn format_timestamp_to_date(timestamp: f64, format: &str) -> String {
    let dt = chrono::DateTime::from_timestamp(timestamp as i64, 0);
    if let Some(dt) = dt {
        dt.format(format).to_string()
    } else {
        timestamp.to_string()
    }
}

/// Add custom X-axis tick marks based on the date range
fn add_custom_x_ticks(_plot_ui: &mut egui_plot::PlotUi, data: &[(NaiveDate, f64)], days_diff: i64) {
    if data.is_empty() {
        return;
    }

    let min_date = data.iter().map(|(d, _)| *d).min().unwrap();
    let max_date = data.iter().map(|(d, _)| *d).max().unwrap();

    let mut tick_dates = Vec::new();

    if days_diff > 90 && days_diff <= 365 {
        // Show 1st and 15th of each month
        let mut current = min_date;
        while current <= max_date {
            // Add 1st of month
            let first = NaiveDate::from_ymd_opt(current.year(), current.month(), 1);
            if let Some(d) = first {
                if d >= min_date && d <= max_date {
                    tick_dates.push(d);
                }
            }

            // Add 15th of month
            let fifteenth = NaiveDate::from_ymd_opt(current.year(), current.month(), 15);
            if let Some(d) = fifteenth {
                if d >= min_date && d <= max_date {
                    tick_dates.push(d);
                }
            }

            // Move to next month
            if let Some(next_month) = current.checked_add_signed(Duration::days(32)) {
                current = NaiveDate::from_ymd_opt(next_month.year(), next_month.month(), 1).unwrap();
            } else {
                break;
            }
        }
    } else if days_diff > 365 {
        // Show 1st of each month for data over 1 year
        let mut current = min_date;
        while current <= max_date {
            let first = NaiveDate::from_ymd_opt(current.year(), current.month(), 1);
            if let Some(d) = first {
                if d >= min_date && d <= max_date {
                    tick_dates.push(d);
                }
            }

            // Move to next month
            if let Some(next_month) = current.checked_add_signed(Duration::days(32)) {
                current = NaiveDate::from_ymd_opt(next_month.year(), next_month.month(), 1).unwrap();
            } else {
                break;
            }
        }
    }

    // Convert tick dates to timestamps and add to plot
    for date in tick_dates {
        let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as f64;
        // Note: egui_plot doesn't have direct API to set ticks, the formatter will handle display
        // This function is prepared for future use if custom tick API becomes available
        let _ = timestamp; // Suppress unused warning
    }
}
