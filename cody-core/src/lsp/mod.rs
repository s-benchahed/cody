pub mod client;
pub mod servers;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;

use client::LspClient;
use servers::ServerSpec;
use crate::db::models::BoundaryEvent;
use crate::extractor::ExtractedFacts;

#[derive(Debug, Default)]
pub struct LspStats {
    pub events_checked:   usize,
    pub events_confirmed: usize,
    pub events_rejected:  usize,
    pub servers_started:  usize,
}

struct VerifiableRef {
    idx:      usize,
    file:     String,
    language: String,
    line:     Option<i64>,
    medium:   String,
    key_raw:  String,
}

fn lang_for_file(file: &str) -> String {
    let ext = std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "js" | "jsx" | "mjs" | "cjs" => "javascript".to_string(),
        "py" => "python".to_string(),
        _ => ext.to_string(),
    }
}

/// Enrich boundary events using LSP hover queries.
pub async fn enrich_boundary_events(
    events: Vec<BoundaryEvent>,
    root_dir: &Path,
) -> Result<(Vec<BoundaryEvent>, LspStats)> {
    let mut stats = LspStats::default();

    let available = servers::detect();
    if available.is_empty() {
        tracing::info!("LSP: no language servers found in PATH — skipping enrichment");
        return Ok((events, stats));
    }
    tracing::info!("LSP: found servers for: {}", available.keys().cloned().collect::<Vec<_>>().join(", "));

    // Filter verifiable events: AST-sourced, has line, medium is actionable
    let verifiable_refs: Vec<VerifiableRef> = events.iter().enumerate()
        .filter(|(_, ev)| {
            ev.prov_source == "ast"
            && ev.line.is_some()
            && matches!(ev.medium.as_str(), "redis"|"kafka"|"sql"|"http_header")
        })
        .map(|(idx, ev)| VerifiableRef {
            idx,
            file: ev.file.clone(),
            language: lang_for_file(&ev.file),
            line: ev.line,
            medium: ev.medium.clone(),
            key_raw: ev.key_raw.clone(),
        })
        .collect();

    if verifiable_refs.is_empty() { return Ok((events, stats)); }

    // Group events by (language, file)
    let mut by_lang: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, vr) in verifiable_refs.iter().enumerate() {
        by_lang.entry(vr.language.clone()).or_default().push(i);
    }

    // Collect updates: (index into events, new_confidence, note)
    let mut updates: Vec<(usize, f64, String)> = Vec::new();

    for (lang, lang_indices) in &by_lang {
        let Some(spec) = available.get(lang.as_str()) else { continue };

        let mut by_workspace: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for &i in lang_indices {
            let file = Path::new(&verifiable_refs[i].file);
            let ws = find_workspace_root(file, lang)
                .unwrap_or_else(|| root_dir.to_path_buf());
            by_workspace.entry(ws).or_default().push(i);
        }

        for (workspace, ws_indices) in &by_workspace {
            let ws_events: Vec<&VerifiableRef> = ws_indices.iter().map(|&i| &verifiable_refs[i]).collect();
            tracing::info!("LSP: starting {} for workspace {}", spec.binary, workspace.display());
            match process_language(workspace, spec, &ws_events, &mut stats).await {
                Ok(mut batch_updates) => updates.append(&mut batch_updates),
                Err(e) => tracing::warn!("LSP enrichment failed for {lang} @ {}: {e}", workspace.display()),
            }
        }
    }

    // Apply updates to events vec
    let mut events = events;
    for (idx, confidence, note) in updates {
        if let Some(ev) = events.get_mut(idx) {
            ev.prov_confidence = confidence;
            ev.prov_note = Some(note);
        }
    }

    Ok((events, stats))
}

// ── workspace root detection ────────────────────────────────────────────────

fn find_workspace_root(file: &Path, language: &str) -> Option<PathBuf> {
    let manifests: &[&str] = match language {
        "rust"                      => &["Cargo.toml"],
        "typescript" | "javascript" => &["tsconfig.json", "package.json"],
        "python"                    => &["pyproject.toml", "setup.cfg", "setup.py", "requirements.txt"],
        _                           => return None,
    };
    let mut dir = file.parent()?;
    loop {
        for manifest in manifests {
            if dir.join(manifest).exists() {
                return Some(dir.to_path_buf());
            }
        }
        match dir.parent() {
            Some(p) => dir = p,
            None    => return None,
        }
    }
}

// ── per-language processing ─────────────────────────────────────────────────

