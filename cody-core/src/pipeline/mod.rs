pub mod walk;
pub mod hash;
pub mod parse;
pub mod ingest;
pub mod entrypoints;
pub mod traces;

use anyhow::Result;
use rayon::prelude::*;
use std::time::Instant;
use crate::config::IndexConfig;
use crate::db;
use crate::extractor;
use crate::patterns;
use crate::traces::walker::TraceConfig;
use hash::ChangeStatus;

pub fn run_index(config: &IndexConfig) -> Result<()> {
    let conn = db::open(&config.db_path)?;
    let registry = crate::plugin::registry::build_registry();

    // ── Step 1: Walk ──────────────────────────────────────────────────────
    let t0 = Instant::now();
    let entries = walk::collect_files(&config.root_dir, &registry);
    tracing::info!("Found {} files in {:.1}s", entries.len(), t0.elapsed().as_secs_f32());

    // ── Step 2: Hash check ────────────────────────────────────────────────
    let t1 = Instant::now();
    let hashed = hash::hash_files(&entries, &conn);
    let changed: Vec<_> = hashed.iter()
        .filter(|h| h.status != ChangeStatus::Unchanged)
        .collect();
    tracing::info!(
        "Hashed in {:.1}s: {} changed / {} new / {} unchanged",
        t1.elapsed().as_secs_f32(),
        changed.iter().filter(|h| h.status == ChangeStatus::Changed).count(),
        changed.iter().filter(|h| h.status == ChangeStatus::New).count(),
        hashed.iter().filter(|h| h.status == ChangeStatus::Unchanged).count(),
    );

    if changed.is_empty() {
        tracing::info!("Nothing changed. Index up to date.");
        print_stats(&conn);
        return Ok(());
    }

    let changed_owned: Vec<_> = changed.iter().map(|h| (*h).clone()).collect();

    // ── Step 3: Parse ─────────────────────────────────────────────────────
    let t2 = Instant::now();
    let parsed = parse::parse_files(&changed_owned, &registry);
    tracing::info!("Parsed {} files in {:.1}s", parsed.len(), t2.elapsed().as_secs_f32());

    // ── Step 4: Extract (parallel) ────────────────────────────────────────
    let t3 = Instant::now();
    let facts: Vec<extractor::ExtractedFacts> = parsed.par_iter()
        .filter_map(|pf| {
            extractor::extract(pf).map_err(|e| {
                tracing::warn!("extract error {}: {e}", pf.hashed.entry.path.display());
                e
            }).ok()
        }).collect();
    tracing::info!("Extracted {} files in {:.1}s", facts.len(), t3.elapsed().as_secs_f32());

    // ── Step 5: Ingest (sequential) ───────────────────────────────────────
    let t4 = Instant::now();
    for f in &facts {
        ingest::ingest_facts(&conn, f)?;
    }
    tracing::info!("Ingested in {:.1}s", t4.elapsed().as_secs_f32());

    // ── Step 5b: Optional LSP enrichment ─────────────────────────────────
    if config.use_lsp {
        let t_lsp = Instant::now();
        let lsp_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(crate::lsp::enrich_boundary_events(&conn, &config.root_dir))
        });
        match lsp_result {
            Ok(s) => tracing::info!(
                "LSP enrichment in {:.1}s: {} checked, {} confirmed, {} rejected",
                t_lsp.elapsed().as_secs_f32(),
                s.events_checked, s.events_confirmed, s.events_rejected,
            ),
            Err(e) => tracing::warn!("LSP enrichment error: {e}"),
        }
    }

    // ── Step 5c: Stitch boundary flows ────────────────────────────────────
    let all_bounds = db::store::load_all_boundary_events(&conn)?;
    let flows = patterns::stitch_boundary_events(&all_bounds, config.min_confidence);
    conn.execute("DELETE FROM boundary_flows", [])?;
    db::store::insert_boundary_flows(&conn, &flows)?;
    tracing::info!("Stitched {} boundary flows", flows.len());

    // ── Step 6: Entry point detection ────────────────────────────────────
    let t5 = Instant::now();
    conn.execute("DELETE FROM entry_points", [])?;
    let entry_points = entrypoints::detect(&conn, &facts, config.min_confidence)?;
    tracing::info!(
        "Entry points detected in {:.1}s: {} total",
        t5.elapsed().as_secs_f32(),
        entry_points.len()
    );

    // ── Step 7: Trace generation (parallel) ──────────────────────────────
    let t6 = Instant::now();
    let trace_config = TraceConfig {
        max_depth:      config.max_depth,
        max_tokens:     2000,
        min_confidence: config.min_confidence,
    };
    conn.execute("DELETE FROM traces", [])?;
    let trace_eps = if config.all_entrypoints {
        entry_points.clone()
    } else {
        entry_points.iter().filter(|e| e.confidence >= config.min_confidence).cloned().collect()
    };
    traces::generate_traces(&conn, &trace_eps, &trace_config, false)?;
    tracing::info!("Traces generated in {:.1}s", t6.elapsed().as_secs_f32());

    print_stats(&conn);
    db::store::checkpoint(&conn, "pipeline", "run", "done", Some("complete"))?;
    Ok(())
}

fn print_stats(conn: &rusqlite::Connection) {
    if let Ok(s) = db::store::stats(conn) {
        println!("\n=== Index Summary ===");
        println!("  files:           {}", s["files"]);
        println!("  symbols:         {}", s["symbols"]);
        println!("  edges:           {}", s["edges"]);
        println!("  boundary_events: {}", s["boundary_events"]);
        println!("  entry_points:    {}", s["entry_points"]);
        println!("  traces:          {}", s["traces"]);
    }
}
