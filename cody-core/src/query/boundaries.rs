use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::boundaries_for_fn;

pub fn cmd_boundaries(conn: &Connection, fn_name: &str) -> Result<()> {
    let events = boundaries_for_fn(conn, fn_name)?;
    if events.is_empty() {
        println!("No boundary events for: {fn_name}");
        return Ok(());
    }
    println!("Boundary events for {fn_name}:");
    for e in events {
        println!("  [{:>5}] {:12} {:12} \"{}\"  (line {}, conf={:.2})",
            e.direction, e.medium, e.prov_source,
            e.key_raw, e.line.unwrap_or(0), e.prov_confidence);
    }
    Ok(())
}
