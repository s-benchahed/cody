use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use crate::db::{models::EntryPoint, store};
use crate::extractor::ExtractedFacts;

/// Run all 5 heuristics, merge by (fn_name, file), write to DB.
pub fn detect(
    conn: &Connection,
    all_facts: &[ExtractedFacts],
    min_confidence: f64,
) -> Result<Vec<EntryPoint>> {
    let mut map: HashMap<(String, String), EntryPoint> = HashMap::new();

    // ── Heuristic 1: Exported symbols with no internal callers (SQL) ───────
    let h1_results = conn.prepare(
        "SELECT s.name, s.file, s.line FROM symbols s
         WHERE s.is_exported = 1
         AND s.name NOT IN (
             SELECT COALESCE(dst_symbol,'') FROM edges
             WHERE rel = 'calls' AND dst_symbol IS NOT NULL
         )"
    )?
    .query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,Option<i64>>(2)?)))?
    .filter_map(|r| r.ok())
    .collect::<Vec<_>>();

    for (name, file, line) in h1_results {
        let key = (name.clone(), file.clone());
        let ep = map.entry(key).or_insert(EntryPoint {
            id: None, fn_name: name, file, line,
            kind: "leaf".into(), framework: None,
            path: None, method: None,
            confidence: 0.0, heuristics: vec![], middleware: None,
        });
        ep.confidence = ep.confidence.max(0.70);
        if !ep.heuristics.contains(&"exported_leaf".to_string()) {
            ep.heuristics.push("exported_leaf".into());
        }
    }

    // ── Heuristics 2–5: Plugin-provided hints from extract phase ──────────
    for facts in all_facts {
        for hint in &facts.entry_hints {
            let key = (hint.fn_name.clone(), hint.file.clone());
            let ep = map.entry(key).or_insert(EntryPoint {
                id: None,
                fn_name:    hint.fn_name.clone(),
                file:       hint.file.clone(),
                line:       hint.line,
                kind:       hint.kind.clone(),
                framework:  hint.framework.clone(),
                path:       hint.path.clone(),
                method:     hint.method.clone(),
                confidence: 0.0,
                heuristics: vec![],
                middleware: None,
            });
            ep.confidence = ep.confidence.max(hint.confidence);
            // Prefer more specific kind
            if hint.kind != "leaf" {
                ep.kind = hint.kind.clone();
            }
            if ep.framework.is_none() { ep.framework = hint.framework.clone(); }
            if ep.path.is_none()      { ep.path      = hint.path.clone(); }
            if ep.method.is_none()    { ep.method     = hint.method.clone(); }
            if !ep.heuristics.contains(&hint.heuristic) {
                ep.heuristics.push(hint.heuristic.clone());
            }
            // Merge middleware from hints
            if !hint.middleware.is_empty() {
                let mw_json = serde_json::to_string(&hint.middleware).unwrap_or_default();
                ep.middleware = Some(mw_json);
            }
        }
    }

    // ── Route path propagation: hints from route-registration files (e.g. main.rs) ──
    // A .route("/path", get(handler)) hint lives in the routing file but the handler
    // fn lives elsewhere. Propagate path/method to the matching (fn_name, actual_file) entry.
    {
        // Collect propagation targets: route hints whose fn_name exists in a different file's entry
        let route_paths: Vec<(String, String, String)> = map.values()
            .filter(|ep| ep.path.is_some() && ep.kind == "route")
            .map(|ep| (ep.fn_name.clone(), ep.path.clone().unwrap(), ep.method.clone().unwrap_or_default()))
            .collect();

        if !route_paths.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT name, file FROM symbols WHERE kind IN ('function','method') AND name = ?1"
            )?;
            for (fn_name, path, method) in &route_paths {
                let matches: Vec<String> = stmt.query_map([fn_name], |r| r.get(1))?
                    .filter_map(|r| r.ok())
                    .collect();
                for actual_file in matches {
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
    }

    // Filter, collect, store
    let eps: Vec<EntryPoint> = map.into_values()
        .filter(|e| e.confidence >= min_confidence || !e.heuristics.is_empty())
        .collect();

    store::insert_entry_points(conn, &eps)?;

    tracing::info!(
        "Entry points: {} detected ({} above threshold {:.2})",
        eps.len(),
        eps.iter().filter(|e| e.confidence >= min_confidence).count(),
        min_confidence
    );

    Ok(eps)
}