async fn process_language(
    root_dir: &Path,
    spec: &ServerSpec,
    events: &[&VerifiableRef],
    stats: &mut LspStats,
) -> Result<Vec<(usize, f64, String)>> {
    let mut lsp = LspClient::spawn(spec.binary, spec.args, root_dir).await?;
    stats.servers_started += 1;

    let mut updates: Vec<(usize, f64, String)> = Vec::new();

    // Group by file so we open each file once
    let mut by_file: HashMap<&str, Vec<&VerifiableRef>> = HashMap::new();
    for ev in events {
        by_file.entry(ev.file.as_str()).or_default().push(ev);
    }

    for (file_path, file_events) in &by_file {
        let path = PathBuf::from(file_path);
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        let lines: Vec<&str> = source.lines().collect();

        if let Err(e) = lsp.open_file(&path, &source, spec.language_id).await {
            tracing::debug!("LSP: open_file {file_path}: {e}");
            continue;
        }

        for ev in file_events {
            let Some(line_1based) = ev.line else { continue };
            let line_0 = (line_1based - 1).max(0) as usize;
            let Some(&line_text) = lines.get(line_0) else { continue };

            let Some(col) = find_receiver_col(line_text, &ev.medium) else { continue };

            stats.events_checked += 1;

            match lsp.hover(&path, line_0 as u32, col as u32).await {
                Ok(Some(hover_text)) => {
                    let verdict = classify_hover(&hover_text, &ev.medium);
                    tracing::debug!(
                        "LSP hover [{}/{}]: {:?} → {:?}",
                        ev.medium, ev.key_raw, hover_text.chars().take(60).collect::<String>(), verdict
                    );
                    match verdict {
                        HoverVerdict::Confirmed => {
                            updates.push((ev.idx, 0.97, "lsp:confirmed".to_string()));
                            stats.events_confirmed += 1;
                        }
                        HoverVerdict::Rejected => {
                            updates.push((ev.idx, 0.15, "lsp:rejected".to_string()));
                            stats.events_rejected += 1;
                        }
                        HoverVerdict::Unknown => {}
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::debug!("LSP hover error: {e}"),
            }
        }
    }

    lsp.shutdown().await.ok();
    Ok(updates)
}

// ── hover type classification ───────────────────────────────────────────────

#[derive(Debug)]
enum HoverVerdict { Confirmed, Rejected, Unknown }

fn classify_hover(text: &str, medium: &str) -> HoverVerdict {
    let t = text.to_lowercase();
    let is_ts_typed = t.contains("```typescript");
    let is_py_typed = t.contains("```python");
    let is_typed_hover = t.contains("```rust") || is_ts_typed || is_py_typed;
    match medium {
        "redis" => {
            let redis_types = ["redis", "ioredis", "redisclient", "connectionpool", "strictredis"];
            let non_redis = [
                "map<", "dict[", "hashmap<", "btreemap<", "object", "any",
                "pgrow", "sqliterow", "mysqlrow", "row", "pgpool", "headermap",
                "value", "jsonvalue", "serde_json",
                "urlsearchparams", "string", "number", "boolean", "undefined", "null",
                "import", "function", "array", "promise",
            ];
            if redis_types.iter().any(|p| t.contains(p)) {
                HoverVerdict::Confirmed
            } else if is_typed_hover {
                HoverVerdict::Rejected
            } else if non_redis.iter().any(|p| t.contains(p)) {
                HoverVerdict::Rejected
            } else {
                HoverVerdict::Unknown
            }
        }
        "kafka" => {
            let kafka_types = ["kafka", "producer", "consumer", "kafkaclient"];
            if kafka_types.iter().any(|p| t.contains(p)) {
                HoverVerdict::Confirmed
            } else if is_typed_hover {
                HoverVerdict::Rejected
            } else {
                HoverVerdict::Unknown
            }
        }
        "sql" => {
            let sql_types = ["prisma", "pool", "connection", "querybuilder", "sequelize", "typeorm", "db", "database"];
            if sql_types.iter().any(|p| t.contains(p)) {
                HoverVerdict::Confirmed
            } else if is_typed_hover {
                HoverVerdict::Rejected
            } else {
                HoverVerdict::Unknown
            }
        }
        "http_header" => {
            let http_types = ["request", "response", "incomingmessage", "serverresponse",
                              "headers", "headermap", "http"];
            let non_http   = ["redis", "map<", "dict[", "pgrow", "sqliterow", "value"];
            if http_types.iter().any(|p| t.contains(p)) {
                HoverVerdict::Confirmed
            } else if non_http.iter().any(|p| t.contains(p)) {
                HoverVerdict::Rejected
            } else if is_typed_hover {
                HoverVerdict::Rejected
            } else {
                HoverVerdict::Unknown
            }
        }
        "grpc" => {
            let grpc_types = ["grpc", "tonic", "protobuf", "prost", "request", "response", "stub"];
            if grpc_types.iter().any(|p| t.contains(p)) { HoverVerdict::Confirmed }
            else if is_typed_hover { HoverVerdict::Rejected }
            else { HoverVerdict::Unknown }
        }
        _ => HoverVerdict::Unknown,
    }
}

// ── edge resolution via textDocument/definition ────────────────────────────

#[derive(Debug, Default)]
pub struct LspEdgeStats {
    pub ambiguous_checked: usize,
    pub resolved:          usize,
}

struct Candidate {
    facts_idx:  usize,
    edge_idx:   usize,
    src_file:   String,
    language:   String,
    line:       i64,
    dst_symbol: String,
}

/// For every call edge where `dst_file` is None and the callee symbol is defined
/// in more than one file (ambiguous for static resolution), use LSP
/// `textDocument/definition` at the call site to resolve the exact target file.
/// Patches `dst_file` in-place so `build_adjacency` takes the direct-edge path.
pub async fn resolve_ambiguous_edges(
    all_facts: &mut Vec<ExtractedFacts>,
    root_dir: &Path,
) -> Result<LspEdgeStats> {
    let mut stats = LspEdgeStats::default();

    let available = servers::detect();
    if available.is_empty() {
        tracing::info!("LSP edge resolution: no language servers found — skipping");
        return Ok(stats);
    }

    // Build symbol → files map (same logic as build_adjacency)
    let mut symbol_files: HashMap<String, Vec<String>> = HashMap::new();
    for facts in all_facts.iter() {
        for sym in &facts.symbols {
            if sym.kind == "function" {
                symbol_files.entry(sym.name.clone())
                    .or_default()
                    .push(facts.file.clone());
            }
        }
    }

    // Collect ambiguous edges: dst_file=None, symbol defined in >1 file
    let mut candidates: Vec<Candidate> = Vec::new();
    for (fi, facts) in all_facts.iter().enumerate() {
        for (ei, edge) in facts.edges.iter().enumerate() {
            if edge.rel != "calls" || edge.dst_file.is_some() { continue; }
            let dst_sym = match &edge.dst_symbol { Some(s) => s.clone(), None => continue };
            let line    = match edge.line          { Some(l) => l,        None => continue };
            let src     = match &edge.src_file     { Some(f) => f.clone(), None => continue };
            let count   = symbol_files.get(&dst_sym).map_or(0, |v| v.len());
            if count <= 1 { continue; } // static resolution handles this already
            candidates.push(Candidate {
                facts_idx: fi, edge_idx: ei,
                src_file: src, language: facts.language.clone(),
                line, dst_symbol: dst_sym,
            });
        }
    }

    stats.ambiguous_checked = candidates.len();
    if candidates.is_empty() { return Ok(stats); }
    tracing::info!("LSP edge resolution: {} ambiguous edges to resolve", candidates.len());

    // Group by language → workspace → [candidate indices]
    let mut by_lang: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        by_lang.entry(c.language.clone()).or_default().push(i);
    }

    // patches: (facts_idx, edge_idx, resolved_file_string)
    let mut patches: Vec<(usize, usize, String)> = Vec::new();

    for (lang, lang_idxs) in &by_lang {
        let Some(spec) = available.get(lang.as_str()) else { continue };

        let mut by_workspace: HashMap<PathBuf, Vec<usize>> = HashMap::new();
        for &i in lang_idxs {
            let ws = find_workspace_root(Path::new(&candidates[i].src_file), lang)
                .unwrap_or_else(|| root_dir.to_path_buf());
            by_workspace.entry(ws).or_default().push(i);
        }

        for (workspace, ws_idxs) in &by_workspace {
            let ws_candidates: Vec<&Candidate> = ws_idxs.iter().map(|&i| &candidates[i]).collect();
            tracing::info!(
                "LSP edge resolution: starting {} for {} ({} edges)",
                spec.binary, workspace.display(), ws_candidates.len()
            );
            match resolve_edges_workspace(&workspace, spec, &ws_candidates, root_dir, &mut stats).await {
                Ok(mut batch) => patches.append(&mut batch),
                Err(e) => tracing::warn!("LSP edge resolution failed for {lang}: {e}"),
            }
        }
    }

    // Apply patches
    for (fi, ei, resolved) in patches {
        if let Some(edge) = all_facts.get_mut(fi).and_then(|f| f.edges.get_mut(ei)) {
            edge.dst_file = Some(resolved);
        }
    }

    tracing::info!(
        "LSP edge resolution: resolved {}/{} ambiguous edges",
        stats.resolved, stats.ambiguous_checked
    );
    Ok(stats)
}

async fn resolve_edges_workspace(
    workspace:  &Path,
    spec:       &ServerSpec,
    candidates: &[&Candidate],
    root_dir:   &Path,
    stats:      &mut LspEdgeStats,
) -> Result<Vec<(usize, usize, String)>> {
    // Candidate is a local struct — we can reference it because resolve_edges_workspace
    // is defined in the same module scope as resolve_ambiguous_edges.
    let mut lsp = LspClient::spawn(spec.binary, spec.args, workspace).await?;
    let mut patches: Vec<(usize, usize, String)> = Vec::new();

    // Group by source file so we open each file once
    let mut by_file: HashMap<&str, Vec<&Candidate>> = HashMap::new();
    for c in candidates {
        by_file.entry(c.src_file.as_str()).or_default().push(c);
    }

    for (file_path, file_candidates) in &by_file {
        let path = std::path::PathBuf::from(file_path);
        let Ok(source) = std::fs::read_to_string(&path) else { continue };
        let lines: Vec<&str> = source.lines().collect();

        if let Err(e) = lsp.open_file(&path, &source, spec.language_id).await {
            tracing::debug!("LSP definition: open_file {file_path}: {e}");
            continue;
        }

        for c in file_candidates {
            let line_0 = (c.line - 1).max(0) as usize;
            let Some(&line_text) = lines.get(line_0) else { continue };

            // Find the column of the callee symbol on this line
            let Some(col) = find_symbol_col(line_text, &c.dst_symbol) else { continue };

            match lsp.definition(&path, line_0 as u32, col as u32, root_dir).await {
                Ok(Some(resolved_path)) => {
                    let resolved_str = resolved_path.to_string_lossy().into_owned();
                    tracing::debug!(
                        "LSP definition: {}:{} {}() → {}",
                        file_path, c.line, c.dst_symbol, resolved_str
                    );
                    patches.push((c.facts_idx, c.edge_idx, resolved_str));
                    stats.resolved += 1;
                }
                Ok(None) => {}
                Err(e) => tracing::debug!("LSP definition error for {}: {e}", c.dst_symbol),
            }
        }
    }

    lsp.shutdown().await.ok();
    Ok(patches)
}

/// Find the column (byte offset) of the first occurrence of `symbol` on `line`.
fn find_symbol_col(line: &str, symbol: &str) -> Option<usize> {
    // Look for the symbol as a whole word (not a substring of a longer identifier)
    let mut start = 0;
    while let Some(pos) = line[start..].find(symbol) {
        let abs = start + pos;
        let before_ok = abs == 0 || !line.as_bytes().get(abs - 1).map_or(false, |&b| b.is_ascii_alphanumeric() || b == b'_');
        let after_ok  = !line.as_bytes().get(abs + symbol.len()).map_or(false, |&b| b.is_ascii_alphanumeric() || b == b'_');
        if before_ok && after_ok {
            return Some(abs);
        }
        start = abs + 1;
    }
    None
}

// (find_receiver_col follows below)

fn find_receiver_col(line: &str, medium: &str) -> Option<usize> {
    let method_patterns: &[&str] = match medium {
        "redis"       => &["get(", "set(", "hget(", "hset(", "del(", "setex(", "lpush("],
        "kafka"       => &["send(", "produce(", "subscribe(", "publish("],
        "sql"         => &["query(", "execute(", "findMany(", "findFirst(", "create("],
        "http_header" => &["setHeader(", "header(", "set(", "get("],
        _             => return None,
    };

    for pat in method_patterns {
        if let Some(idx) = line.find(pat) {
            let before_dot = idx.saturating_sub(1);
            if line.as_bytes().get(before_dot) == Some(&b'.') {
                let start = line[..before_dot]
                    .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                if start < before_dot {
                    return Some(start);
                }
            }
        }
    }
    None
}
