use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;
use crate::db::models::{BoundaryEvent, EntryPoint};
use crate::traces::span::{EdgeKind, Span, service_from_path};
use crate::traces::baggage::BaggageMap;

/// In-memory adjacency map: (symbol_name, file) → [(dst_symbol, dst_file, edge_kind, confidence)]
/// Keyed by (name, file) so symbols with the same name in different files are distinct.
pub type AdjacencyMap = HashMap<(String, String), Vec<(String, String, String, f64)>>;

pub struct TraceConfig {
    pub max_depth:      usize,
    pub max_tokens:     usize,
    pub min_confidence: f64,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self { max_depth: 6, max_tokens: 2000, min_confidence: 0.5 }
    }
}

pub fn build_trace(
    entry: &EntryPoint,
    adj: &AdjacencyMap,
    boundary_index: &HashMap<String, Vec<BoundaryEvent>>,
    config: &TraceConfig,
) -> Span {
    let trace_id = Uuid::new_v4().to_string();
    let mut baggage = BaggageMap::new();
    let mut visited: HashSet<String> = HashSet::new();

    build_span(
        &entry.fn_name,
        &entry.file,
        &trace_id,
        None,
        EdgeKind::Root,
        0,
        adj,
        boundary_index,
        config,
        &mut visited,
        &mut baggage,
    )
}

fn build_span(
    fn_name: &str,
    file: &str,
    trace_id: &str,
    parent_id: Option<String>,
    edge_kind: EdgeKind,
    depth: usize,
    adj: &AdjacencyMap,
    boundary_index: &HashMap<String, Vec<BoundaryEvent>>,
    config: &TraceConfig,
    visited: &mut HashSet<String>,
    baggage: &mut BaggageMap,
) -> Span {
    let id = Uuid::new_v4().to_string();
    let service = service_from_path(file);

    // Boundary events for this function
    let bounds = boundary_index.get(fn_name).cloned().unwrap_or_default();
    let boundary_in: Vec<BoundaryEvent>  = bounds.iter().filter(|e| e.direction == "read").cloned().collect();
    let boundary_out: Vec<BoundaryEvent> = bounds.iter().filter(|e| e.direction == "write").cloned().collect();

    // Assign baggage names for boundary events
    let mut span_baggage = Vec::new();
    for be in bounds.iter() {
        let bname = baggage.get_or_assign(&format!("{}:{}", be.medium, be.key_norm));
        if !span_baggage.contains(&bname) { span_baggage.push(bname); }
    }

    let mut span = Span {
        id: id.clone(),
        trace_id: trace_id.to_string(),
        parent_id,
        fn_name: fn_name.to_string(),
        service,
        file: file.to_string(),
        line: None,
        edge_kind,
        depth,
        reads:        vec![],
        writes:       vec![],
        boundary_in,
        boundary_out,
        baggage: span_baggage,
        confidence: 1.0,
        children:  vec![],
        truncated: None,
    };

    if depth >= config.max_depth {
        return span;
    }

    // Prevent cycles
    visited.insert(fn_name.to_string());

    let children_raw = adj.get(&(fn_name.to_string(), file.to_string())).cloned().unwrap_or_default();
    let mut children: Vec<Span> = Vec::new();
    let mut omitted = 0usize;

    // Simple token budget: estimate current serialised size
    let estimated_tokens = estimate_tokens(&span);
    let mut token_budget = if estimated_tokens < config.max_tokens {
        config.max_tokens - estimated_tokens
    } else {
        0
    };

    for (dst_fn, dst_file, rel, confidence) in children_raw {
        if confidence < config.min_confidence { continue; }
        if visited.contains(&dst_fn) { continue; }
        if token_budget == 0 {
            omitted += 1;
            continue;
        }
        let ek = if rel == "data_flow" { EdgeKind::DataFlow }
                 else if rel == "boundary_flow" { EdgeKind::BoundaryFlow }
                 else { EdgeKind::Call };
        let child = build_span(
            &dst_fn, &dst_file, trace_id, Some(id.clone()),
            ek, depth + 1, adj, boundary_index, config,
            &mut visited.clone(), baggage,
        );
        let child_tokens = estimate_tokens(&child);
        token_budget = token_budget.saturating_sub(child_tokens * 4);
        children.push(child);
    }

    visited.remove(fn_name);

    if omitted > 0 { span.truncated = Some(omitted); }
    span.children = children;
    span
}

fn estimate_tokens(span: &Span) -> usize {
    // Rough estimate: 1 token per 4 chars of fn_name + file + boundaries
    let base = span.fn_name.len() + span.file.len() + 20;
    let bounds = (span.boundary_in.len() + span.boundary_out.len()) * 30;
    (base + bounds) / 4
}
