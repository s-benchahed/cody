pub mod client;
pub mod servers;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;

use client::LspClient;
use servers::ServerSpec;
use crate::db::models::BoundaryEvent;

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
