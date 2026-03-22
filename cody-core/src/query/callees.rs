use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::callees_of;

pub fn cmd_callees(conn: &Connection, symbol: &str) -> Result<()> {
    let edges = callees_of(conn, symbol)?;
    if edges.is_empty() {
        println!("No callees found for: {symbol}");
        return Ok(());
    }
    println!("Functions called by {symbol}:");
    for e in edges {
        println!("  {} (line {})", e.dst_symbol.unwrap_or_default(), e.line.unwrap_or(0));
    }
    Ok(())
}
