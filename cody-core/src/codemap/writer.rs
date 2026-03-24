use std::collections::{BTreeMap, HashSet};
use chrono::Local;
use super::{Codemap, ServiceEntry};

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
        let mut topo: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for (src, dst, med, key) in &codemap.topology {
            topo.entry((src.clone(), dst.clone()))
                .or_default()
                .push(format!("{}: {}", med, key));
        }
        for ((src, dst), mut keys) in topo {
            keys.sort(); keys.dedup();
            out.push_str(&format!("  {:20} →  {:20} {}\n", src, dst, keys.join(", ")));
        }
        out.push('\n');
    }

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
            for e in &public { out.push_str(&format_entry(e)); }
            out.push('\n');
        }
        for (auth, entries) in &authed {
            out.push_str(&format!("### [auth: {}]\n", auth));
            for e in entries { out.push_str(&format_entry(e)); }
            out.push('\n');
        }
        if !background.is_empty() {
            out.push_str("### Background\n");
            for e in &background { out.push_str(&format_entry(e)); }
            out.push('\n');
        }
    }

    out
}

fn format_entry(entry: &ServiceEntry) -> String {
    let mut out = String::new();

    if let (Some(method), Some(path)) = (&entry.ep.method, &entry.ep.path) {
        out.push_str(&format!("{} {}\n", method, path));
    } else {
        out.push_str(&format!("{}\n", entry.ep.fn_name));
    }

    if entry.io.is_empty() { return out; }

    // in: consumed inputs
    let grpc_in: HashSet<String> = entry.io.iter()
        .filter(|e| e.medium == "grpc" && e.direction == "read")
        .map(|e| format!("body{{{}}}", e.key_raw))
        .collect();
    let header_in: HashSet<String> = entry.io.iter()
        .filter(|e| e.medium == "http_header" && e.direction == "read")
        .map(|e| e.key_raw.clone())
        .collect();

    let mut inputs: Vec<String> = grpc_in.into_iter().collect();
    if !header_in.is_empty() {
        let mut hv: Vec<String> = header_in.into_iter().collect();
        hv.sort();
        inputs.push(format!("headers{{{}}}", hv.join(", ")));
    }
    if !inputs.is_empty() {
        inputs.sort();
        out.push_str(&format!("  in:   {}\n", inputs.join(", ")));
    }

    // I/O by medium
    for medium in &["sql", "redis", "kafka"] {
        let reads: HashSet<String> = entry.io.iter()
            .filter(|e| e.medium.as_str() == *medium && e.direction == "read")
            .map(|e| e.key_norm.clone())
            .collect();
        let writes: HashSet<String> = entry.io.iter()
            .filter(|e| e.medium.as_str() == *medium && e.direction == "write")
            .map(|e| e.key_norm.clone())
            .collect();
        if !reads.is_empty() {
            let mut rv: Vec<String> = reads.into_iter().collect(); rv.sort();
            out.push_str(&format!("  {}:  reads {}\n", medium, rv.join(", ")));
        }
        if !writes.is_empty() {
            let mut wv: Vec<String> = writes.into_iter().collect(); wv.sort();
            out.push_str(&format!("  {}:  writes {}\n", medium, wv.join(", ")));
        }
    }

    // Outbound gRPC
    let grpc_out: HashSet<String> = entry.io.iter()
        .filter(|e| e.medium == "grpc" && e.direction == "write")
        .map(|e| format!("→ ({})", e.key_raw))
        .collect();
    if !grpc_out.is_empty() {
        let mut gv: Vec<String> = grpc_out.into_iter().collect(); gv.sort();
        out.push_str(&format!("  grpc: {}\n", gv.join(", ")));
    }

    out
}
