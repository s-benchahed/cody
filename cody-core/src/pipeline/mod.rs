pub mod walk;
pub mod hash;
pub mod parse;
pub mod entrypoints;

use anyhow::Result;
use rayon::prelude::*;
use std::time::Instant;
use crate::config::MapConfig;
use crate::extractor;
use crate::plugin::registry::build_registry;
use crate::codemap;
use hash::ChangeStatus;

pub fn run_map(config: &MapConfig) -> Result<()> {
    let registry = build_registry();

    let t0 = Instant::now();
    let entries = walk::collect_files(&config.root_dir, &registry);
    tracing::info!("Found {} files in {:.1}s", entries.len(), t0.elapsed().as_secs_f32());

    let t1 = Instant::now();
    let cache_path = config.root_dir.join(".cody-cache");
    let hashed = hash::hash_files_cached(&entries, &cache_path);
    tracing::info!(
        "Hashed in {:.1}s: {} changed / {} new / {} unchanged",
        t1.elapsed().as_secs_f32(),
        hashed.iter().filter(|h| h.status == ChangeStatus::Changed).count(),
        hashed.iter().filter(|h| h.status == ChangeStatus::New).count(),
        hashed.iter().filter(|h| h.status == ChangeStatus::Unchanged).count(),
    );

    let t2 = Instant::now();
    let parsed = parse::parse_files(&hashed, &registry);
    tracing::info!("Parsed {} files in {:.1}s", parsed.len(), t2.elapsed().as_secs_f32());

    let t3 = Instant::now();
    let mut all_facts: Vec<extractor::ExtractedFacts> = parsed.par_iter()
        .filter_map(|pf| extractor::extract(pf).map_err(|e| {
            tracing::warn!("extract error {}: {e}", pf.hashed.entry.path.display()); e
        }).ok())
        .collect();
    tracing::info!("Extracted {} files in {:.1}s", all_facts.len(), t3.elapsed().as_secs_f32());

    if config.use_lsp {
        // ── Pass 1: boundary event confidence enrichment (hover) ──────────────
        let t_lsp = Instant::now();
        let all_events: Vec<_> = all_facts.iter()
            .flat_map(|f| f.boundary_events.clone())
            .collect();
        let lsp_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(crate::lsp::enrich_boundary_events(all_events, &config.root_dir))
        });
        match lsp_result {
            Ok((enriched, stats)) => {
                tracing::info!(
                    "LSP enrichment in {:.1}s: {} checked, {} confirmed, {} rejected",
                    t_lsp.elapsed().as_secs_f32(),
                    stats.events_checked, stats.events_confirmed, stats.events_rejected,
                );
                let mut enriched_map: std::collections::HashMap<(String, String, String, String), (f64, Option<String>)> =
                    std::collections::HashMap::new();
                for ev in &enriched {
                    enriched_map.insert(
                        (ev.fn_name.clone(), ev.file.clone(), ev.medium.clone(), ev.key_norm.clone()),
                        (ev.prov_confidence, ev.prov_note.clone()),
                    );
                }
                for facts in &mut all_facts {
                    for ev in &mut facts.boundary_events {
                        if let Some((conf, note)) = enriched_map.get(&(
                            ev.fn_name.clone(), ev.file.clone(), ev.medium.clone(), ev.key_norm.clone()
                        )) {
                            ev.prov_confidence = *conf;
                            ev.prov_note = note.clone();
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("LSP enrichment error: {e}"),
        }

        // ── Pass 2: resolve ambiguous call edges (go-to-definition) ───────────
        // Patches edges that static symbol lookup can't resolve because the callee
        // name is defined in multiple files (e.g. wrapper classes, service objects).
        let t_edges = Instant::now();
        let edge_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(crate::lsp::resolve_ambiguous_edges(&mut all_facts, &config.root_dir))
        });
        match edge_result {
            Ok(stats) => tracing::info!(
                "LSP edge resolution in {:.1}s: {}/{} ambiguous edges resolved",
                t_edges.elapsed().as_secs_f32(),
                stats.resolved, stats.ambiguous_checked,
            ),
            Err(e) => tracing::warn!("LSP edge resolution error: {e}"),
        }
    }

    let entry_points = entrypoints::detect(&all_facts, config.min_confidence);
    tracing::info!("Detected {} entry points", entry_points.len());

    let codemap = codemap::build(&all_facts, &entry_points, config);
    let content = codemap::writer::write(&codemap);
    std::fs::write(&config.out_path, &content)?;

    hash::save_cache(&hashed, &cache_path);

    let line_count = content.lines().count();
    tracing::info!("Wrote {} ({} lines)", config.out_path, line_count);

    println!("\n=== Codemap Summary ===");
    println!("  output:    {}", config.out_path);
    println!("  files:     {}", codemap.file_count);
    println!("  languages: {}", codemap.languages.join(", "));
    println!("  services:  {}", codemap.services.len());
    println!("  topology:  {} cross-service flows", codemap.topology.len());
    println!("  lines:     {}", line_count);

    Ok(())
}
