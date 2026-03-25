use std::collections::{HashMap, HashSet};
use crate::db::models::BoundaryEvent;
use crate::extractor::ExtractedFacts;

pub type AdjacencyMap = HashMap<(String, String), Vec<(String, String)>>;

pub fn build_adjacency(all_facts: &[ExtractedFacts]) -> AdjacencyMap {
    // Build a symbol → file(s) lookup for resolving calls without known dst_file.
    // Use HashSet per symbol to deduplicate — same function can be detected by multiple
    // SCM patterns (e.g. both @fn and @method match impl-block functions).
    let mut symbol_files_set: HashMap<String, HashSet<String>> = HashMap::new();
    for facts in all_facts {
        for sym in &facts.symbols {
            if sym.kind == "function" {
                symbol_files_set.entry(sym.name.clone()).or_default().insert(facts.file.clone());
            }
        }
    }
    let symbol_files: HashMap<String, Vec<String>> = symbol_files_set.into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();

    let mut adj: AdjacencyMap = HashMap::new();
    for facts in all_facts {
        for edge in &facts.edges {
            if edge.rel != "calls" { continue; }
            let src_fn   = match &edge.src_symbol { Some(s) => s.clone(), None => continue };
            let src_file = edge.src_file.clone().unwrap_or_else(|| facts.file.clone());
            let dst_fn   = match &edge.dst_symbol { Some(s) => s.clone(), None => continue };

            if let Some(dst_file) = &edge.dst_file {
                // Direct edge with known destination file
                adj.entry((src_fn, src_file))
                    .or_default()
                    .push((dst_fn, dst_file.clone()));
            } else {
                // No dst_file: resolve by symbol lookup (best effort)
                // Only follow if the symbol is defined in exactly one file to avoid explosion
                if let Some(files) = symbol_files.get(&dst_fn) {
                    let targets: Vec<_> = files.iter()
                        .filter(|f| **f != src_file) // don't self-loop within same file... actually allow same file
                        .cloned().collect();
                    // If multiple files define this symbol, skip (too ambiguous)
                    if targets.len() == 1 {
                        adj.entry((src_fn, src_file))
                            .or_default()
                            .push((dst_fn, targets[0].clone()));
                    }
                }
            }
        }
    }
    adj
}

pub fn collect_io(
    fn_name: &str,
    file: &str,
    adj: &AdjacencyMap,
    boundary_index: &HashMap<String, Vec<BoundaryEvent>>,
    max_depth: usize,
) -> Vec<BoundaryEvent> {
    let mut result: Vec<BoundaryEvent> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    dfs(fn_name, file, adj, boundary_index, max_depth, 0, &mut visited, &mut result);
    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    result.retain(|e| seen.insert((e.medium.clone(), e.key_norm.clone(), e.direction.clone())));
    result
}

fn dfs(
    fn_name: &str, file: &str,
    adj: &AdjacencyMap,
    boundary_index: &HashMap<String, Vec<BoundaryEvent>>,
    max_depth: usize, depth: usize,
    visited: &mut HashSet<String>,
    result: &mut Vec<BoundaryEvent>,
) {
    if depth > max_depth || visited.contains(fn_name) { return; }
    visited.insert(fn_name.to_string());
    if let Some(evs) = boundary_index.get(fn_name) {
        result.extend(evs.iter().cloned());
    }
    if let Some(children) = adj.get(&(fn_name.to_string(), file.to_string())) {
        for (child_fn, child_file) in children {
            dfs(child_fn, child_file, adj, boundary_index, max_depth, depth + 1, visited, result);
        }
    }
}
