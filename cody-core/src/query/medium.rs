use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::boundaries_for_medium;

pub fn cmd_medium(conn: &Connection, medium: &str) -> Result<()> {
    let events = boundaries_for_medium(conn, medium)?;
    if events.is_empty() {
        println!("No boundary events for medium: {medium}");
        return Ok(());
    }
    println!("All {} events for medium '{medium}':", events.len());
    for e in events {
        println!("  [{:>5}] {:30} \"{}\"  {}:{}",
            e.direction, e.fn_name, e.key_raw,
            e.file, e.line.unwrap_or(0));
    }
    Ok(())
}
