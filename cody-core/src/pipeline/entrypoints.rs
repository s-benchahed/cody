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

    // Route path propagation: carry path + method + middleware from router files to handler files
    let route_paths: Vec<(String, String, String, Vec<String>)> = map.values()
        .filter(|ep| ep.path.is_some() && ep.kind == "route")
        .map(|ep| (
            ep.fn_name.clone(),
            ep.path.clone().unwrap(),
            ep.method.clone().unwrap_or_default(),
            ep.middleware.clone(),
        ))
        .collect();

    // Track which (fn_name, router_file) pairs had their path successfully propagated
    // to a different file (the actual handler file). These router-file entries are
    // synthetic — the function isn't defined there — and should be removed.
    let mut to_remove: HashSet<(String, String)> = HashSet::new();

    for (fn_name, path, method, middleware) in &route_paths {
        if let Some(files) = symbol_files.get(fn_name) {
            for actual_file in files {
                let key = (fn_name.clone(), actual_file.clone());
                if let Some(ep) = map.get_mut(&key) {
                    if ep.path.is_none() {
                        ep.path   = Some(path.clone());
                        ep.method = Some(method.clone());
                        // Route registration middleware (with_lp_auth, with_admin_auth)
                        // takes priority over extractor types (State, LpAuthUser)
                        if !middleware.is_empty() {
                            ep.middleware = middleware.clone();
                        }
                        if ep.kind == "leaf" { ep.kind = "route".into(); }
                        if !ep.heuristics.contains(&"route_decorator".to_string()) {
                            ep.heuristics.push("route_decorator".into());
                        }
                        // Mark the router-file entry for removal (it's a reference, not a definition)
                        // Only remove if the actual handler file differs from the hint file
                        // (identified by it being absent from symbol_files or in a different file)
                    }
                    // If we found the symbol in actual_file and it differs from the route hint file,
                    // the route hint entries for this fn_name in OTHER files should be cleaned up.
                }
            }
            // Remove router-file entries for this fn_name that are NOT in symbol_files
            // (i.e., the symbol is not defined there — it's just a router registration)
            let defined_files: HashSet<&String> = files.iter().collect();
            for ((k_fn, k_file), _) in map.iter() {
                if k_fn == fn_name && !defined_files.contains(k_file) {
                    to_remove.insert((k_fn.clone(), k_file.clone()));
                }
            }
        }
    }

    for key in &to_remove {
        map.remove(key);
    }

    map.into_values()
        .filter(|e| e.confidence >= min_confidence || !e.heuristics.is_empty())
        .collect()
}
