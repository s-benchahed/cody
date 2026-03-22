use anyhow::Result;
use rusqlite::Connection;
use crate::db::{models::FileMeta, store};
use crate::extractor::ExtractedFacts;

pub fn ingest_facts(conn: &Connection, facts: &ExtractedFacts) -> Result<()> {
    // Wrap per-file ingest in a transaction for performance
    conn.execute("BEGIN", [])?;

    store::upsert_file_meta(conn, &FileMeta {
        file:     facts.file.clone(),
        language: facts.language.clone(),
        lines:    facts.meta.line_count,
        exports:  facts.meta.export_count,
        imports:  facts.meta.import_count,
        hash:     facts.sha256.clone(),
    })?;

    // Delete stale data for this file before re-inserting
    conn.execute("DELETE FROM symbols WHERE file = ?1", [&facts.file])?;
    conn.execute("DELETE FROM edges WHERE src_file = ?1", [&facts.file])?;
    conn.execute("DELETE FROM boundary_events WHERE file = ?1", [&facts.file])?;

    store::insert_symbols(conn, &facts.symbols)?;
    store::insert_edges(conn, &facts.edges)?;
    store::insert_boundary_events(conn, &facts.boundary_events)?;

    conn.execute("COMMIT", [])?;
    Ok(())
}
