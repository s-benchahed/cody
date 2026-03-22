use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::deps_of_file;

pub fn cmd_deps(conn: &Connection, file: &str) -> Result<()> {
    let edges = deps_of_file(conn, file)?;
    if edges.is_empty() {
        println!("No imports found for: {file}");
        return Ok(());
    }
    println!("Imports in {file}:");
    for e in edges {
        println!("  → {}", e.dst_file.unwrap_or_default());
    }
    Ok(())
}
