pub mod db;
pub mod models;

pub use db::*;
pub use db::downloads::{download_bhavcopy_with_limit, download_bhavcopy_with_date_range};
pub use db::operations::{StockData, get_all_stocks_with_metrics, get_stock_data_in_range};

// Re-export rusqlite types
pub use rusqlite::{Connection, Result};

#[derive(Debug)]
pub enum BhavCopyMessage {
    Progress(String),
    DateRangeUpdated(chrono::NaiveDate, chrono::NaiveDate),
    Done(Result<(), String>),
}
