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
                        id: None, name,
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
                                id: None, src_file: Some(file_str.clone()), src_symbol: None,
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
                            id: None, src_file: Some(file_str.clone()), src_symbol: None,
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
                let key_raw = node_text(&key_cap.node, source)
                    .trim_matches(|c| c == '"')
                    .to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = rust_classify(&pattern);
                events.push(BoundaryEvent {
                    id: None, fn_name: "<module>".into(), file: file_str.clone(),
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

        // Rust extractor middleware detection
        let extractor_fn_re = once_cell::sync::Lazy::force(&crate::patterns::http::RUST_EXTRACTOR_FN_RE);
        let extractor_types_re = once_cell::sync::Lazy::force(&crate::patterns::http::RUST_EXTRACTOR_TYPES_RE);
        // Build a set of known route handler names
        let route_names: std::collections::HashSet<String> = hints.iter()
            .filter(|h| h.kind == "route")
            .map(|h| h.fn_name.clone())
            .collect();

        for cap in extractor_fn_re.captures_iter(src) {
            let fn_name = cap.get(1).map_or("", |m| m.as_str()).to_string();
            if fn_name.is_empty() { continue; }
            // Collect extractor types from the params substring
            let full_match = cap.get(0).map_or("", |m| m.as_str());
            let mw: Vec<String> = extractor_types_re.captures_iter(full_match)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect();
            if mw.is_empty() { continue; }

            if let Some(existing) = hints.iter_mut().find(|h| h.fn_name == fn_name && h.kind == "route") {
                existing.middleware = mw;
            } else if !route_names.contains(&fn_name) {
                hints.push(EntryPointHint {
                    fn_name, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("axum".into()),
                    path: None, method: None,
                    confidence: 0.75, heuristic: "extractor_fn".into(), middleware: mw,
                });
            }
        }

        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() as i64 + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("pub fn ").count() as i64 + src.matches("pub struct ").count() as i64;
        let imports = src.matches("use ").count() as i64;
        Ok(FileMetaCounts { line_count: lines, export_count: exports, import_count: imports })
    }
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
