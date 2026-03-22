use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use rayon::prelude::*;
use rusqlite::Connection;
use uuid::Uuid;
use crate::db::{models::{BoundaryEvent, EntryPoint, Trace}, store};
use crate::traces::{
    walker::{AdjacencyMap, TraceConfig, build_trace},
    formatter::{serialize_trace, serialize_compact, collect_fn_names, collect_media, min_confidence, count_spans},
    otlp::serialize_otlp,
};

pub fn generate_traces(
    conn: &Connection,
    entry_points: &[EntryPoint],
    config: &TraceConfig,
    include_otlp: bool,
) -> Result<Vec<Trace>> {
    // Load adjacency map into memory once
    let adj = load_adjacency(conn)?;
    let adj = Arc::new(adj);

    // Load boundary events indexed by fn_name
    let all_bounds = store::load_all_boundary_events(conn)?;
    let mut boundary_index: HashMap<String, Vec<BoundaryEvent>> = HashMap::new();
    for be in all_bounds {
        boundary_index.entry(be.fn_name.clone()).or_default().push(be);
    }
    let boundary_index = Arc::new(boundary_index);

    let traces: Vec<Trace> = entry_points.par_iter().map(|ep| {
        let span = build_trace(ep, &adj, &boundary_index, config);
        let text    = serialize_trace(&span);
        let compact = serialize_compact(&span);
        let otlp    = if include_otlp { Some(serialize_otlp(&span).to_string()) } else { None };
        let fn_names  = collect_fn_names(&span);
        let media     = collect_media(&span);
        let min_conf  = min_confidence(&span);
        let span_count = count_spans(&span);
        let value_names: Vec<String> = span.baggage.clone();

        Trace {
            id: None,
            trace_id:       Uuid::new_v4().to_string(),
            root_fn:        ep.fn_name.clone(),
            root_file:      ep.file.clone(),
            service:        crate::traces::span::service_from_path(&ep.file),
            text, compact, otlp,
            span_count,
            fn_names, media, value_names,
            min_confidence: min_conf,
            created_at:     Utc::now().to_rfc3339(),
        }
    }).collect();

    for trace in &traces {
        store::insert_trace(conn, trace)?;
    }

    tracing::info!("Generated {} traces", traces.len());
    Ok(traces)
}

fn load_adjacency(conn: &Connection) -> Result<AdjacencyMap> {
    use std::collections::HashSet;
    let mut adj: AdjacencyMap = HashMap::new();
    // Only include edges where the callee is a known project symbol,
    // filtered to same-language when possible (prefer same file extension).
    // Deduplicate (src, dst) to avoid identical subtrees in traces.
    let mut stmt = conn.prepare(
        "SELECT DISTINCT e.src_symbol, COALESCE(e.src_file,''), e.dst_symbol, COALESCE(s.file,'')
         FROM edges e
         INNER JOIN symbols s ON s.name = e.dst_symbol
         WHERE e.rel IN ('calls','data_flow')
           AND e.src_symbol IS NOT NULL AND e.dst_symbol IS NOT NULL"
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?,
            r.get::<_,String>(2)?, r.get::<_,String>(3)?))
    })?;
    // Deduplicate (src_symbol, dst_symbol) pairs — each logical edge once
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for row in rows {
        let (src, src_file, dst, dst_file) = row?;
        // Prefer same-language callees: if src and dst share file extension, use them;
        // if a cross-language name collision would otherwise dominate, skip it
        let src_ext = src_file.rsplit('.').next().unwrap_or("");
        let dst_ext = dst_file.rsplit('.').next().unwrap_or("");
        let same_lang = src_ext == dst_ext || src_ext.is_empty() || dst_ext.is_empty();
        if !same_lang {
            // Only include cross-language if no same-language callee exists for this (src,dst)
            // We'll do a second pass below for cross-language fallbacks
            continue;
        }
        if seen.insert((src.clone(), dst.clone())) {
            adj.entry((src, src_file)).or_default().push((dst, dst_file, "calls".into(), 0.95));
        }
    }
    // Second pass: add cross-language callees only if (src, dst) not already present
    let mut stmt2 = conn.prepare(
        "SELECT DISTINCT e.src_symbol, COALESCE(e.src_file,''), e.dst_symbol, COALESCE(s.file,'')
         FROM edges e
         INNER JOIN symbols s ON s.name = e.dst_symbol
         WHERE e.rel IN ('calls','data_flow')
           AND e.src_symbol IS NOT NULL AND e.dst_symbol IS NOT NULL"
    )?;
    let rows2 = stmt2.query_map([], |r| {
        Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?,
            r.get::<_,String>(2)?, r.get::<_,String>(3)?))
    })?;
    for row in rows2 {
        let (src, src_file, dst, dst_file) = row?;
        let src_ext = src_file.rsplit('.').next().unwrap_or("");
        let dst_ext = dst_file.rsplit('.').next().unwrap_or("");
        let same_lang = src_ext == dst_ext || src_ext.is_empty() || dst_ext.is_empty();
        if !same_lang && seen.insert((src.clone(), dst.clone())) {
            adj.entry((src, src_file)).or_default().push((dst, dst_file, "calls".into(), 0.80));
        }
    }
    // Load boundary flows as edges
    let mut stmt = conn.prepare(
        "SELECT write_fn, write_file, read_fn, read_file, confidence
         FROM boundary_flows"
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?,
            r.get::<_,String>(2)?, r.get::<_,String>(3)?,
            r.get::<_,f64>(4)?))
    })?;
    for row in rows {
        let (wfn, wf, rfn, rf, conf) = row?;
        adj.entry((wfn, wf)).or_default().push((rfn, rf, "boundary_flow".into(), conf));
    }
    Ok(adj)
}
