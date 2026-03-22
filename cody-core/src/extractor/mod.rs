pub mod regex_boundaries;

use std::collections::HashMap;
use crate::db::models::*;
use crate::pipeline::parse::ParsedFile;

pub struct ExtractedFacts {
    pub file:            String,
    pub language:        String,
    pub sha256:          String,
    pub symbols:         Vec<Symbol>,
    pub edges:           Vec<Edge>,
    pub boundary_events: Vec<BoundaryEvent>,
    pub entry_hints:     Vec<EntryPointHint>,
    pub meta:            FileMetaCounts,
}

pub fn extract(pf: &ParsedFile) -> anyhow::Result<ExtractedFacts> {
    let file = pf.hashed.entry.path.as_path();
    let source = &pf.hashed.source;

    let symbols          = pf.plugin.extract_symbols(&pf.tree, source, file)?;
    let mut edges        = pf.plugin.extract_edges(&pf.tree, source, file)?;
    let ast_bounds       = pf.plugin.extract_boundary_events(&pf.tree, source, file)?;
    let entry_hints      = pf.plugin.entry_point_hints(&pf.tree, source, file)?;
    let meta             = pf.plugin.file_meta_counts(&pf.tree, source)?;

    // Attribute call edges to their enclosing function (src_symbol)
    attribute_src_symbols(&symbols, &mut edges);

    // Layer 2: regex boundary detection on top of AST results
    let regex_bounds = regex_boundaries::extract(
        source,
        file,
        pf.plugin.language_name(),
        &symbols,
    );

    // Merge: deduplicate by (file, line, medium) keeping higher confidence
    let mut combined: Vec<BoundaryEvent> = ast_bounds;
    for rb in regex_bounds {
        let duplicate = combined.iter().any(|existing| {
            existing.line == rb.line && existing.medium == rb.medium
        });
        if !duplicate {
            combined.push(rb);
        }
    }

    attribute_fn_names(&symbols, &mut combined);

    Ok(ExtractedFacts {
        file:            file.to_string_lossy().to_string(),
        language:        pf.plugin.language_name().to_string(),
        sha256:          pf.hashed.sha256.clone(),
        symbols,
        edges,
        boundary_events: combined,
        entry_hints,
        meta,
    })
}

/// Attribute boundary events to their enclosing function using line numbers.
fn attribute_fn_names(symbols: &[Symbol], events: &mut Vec<BoundaryEvent>) {
    use std::collections::HashMap;
    let mut file_fns: HashMap<&str, Vec<(i64, &str)>> = HashMap::new();
    for s in symbols {
        if let Some(line) = s.line {
            if s.kind == "function" {
                file_fns.entry(s.file.as_str()).or_default().push((line, s.name.as_str()));
            }
        }
    }
    for syms in file_fns.values_mut() {
        syms.sort_unstable_by_key(|(l, _)| *l);
    }
    for event in events.iter_mut() {
        let event_line = match event.line { Some(l) => l, None => continue };
        let syms = match file_fns.get(event.file.as_str()) {
            Some(s) => s,
            None => continue,
        };
        if let Some((_, name)) = syms.iter().rev().find(|(sl, _)| *sl <= event_line) {
            event.fn_name = (*name).to_string();
        }
    }
}

/// For each call edge with a line number, find the closest preceding function
/// symbol in the same file and set it as src_symbol.
fn attribute_src_symbols(symbols: &[Symbol], edges: &mut Vec<Edge>) {
    // Build a per-file sorted list of (start_line, symbol_name)
    let mut file_fns: HashMap<&str, Vec<(i64, &str)>> = HashMap::new();
    for s in symbols {
        if let Some(line) = s.line {
            if matches!(s.kind.as_str(), "function" | "class") {
                file_fns.entry(s.file.as_str()).or_default().push((line, s.name.as_str()));
            }
        }
    }
    for syms in file_fns.values_mut() {
        syms.sort_unstable_by_key(|(l, _)| *l);
    }

    for edge in edges.iter_mut() {
        if edge.src_symbol.is_some() { continue; }
        let (src_file, edge_line) = match (&edge.src_file, edge.line) {
            (Some(f), Some(l)) => (f.as_str(), l),
            _ => continue,
        };
        let syms = match file_fns.get(src_file) {
            Some(s) => s,
            None => continue,
        };
        // Last symbol that starts at or before this line
        if let Some((_, name)) = syms.iter().rev().find(|(sl, _)| *sl <= edge_line) {
            edge.src_symbol = Some((*name).to_string());
        }
    }
}
