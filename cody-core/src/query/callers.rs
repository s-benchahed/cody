use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::callers_of;

pub fn cmd_callers(conn: &Connection, symbol: &str) -> Result<()> {
    let edges = callers_of(conn, symbol)?;
    if edges.is_empty() {
        println!("No callers found for: {symbol}");
        return Ok(());
    }
    println!("Callers of {symbol}:");
    for e in edges {
        println!("  {} ({}:{})", e.src_symbol.unwrap_or_default(),
            e.src_file.unwrap_or_default(), e.line.unwrap_or(0));
    }
    Ok(())
}
