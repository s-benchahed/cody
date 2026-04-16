use std::collections::{BTreeMap, BTreeSet};
use chrono::Local;
use super::{Codemap, ServiceEntry};
use crate::db::models::BoundaryEvent;

pub fn write(codemap: &Codemap) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "# Codemap — {}\nGenerated: {} | Files: {} | Languages: {}\n\n",
        codemap.project_name,
        Local::now().format("%Y-%m-%d"),
        codemap.file_count,
        codemap.languages.join(", "),
    ));

    if !codemap.topology.is_empty() {
        out.push_str("## Service Topology\n");
        // Group by (src, dst, medium) → sorted list of keys
        let mut topo: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
        for (src, dst, med, key) in &codemap.topology {
            topo.entry((src.clone(), dst.clone(), med.clone()))
                .or_default()
                .push(key.clone());
        }
        // Re-group by (src, dst) for display, collecting "medium: k1, k2" strings
        let mut display: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for ((src, dst, med), mut keys) in topo {
            keys.sort(); keys.dedup();
            display.entry((src, dst))
                .or_default()
                .push(format!("{}: {}", med, keys.join(", ")));
        }
        for ((src, dst), mut items) in display {
            items.sort();
            out.push_str(&format!("  {:20} →  {:20} {}\n", src, dst, items.join("  ")));
        }
        out.push('\n');
    }

    let root = codemap.root_dir.trim_end_matches('/');

    for (svc, data) in &codemap.services {
        out.push_str(&format!("## {} [{}]\n\n", svc, data.language));

        let mut sorted = data.entries.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| {
            a.ep.path.as_deref().unwrap_or("")
                .cmp(b.ep.path.as_deref().unwrap_or(""))
                .then(a.ep.fn_name.cmp(&b.ep.fn_name))
        });

        let mut public: Vec<&ServiceEntry> = vec![];
        let mut authed: BTreeMap<String, Vec<&ServiceEntry>> = BTreeMap::new();
        let mut background: Vec<&ServiceEntry> = vec![];

        for entry in &sorted {
            let auth_mw: Vec<String> = entry.ep.middleware.iter()
                .filter(|m| {
                    let ml = m.to_lowercase();
                    ml.contains("auth") || ml.contains("lp_auth") || ml.contains("admin")
                })
                .cloned().collect();
            if entry.ep.path.is_none() {
                background.push(entry);
            } else if auth_mw.is_empty() {
                public.push(entry);
            } else {
                authed.entry(auth_mw.join(", ")).or_default().push(entry);
            }
        }

        if !public.is_empty() {
            out.push_str("### Public\n");
            for e in &public { out.push_str(&format_entry(e, root)); }
            out.push('\n');
        }
        for (auth, entries) in &authed {
            out.push_str(&format!("### [auth: {}]\n", auth));
            for e in entries { out.push_str(&format_entry(e, root)); }
            out.push('\n');
        }
        if !background.is_empty() {
            out.push_str("### Background\n");
            for e in &background { out.push_str(&format_entry(e, root)); }
            out.push('\n');
        }
    }

    out
}

/// Strip `root` prefix and return a short relative path, optionally with `:line`.
fn rel_path(file: &str, root: &str, line: Option<i64>) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file).trim_start_matches('/');
    match line {
        Some(l) => format!("{}:{}", rel, l),
        None    => rel.to_string(),
    }
}

/// Collect unique `file:line` strings from a set of boundary events.
/// Returns an empty string when there's only one unique file and it matches `handler_file`.
/// Skips test/spec files as they're not useful navigation targets.
fn file_refs(events: &[&BoundaryEvent], root: &str, handler_file: &str) -> String {
    let handler_rel = handler_file.strip_prefix(root).unwrap_or(handler_file).trim_start_matches('/');
    let refs: BTreeSet<String> = events.iter()
        .map(|e| rel_path(&e.file, root, e.line))
        .filter(|r| {
            // skip test/spec files
            !r.contains("/test") && !r.contains("/spec") && !r.contains("_test.") && !r.contains("_spec.")
            // skip handler file itself (no new information)
            && !r.starts_with(handler_rel)
        })
        .collect();
    if refs.is_empty() {
        return String::new();
    }
    format!("  [{}]", refs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
}

fn format_entry(entry: &ServiceEntry, root: &str) -> String {
    let mut out = String::new();
    let handler_file = &entry.ep.file;

    if let (Some(method), Some(path)) = (&entry.ep.method, &entry.ep.path) {
        out.push_str(&format!("{} {}\n", method, path));
    } else {
        out.push_str(&format!("{}\n", entry.ep.fn_name));
    }

    // Always show the handler file
    out.push_str(&format!("  file: {}\n", rel_path(handler_file, root, entry.ep.line)));

    if entry.io.is_empty() { return out; }

    // in: consumed inputs
    let grpc_in: Vec<&BoundaryEvent> = entry.io.iter()
        .filter(|e| e.medium == "grpc" && e.direction == "read")
        .collect();
    let header_in: Vec<&BoundaryEvent> = entry.io.iter()
        .filter(|e| e.medium == "http_header" && e.direction == "read")
        .collect();

    let mut inputs: Vec<String> = grpc_in.iter()
        .map(|e| format!("body{{{}}}", e.key_raw))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter().collect();
    if !header_in.is_empty() {
        let mut hv: Vec<String> = header_in.iter()
            .map(|e| e.key_raw.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter().collect();
        hv.sort();
        inputs.push(format!("headers{{{}}}", hv.join(", ")));
    }
    if !inputs.is_empty() {
        inputs.sort();
        out.push_str(&format!("  in:   {}\n", inputs.join(", ")));
    }

    // I/O by medium: group by (medium, direction, key_norm), collect file refs
    for medium in &["sql", "redis", "kafka"] {
        let reads: Vec<&BoundaryEvent> = entry.io.iter()
            .filter(|e| e.medium.as_str() == *medium && e.direction == "read")
            .collect();
        let writes: Vec<&BoundaryEvent> = entry.io.iter()
            .filter(|e| e.medium.as_str() == *medium && e.direction == "write")
            .collect();

        if !reads.is_empty() {
            let mut keys: Vec<String> = reads.iter()
                .map(|e| e.key_norm.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter().collect();
            keys.sort();
            let refs = file_refs(&reads, root, handler_file);
            out.push_str(&format!("  {}:  reads {}{}\n", medium, keys.join(", "), refs));
        }
        if !writes.is_empty() {
            let mut keys: Vec<String> = writes.iter()
                .map(|e| e.key_norm.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter().collect();
            keys.sort();
            let refs = file_refs(&writes, root, handler_file);
            out.push_str(&format!("  {}:  writes {}{}\n", medium, keys.join(", "), refs));
        }
    }

    out
}
