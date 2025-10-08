use rusqlite::{Connection, Result, params};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct NseSymbol {
    pub id: i64,
    pub symbol: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecentlyViewed {
    pub symbol: String,
    pub name: Option<String>,
}

pub fn save_nse_symbols(conn: &Connection, symbols: Vec<String>) -> Result<(usize, Vec<String>)> {
    let now = Utc::now().timestamp();
    let mut saved_count = 0;
    let mut errors = Vec::new();

    for symbol in symbols {
        let trimmed = symbol.trim().to_uppercase();

        // Validate symbol format (alphanumeric and underscore only)
        if !trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') || trimmed.is_empty() {
            errors.push(trimmed);
            continue;
        }

        match conn.execute(
            "INSERT INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?2)
             ON CONFLICT(symbol) DO UPDATE SET updated_at = ?2",
            params![trimmed, now],
        ) {
            Ok(_) => saved_count += 1,
            Err(_) => errors.push(trimmed),
        }
    }

    Ok((saved_count, errors))
}

pub fn get_recently_viewed(conn: &Connection, limit: usize) -> Result<Vec<RecentlyViewed>> {
    let mut stmt = conn.prepare(
        "SELECT ns.symbol, ns.name
         FROM recently_viewed rv
         JOIN nse_symbols ns ON rv.symbol_id = ns.id
         ORDER BY rv.viewed_at DESC
         LIMIT ?1"
    )?;

    let items = stmt.query_map(params![limit], |row| {
        Ok(RecentlyViewed {
            symbol: row.get(0)?,
            name: row.get(1)?,
        })
    })?;

    items.collect()
}

// For demo purposes, populate some random recently viewed items
pub fn populate_demo_data(conn: &Connection) -> Result<()> {
    let now = Utc::now().timestamp();

    // Add some demo symbols
    let demo_symbols = vec![
        "RELIANCE", "TCS", "HDFCBANK", "INFY", "ICICIBANK",
        "HINDUNILVR", "ITC", "SBIN", "BHARTIARTL", "KOTAKBANK",
        "LT", "AXISBANK", "ASIANPAINT", "MARUTI", "TITAN",
        "SUNPHARMA", "BAJFINANCE", "HCLTECH", "WIPRO", "ULTRACEMCO"
    ];

    for symbol in &demo_symbols {
        conn.execute(
            "INSERT OR IGNORE INTO nse_symbols (symbol, name, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?2)",
            params![symbol, now],
        )?;
    }

    // Add some to recently viewed
    for (i, symbol) in demo_symbols.iter().take(10).enumerate() {
        let symbol_id: i64 = conn.query_row(
            "SELECT id FROM nse_symbols WHERE symbol = ?1",
            params![symbol],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO recently_viewed (symbol_id, viewed_at)
             VALUES (?1, ?2)",
            params![symbol_id, now - (i as i64 * 3600)],
        )?;
    }

    Ok(())
}
