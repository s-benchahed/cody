use anyhow::Result;
use rusqlite::Connection;
use crate::db::store::lookup_symbol;

pub fn cmd_lookup(conn: &Connection, name: &str) -> Result<()> {
    let syms = lookup_symbol(conn, name)?;
    if syms.is_empty() {
        println!("No symbol found: {name}");
        return Ok(());
    }
    let is_fuzzy = syms.iter().any(|s| s.name != name);
    if is_fuzzy {
        println!("No exact match for '{name}' — fuzzy results:");
    }
    for s in syms {
        println!("{} {} {}:{} [exported={}] [conf={:.2}]",
            s.kind, s.name, s.file, s.line.unwrap_or(0), s.is_exported, s.prov_confidence);
        if let Some(sig) = s.signature { println!("  sig: {sig}"); }
    }
    Ok(())
}
