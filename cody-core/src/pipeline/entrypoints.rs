use std::collections::{HashMap, HashSet};
use crate::db::models::EntryPoint;
use crate::extractor::ExtractedFacts;

pub fn detect(all_facts: &[ExtractedFacts], min_confidence: f64) -> Vec<EntryPoint> {
    let mut map: HashMap<(String, String), EntryPoint> = HashMap::new();

    // Heuristic 1: exported symbols not called by anyone
    let all_callees: HashSet<String> = all_facts.iter()
        .flat_map(|f| f.edges.iter())
        .filter(|e| e.rel == "calls")
        .filter_map(|e| e.dst_symbol.clone())
        .collect();

    for facts in all_facts {
        for sym in &facts.symbols {
            if sym.is_exported && !all_callees.contains(&sym.name) {
                let key = (sym.name.clone(), facts.file.clone());
                let ep = map.entry(key).or_insert(EntryPoint {
                    fn_name: sym.name.clone(), file: facts.file.clone(),
                    line: sym.line, kind: "leaf".into(), framework: None,
                    path: None, method: None, confidence: 0.0,
                    heuristics: vec![], middleware: vec![],
                });
                ep.confidence = ep.confidence.max(0.70);
                if !ep.heuristics.contains(&"exported_leaf".to_string()) {
                    ep.heuristics.push("exported_leaf".into());
                }
            }
        }
    }

    // Heuristics 2-5: plugin-provided hints
    let symbol_files: HashMap<String, Vec<String>> = all_facts.iter()
        .flat_map(|f| f.symbols.iter().map(move |s| (s.name.clone(), f.file.clone())))
        .fold(HashMap::new(), |mut m, (n, f)| { m.entry(n).or_default().push(f); m });

    for facts in all_facts {
        for hint in &facts.entry_hints {
            let key = (hint.fn_name.clone(), hint.file.clone());
            let ep = map.entry(key).or_insert(EntryPoint {
                fn_name:    hint.fn_name.clone(),
                file:       hint.file.clone(),
                line:       hint.line,
                kind:       hint.kind.clone(),
                framework:  hint.framework.clone(),
                path:       hint.path.clone(),
                method:     hint.method.clone(),
                confidence: 0.0,
                heuristics: vec![],
                middleware: hint.middleware.clone(),
            });
            ep.confidence = ep.confidence.max(hint.confidence);
            if hint.kind != "leaf" { ep.kind = hint.kind.clone(); }
            if ep.framework.is_none() { ep.framework = hint.framework.clone(); }
            if ep.path.is_none()      { ep.path      = hint.path.clone(); }
            if ep.method.is_none()    { ep.method    = hint.method.clone(); }
            if !ep.heuristics.contains(&hint.heuristic) {
                ep.heuristics.push(hint.heuristic.clone());
            }
            if ep.middleware.is_empty() && !hint.middleware.is_empty() {
                ep.middleware = hint.middleware.clone();
            }
        }
    }

    // Route path propagation
    let route_paths: Vec<(String, String, String)> = map.values()
        .filter(|ep| ep.path.is_some() && ep.kind == "route")
        .map(|ep| (ep.fn_name.clone(), ep.path.clone().unwrap(), ep.method.clone().unwrap_or_default()))
        .collect();

    for (fn_name, path, method) in &route_paths {
        if let Some(files) = symbol_files.get(fn_name) {
            for actual_file in files {
                let key = (fn_name.clone(), actual_file.clone());
                if let Some(ep) = map.get_mut(&key) {
                    if ep.path.is_none() {
                        ep.path   = Some(path.clone());
                        ep.method = Some(method.clone());
                        if !ep.heuristics.contains(&"route_decorator".to_string()) {
                            ep.heuristics.push("route_decorator".into());
                        }
                    }
                }
            }
        }
    }

    map.into_values()
        .filter(|e| e.confidence >= min_confidence || !e.heuristics.is_empty())
        .collect()
}
