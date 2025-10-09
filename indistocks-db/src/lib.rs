pub mod db;
pub mod models;

pub use db::*;

// Re-export rusqlite types
pub use rusqlite::{Connection, Result};

#[derive(Debug)]
pub enum BhavCopyMessage {
    Progress(String),
    Done(Result<(), String>),
}
