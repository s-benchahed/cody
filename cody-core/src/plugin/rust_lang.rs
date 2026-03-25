use std::path::Path;
use anyhow::Result;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct RustPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/rust_lang/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/rust_lang/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/rust_lang/boundaries.scm");

impl LanguagePlugin for RustPlugin {
    fn language_name(&self) -> &'static str { "rust" }
    fn extensions(&self) -> &[&'static str] { &["rs"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("Rust symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            let is_class = matches!(pattern.as_str(), "struct" | "enum" | "trait");
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() || name == "vis" { continue; }
                    // Rust: pub items are exported
                    let node = cap.node;
                    let is_exported = node.parent()
                        .and_then(|p| p.child_by_field_name("visibility"))
                        .is_some();
                    symbols.push(Symbol {
                        name,
                        kind: if is_class { "class" } else { "function" }.to_string(),
                        file: file_str.clone(),
                        line: Some(node.start_position().row as i64 + 1),
                        signature: None, is_exported,
                        prov_source: "ast".into(), prov_confidence: 0.95,
                    });
                }
            }
        }
        Ok(symbols)
    }

    fn extract_edges(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Edge>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, EDGES_SCM)
            .map_err(|e| anyhow::anyhow!("Rust edges: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut edges = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            match pattern.as_str() {
                "call" | "method_call" => {
                    let cap = m.captures.iter().find(|c| {
                        let n = &query.capture_names()[c.index as usize];
                        *n == "callee" || *n == "method"
                    });
                    if let Some(cap) = cap {
                        let name = node_text(&cap.node, source).to_string();
                        if !name.is_empty() {
                            edges.push(Edge {
                                src_file: Some(file_str.clone()), src_symbol: None,
                                rel: "calls".into(), dst_file: None, dst_symbol: Some(name),
                                context: None, line: Some(cap.node.start_position().row as i64 + 1),
                            });
                        }
                    }
                }
                "use" => {
                    let cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "crate");
                    if let Some(cap) = cap {
                        edges.push(Edge {
                            src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "imports".into(),
                            dst_file: Some(node_text(&cap.node, source).to_string()),
                            dst_symbol: None, context: None,
                            line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(edges)
    }

    fn extract_boundary_events(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<BoundaryEvent>> {
        let lang = self.tree_sitter_language();
        let query = match Query::new(&lang, BOUNDARIES_SCM) {
            Ok(q) => q,
            Err(_) => return Ok(vec![]),
        };
        let mut cursor = QueryCursor::new();
        let mut events = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = m.captures.iter()
                .max_by_key(|c| c.node.byte_range().len())
                .map(|c| query.capture_names()[c.index as usize].to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let key_cap = m.captures.iter().find(|c| {
                let n = &query.capture_names()[c.index as usize];
                *n == "key" || *n == "path" || *n == "sql"
            });
            if let Some(key_cap) = key_cap {
                let raw_text = node_text(&key_cap.node, source);
                // Strip string literal quotes. Handle both regular ("...") and raw (r#"..."#) strings.
                // For non-literal keys (variables, refs), strip leading & and use the identifier name.
                let key_raw = strip_rust_string_literal(raw_text)
                    .trim_start_matches('&')
                    .trim()
                    .to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = rust_classify(&pattern);
                // For SQL queries, infer read vs write from the SQL verb.
                let (medium, direction) = if medium == "sql" {
                    let sql_upper = key_raw.trim_start().to_uppercase();
                    let dir = if sql_upper.starts_with("SELECT") || sql_upper.starts_with("WITH") {
                        "read"
                    } else if sql_upper.starts_with("INSERT") || sql_upper.starts_with("UPDATE")
                        || sql_upper.starts_with("DELETE") || sql_upper.starts_with("MERGE")
                        || sql_upper.starts_with("UPSERT")
                    {
                        "write"
                    } else {
                        direction.as_str()
                    };
                    ("sql".to_string(), dir.to_string())
                // Reclassify redis reads that are HTTP header reads by key name.
                } else if medium == "redis" && direction == "read" && is_http_header_name(&key_raw) {
                    ("http_header".to_string(), "read".to_string())
                } else {
                    (medium, direction)
                };
                // For SQL: use table name as the display key, not the full query string
                let key_norm = if medium == "sql" {
                    let t = sql_table_name(&key_raw);
                    if t.is_empty() { key_norm } else { t }
                } else {
                    key_norm
                };
                events.push(BoundaryEvent {
                    fn_name: "<module>".into(), file: file_str.clone(),
                    line: Some(key_cap.node.start_position().row as i64 + 1),
                    direction, medium, key_raw, key_norm,
                    local_var: None, raw_context: None,
                    prov_source: "ast".into(), prov_confidence: 0.90,
                    prov_plugin: "rust".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // fn main()
        if src.contains("fn main(") {
            hints.push(EntryPointHint {
                fn_name: "main".into(), file: file_str.clone(), line: None,
                kind: "main".into(), framework: None, path: None, method: None,
                confidence: 0.95, heuristic: "main_fn".into(), middleware: vec![],
            });
        }

        // Actix/Rocket route macros
        let re = once_cell::sync::Lazy::force(&crate::patterns::http::RUST_ROUTE_RE);
        for cap in re.captures_iter(src) {
            let method = cap.get(1).map_or("get", |m| m.as_str()).to_uppercase();
            let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("actix/rocket".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // Axum builder: .route("/path", get(handler)) or .route("/path", get(module::sub::handler))
        let axum_re = once_cell::sync::Lazy::force(&crate::patterns::http::AXUM_ROUTE_RE);
        for cap in axum_re.captures_iter(src) {
            let path = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let method = cap.get(2).map_or("get", |m| m.as_str()).to_uppercase();
            let handler_full = cap.get(3).map_or("", |m| m.as_str());
            // Use only the last segment for fn_name (handles module::path::handler)
            let handler = handler_full.split("::").last().unwrap_or(handler_full).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("axum".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // Axum wrapped: .route("/path", with_lp_auth(post(handlers::fn), ...))
        let axum_wrapped_re = once_cell::sync::Lazy::force(&crate::patterns::http::AXUM_WRAPPED_ROUTE_RE);
        for cap in axum_wrapped_re.captures_iter(src) {
            let path    = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let wrapper = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let method  = cap.get(3).map_or("post", |m| m.as_str()).to_uppercase();
            let handler_full = cap.get(4).map_or("", |m| m.as_str());
            let handler = handler_full.split("::").last().unwrap_or(handler_full).to_string();
            if !handler.is_empty() {
                // The wrapper function name becomes a middleware label
                let mw = if wrapper.is_empty() { vec![] } else { vec![wrapper] };
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("axum".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: mw,
                });
            }
        }

        // Detect handler functions by presence of known Axum framework extractors.
        // Only adds hints for functions not already detected via route registration.
        // Never sets middleware — auth labels come only from route wrappers above.
        let extractor_fn_re = once_cell::sync::Lazy::force(&crate::patterns::http::RUST_EXTRACTOR_FN_RE);
        let route_names: std::collections::HashSet<String> = hints.iter()
            .filter(|h| h.kind == "route")
            .map(|h| h.fn_name.clone())
            .collect();

        for cap in extractor_fn_re.captures_iter(src) {
            let fn_name = cap.get(1).map_or("", |m| m.as_str()).to_string();
            if fn_name.is_empty() || route_names.contains(&fn_name) { continue; }
            hints.push(EntryPointHint {
                fn_name, file: file_str.clone(), line: None,
                kind: "route".into(), framework: Some("axum".into()),
                path: None, method: None,
                confidence: 0.75, heuristic: "extractor_fn".into(), middleware: vec![],
            });
        }

        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("pub fn ").count() + src.matches("pub struct ").count();
        let imports = src.matches("use ").count();
        Ok(FileMetaCounts { lines, exports, imports })
    }
}

/// Strip Rust string literal delimiters from a captured node's text.
/// Handles: "regular", r"raw", r#"raw_hash"#, r##"raw_double_hash"##
/// Non-string nodes (identifiers, expressions) are returned as-is.
fn strip_rust_string_literal(s: &str) -> &str {
    let s = s.trim();
    // Raw string: r"..." or r#"..."# etc.
    if s.starts_with('r') {
        let after_r = &s[1..];
        let hash_count = after_r.chars().take_while(|&c| c == '#').count();
        let prefix_len = 1 + hash_count + 1; // r + hashes + opening "
        let suffix_len = 1 + hash_count;      // closing " + hashes
        if s.len() >= prefix_len + suffix_len {
            let inner = &s[prefix_len..s.len() - suffix_len];
            // Verify the prefix ends with " and suffix starts with "
            if s.as_bytes().get(prefix_len - 1) == Some(&b'"')
                && s.as_bytes().get(s.len() - suffix_len) == Some(&b'"')
            {
                return inner;
            }
        }
    }
    // Regular string: "..."
    s.trim_matches('"')
}

/// Extract the primary table name from a SQL string for display purposes.
/// e.g. "SELECT id FROM users WHERE ..." → "users"
///      "INSERT INTO pacts (...) VALUES ..." → "pacts"
///      "DELETE FROM segmented_configs WHERE ..." → "segmented_configs"
fn sql_table_name(sql: &str) -> String {
    // Normalise whitespace
    let s = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = s.to_uppercase();

    // Find keyword that precedes the table name
    let after = if let Some(i) = upper.find("FROM ") {
        &s[i + 5..]
    } else if let Some(i) = upper.find("INTO ") {
        &s[i + 5..]
    } else if let Some(i) = upper.find("UPDATE ") {
        &s[i + 7..]
    } else if let Some(i) = upper.find("JOIN ") {
        &s[i + 5..]
    } else {
        return String::new();
    };

    // Take the first word, strip any trailing punctuation
    after.split_whitespace().next()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .to_lowercase()
}

/// Returns true if the key looks like an HTTP header name rather than a redis key.
/// HTTP headers are case-insensitive, typically hyphenated, and have well-known names.
fn is_http_header_name(key: &str) -> bool {
    let k = key.to_lowercase();
    // Standard and common custom HTTP headers
    const KNOWN: &[&str] = &[
        "authorization", "content-type", "content-length", "accept",
        "accept-language", "accept-encoding", "cache-control", "connection",
        "host", "origin", "referer", "user-agent", "cookie", "set-cookie",
        "location", "etag", "if-none-match", "if-modified-since",
        "last-modified", "vary", "access-control-allow-origin",
    ];
    if KNOWN.contains(&k.as_str()) { return true; }
    // Custom headers conventionally start with "x-"
    k.starts_with("x-")
}

fn rust_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"    => ("env".into(), "read".into()),
        "sql_query"   => ("sql".into(), "read".into()),
        "redis_get"   => ("redis".into(), "read".into()),
        "redis_set"   => ("redis".into(), "write".into()),
        "redis_op"    => ("redis".into(), "read".into()),
        "route"       => ("http_body".into(), "read".into()),
        "grpc_encode" => ("grpc".into(), "write".into()),
        "grpc_decode" => ("grpc".into(), "read".into()),
        _             => ("unknown".into(), "read".into()),
    }
}
