pub mod client;
pub mod servers;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;
use rusqlite::Connection;

use client::LspClient;
use servers::ServerSpec;

#[derive(Debug, Default)]
pub struct LspStats {
    pub events_checked:   usize,
    pub events_confirmed: usize,
    pub events_rejected:  usize,
    pub servers_started:  usize,
}

/// Enrich boundary events using LSP hover queries.
///
/// For each AST-sourced boundary event with a line number, spawn the
/// appropriate language server, hover over the receiver object, and
/// check whether its type matches the expected medium. Confirmed events
/// get prov_confidence bumped to 0.97; rejected events drop to 0.15.
pub async fn enrich_boundary_events(
    conn: &Connection,
    root_dir: &Path,
) -> Result<LspStats> {
    let mut stats = LspStats::default();

    let available = servers::detect();
    if available.is_empty() {
        tracing::info!("LSP: no language servers found in PATH — skipping enrichment");
        return Ok(stats);
    }
    tracing::info!("LSP: found servers for: {}", available.keys().cloned().collect::<Vec<_>>().join(", "));

    // Load verifiable events: AST-sourced, has line, medium is actionable
    let events = load_verifiable_events(conn)?;
    if events.is_empty() { return Ok(stats); }

    // Group events by (language, file)
    let mut by_lang: HashMap<String, Vec<EventRow>> = HashMap::new();
    for ev in events {
        by_lang.entry(ev.language.clone()).or_default().push(ev);
    }

    for (lang, lang_events) in &by_lang {
        let Some(spec) = available.get(lang.as_str()) else { continue };

        // Group events by workspace root (nearest Cargo.toml / package.json / etc.)
        // so each LSP server is started with the correct workspace.
        let mut by_workspace: HashMap<PathBuf, Vec<&EventRow>> = HashMap::new();
        for ev in lang_events {
            let file = Path::new(&ev.file);
            let ws = find_workspace_root(file, &lang)
                .unwrap_or_else(|| root_dir.to_path_buf());
            by_workspace.entry(ws).or_default().push(ev);
        }

        for (workspace, ws_events) in &by_workspace {
            tracing::info!("LSP: starting {} for workspace {}", spec.binary, workspace.display());
            match process_language(conn, workspace, spec, ws_events, &mut stats).await {
                Ok(()) => {}
                Err(e) => tracing::warn!("LSP enrichment failed for {lang} @ {}: {e}", workspace.display()),
            }
        }
    }

    Ok(stats)
}

// ── workspace root detection ────────────────────────────────────────────────

/// Walk up from a file to find the nearest directory containing the language's
/// workspace manifest (Cargo.toml, package.json, pyproject.toml, etc.).
fn find_workspace_root(file: &Path, language: &str) -> Option<PathBuf> {
    let manifests: &[&str] = match language {
        "rust"                   => &["Cargo.toml"],
        "typescript" | "javascript" => &["tsconfig.json", "package.json"],
        "python"                 => &["pyproject.toml", "setup.cfg", "setup.py", "requirements.txt"],
        _                        => return None,
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
    conn: &Connection,
    root_dir: &Path,
    spec: &ServerSpec,
    events: &[&EventRow],
    stats: &mut LspStats,
) -> Result<()> {
    let mut lsp = LspClient::spawn(spec.binary, spec.args, root_dir).await?;
    stats.servers_started += 1;

    // Group by file so we open each file once
    let mut by_file: HashMap<&str, Vec<&EventRow>> = HashMap::new();
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

            // Find the column of the receiver object on this line
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
                            update_event(conn, ev.id, 0.97, "lsp:confirmed")?;
                            stats.events_confirmed += 1;
                        }
                        HoverVerdict::Rejected => {
                            update_event(conn, ev.id, 0.15, "lsp:rejected")?;
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
    Ok(())
}

// ── hover type classification ───────────────────────────────────────────────

#[derive(Debug)]
enum HoverVerdict { Confirmed, Rejected, Unknown }

fn classify_hover(text: &str, medium: &str) -> HoverVerdict {
    let t = text.to_lowercase();
    // When a typed language server (TS, Rust) returns a hover, the type is authoritative.
    // If the type doesn't contain our target keyword, the object is definitely not that medium.
    // rust-analyzer may return plain text or markdown depending on content format negotiation.
    // TypeScript-language-server and pyright always return markdown-wrapped type annotations.
    // rust-analyzer may return plaintext; handled per-medium with known Rust type names.
    let is_ts_typed = t.contains("```typescript");
    let is_py_typed = t.contains("```python");
    let is_typed_hover = t.contains("```rust") || is_ts_typed || is_py_typed;
    match medium {
        "redis" => {
            let redis_types = ["redis", "ioredis", "redisclient", "connectionpool", "strictredis"];
            // Known non-Redis types: DB rows, JSON values, HTTP types, common Rust/TS types
            let non_redis = [
                "map<", "dict[", "hashmap<", "btreemap<", "object", "any",
                // Rust (sqlx, actix, serde)
                "pgrow", "sqliterow", "mysqlrow", "row", "pgpool", "headermap",
                "value", "jsonvalue", "serde_json",
                // TypeScript common non-Redis
                "urlsearchparams", "string", "number", "boolean", "undefined", "null",
                "import", "function", "array", "promise",
            ];
            if redis_types.iter().any(|p| t.contains(p)) {
                HoverVerdict::Confirmed
            } else if is_typed_hover {
                // Typed TS/Rust with markdown blocks → reject if no Redis keyword
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

/// Find the 0-based column of the receiver object in a source line for the given medium.
/// Looks for patterns like `obj.get(`, `obj.set(`, `obj.publish(`, etc.
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
            // Walk backwards from the `.` before the method name to find the object identifier
            let before_dot = idx.saturating_sub(1);
            if line.as_bytes().get(before_dot) == Some(&b'.') {
                // Find the start of the identifier before the dot
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

// ── DB helpers ──────────────────────────────────────────────────────────────

struct EventRow {
    id:       i64,
    file:     String,
    language: String,
    line:     Option<i64>,
    medium:   String,
    key_raw:  String,
}

fn load_verifiable_events(conn: &Connection) -> Result<Vec<EventRow>> {
    let mut stmt = conn.prepare(
        "SELECT be.id, be.file, COALESCE(fm.language,''), be.line, be.medium, be.key_raw
         FROM boundary_events be
         LEFT JOIN file_meta fm ON fm.file = be.file
         WHERE be.prov_source = 'ast'
           AND be.line IS NOT NULL
           AND be.medium IN ('redis','kafka','sql','http_header')
         ORDER BY be.file, be.line"
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(EventRow {
            id:       r.get(0)?,
            file:     r.get(1)?,
            language: r.get(2)?,
            line:     r.get(3)?,
            medium:   r.get(4)?,
            key_raw:  r.get(5)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
}

fn update_event(conn: &Connection, id: i64, confidence: f64, note: &str) -> Result<()> {
    conn.execute(
        "UPDATE boundary_events SET prov_confidence = ?1, prov_note = ?2 WHERE id = ?3",
        rusqlite::params![confidence, note, id],
    )?;
    Ok(())
}
