use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NseDownload {
    pub id: i64,
    pub symbol: Option<String>,
    pub from_date: i64,
    pub to_date: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub status: String,
    pub error_message: Option<String>,
    pub downloaded_at: i64,
}
