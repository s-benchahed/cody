use std::path::Path;
use anyhow::Result;
use tree_sitter::{Node, Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct JavaScriptPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/javascript/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/javascript/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/javascript/boundaries.scm");

impl LanguagePlugin for JavaScriptPlugin {
    fn language_name(&self) -> &'static str { "javascript" }
    fn extensions(&self) -> &[&'static str] { &["js", "jsx", "mjs", "cjs"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_javascript::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("JS symbols query: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let is_export = query.capture_names()[m.pattern_index].contains("export");
            for cap in m.captures {
                let cap_name = &query.capture_names()[cap.index as usize];
                if *cap_name == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    let kind = if cap_name.contains("class") { "class" } else { "function" }.to_string();
                    symbols.push(Symbol {
                        id: None, name, kind, file: file_str.clone(),
                        line: Some(cap.node.start_position().row as i64 + 1),
                        signature: None,
                        is_exported: is_export,
                        prov_source: "ast".into(),
                        prov_confidence: 0.95,
                    });
                }
            }
        }
        Ok(symbols)
    }

    fn extract_edges(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Edge>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, EDGES_SCM)
            .map_err(|e| anyhow::anyhow!("JS edges query: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut edges = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            match pattern.as_str() {
                "call" | "method_call" => {
                    let callee_cap = m.captures.iter().find(|c| {
                        let n = query.capture_names()[c.index as usize].to_string();
                        n == "callee" || n == "method"
                    });
                    if let Some(cap) = callee_cap {
                        edges.push(Edge {
                            id: None, src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "calls".into(),
                            dst_file: None,
                            dst_symbol: Some(node_text(&cap.node, source).to_string()),
                            context: None,
                            line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                "import" | "require" => {
                    let path_cap = m.captures.iter().find(|c| {
                        query.capture_names()[c.index as usize] == "import_path"
                    });
                    if let Some(cap) = path_cap {
                        let raw = node_text(&cap.node, source);
                        let path = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
                        edges.push(Edge {
                            id: None, src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "imports".into(),
                            dst_file: Some(path),
                            dst_symbol: None, context: None,
                            line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                "extends" => {
                    let child = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "child");
                    let parent = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "parent");
                    if let (Some(child), Some(parent)) = (child, parent) {
                        edges.push(Edge {
                            id: None, src_file: Some(file_str.clone()),
                            src_symbol: Some(node_text(&child.node, source).to_string()),
                            rel: "extends".into(), dst_file: None,
                            dst_symbol: Some(node_text(&parent.node, source).to_string()),
                            context: None,
                            line: Some(child.node.start_position().row as i64 + 1),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(edges)
    }

    fn extract_boundary_events(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<BoundaryEvent>> {
        extract_js_boundaries(tree, source, file, BOUNDARIES_SCM, self.language_name())
    }

    fn entry_point_hints(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        detect_js_routes(tree, source, file, self.language_name())
    }

    fn file_meta_counts(&self, tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() as i64 + 1;
        let src_str = std::str::from_utf8(source).unwrap_or("");
        let exports = src_str.matches("export ").count() as i64;
        let imports = src_str.matches("import ").count() as i64
            + src_str.matches("require(").count() as i64;
        Ok(FileMetaCounts { line_count: lines, export_count: exports, import_count: imports })
    }
}

// ── shared helpers ─────────────────────────────────────────────────────────

pub fn extract_js_boundaries(
    tree: &tree_sitter::Tree,
    source: &[u8],
    file: &Path,
    scm: &str,
    plugin_name: &str,
) -> Result<Vec<BoundaryEvent>> {
    let lang: tree_sitter::Language = tree_sitter_javascript::language();
    let query = match Query::new(&lang, scm) {
        Ok(q) => q,
        Err(_) => return Ok(vec![]),
    };
    let mut cursor = QueryCursor::new();
    let mut events = Vec::new();
    let file_str = file.to_string_lossy().to_string();

    for m in cursor.matches(&query, tree.root_node(), source) {
        let pattern_name = m.captures.iter()
            .max_by_key(|c| c.node.byte_range().len())
            .map(|c| query.capture_names()[c.index as usize].to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let key_cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "key");
        if let Some(key_cap) = key_cap {
            let key_raw = node_text(&key_cap.node, source)
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .to_string();
            if key_raw.is_empty() { continue; }
            let key_norm = crate::patterns::normalise_key(&key_raw);
            let (medium, direction) = classify_boundary(&pattern_name);
            let fn_name = enclosing_function(key_cap.node, source);
            events.push(BoundaryEvent {
                id: None, fn_name, file: file_str.clone(),
                line: Some(key_cap.node.start_position().row as i64 + 1),
                direction, medium, key_raw, key_norm,
                local_var: None,
                raw_context: source_line(source, key_cap.node.start_position().row),
                prov_source: "ast".into(), prov_confidence: 0.90,
                prov_plugin: plugin_name.into(), prov_note: None,
            });
        }
    }
    Ok(events)
}

fn classify_boundary(pattern: &str) -> (String, String) {
    match pattern {
        "http_header_write" => ("http_header".into(), "write".into()),
        "http_header_read"  => ("http_header".into(), "read".into()),
        "env_read" | "env_read2" => ("env".into(), "read".into()),
        "redis_get"         => ("redis".into(), "read".into()),
        "redis_set"         => ("redis".into(), "write".into()),
        "redis_op"          => ("redis".into(), "read".into()),
        "ws_op"             => ("websocket".into(), "write".into()),
        "fs_op"             => ("filesystem".into(), "read".into()),
        "kafka_write"       => ("kafka".into(), "write".into()),
        "cookie_write"      => ("cookie".into(), "write".into()),
        "route"             => ("http_body".into(), "read".into()),
        "grpc_encode"       => ("grpc".into(), "write".into()),
        "grpc_decode"       => ("grpc".into(), "read".into()),
        _                   => ("unknown".into(), "read".into()),
    }
}

fn enclosing_function(node: Node, source: &[u8]) -> String {
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(n.kind(), "function_declaration" | "function" | "arrow_function" | "method_definition") {
            if let Some(name_node) = n.child_by_field_name("name") {
                return node_text(&name_node, source).to_string();
            }
        }
        current = n.parent();
    }
    "<module>".to_string()
}

fn source_line(source: &[u8], row: usize) -> Option<String> {
    let s = std::str::from_utf8(source).ok()?;
    s.lines().nth(row).map(|l| l.trim().to_string())
}

pub fn detect_express_routes(
    _tree: &tree_sitter::Tree,
    source: &[u8],
    file: &Path,
    plugin_name: &str,
) -> Result<Vec<EntryPointHint>> {
    detect_js_routes(_tree, source, file, plugin_name)
}

fn detect_js_routes(
    _tree: &tree_sitter::Tree,
    source: &[u8],
    file: &Path,
    plugin_name: &str,
) -> Result<Vec<EntryPointHint>> {
    let src = std::str::from_utf8(source).unwrap_or("");
    let file_str = file.to_string_lossy().to_string();
    let mut hints = Vec::new();

    // Collect file-level middleware (Express use middleware)
    let express_mw_re = once_cell::sync::Lazy::force(&crate::patterns::http::EXPRESS_USE_MW_RE);
    let mut file_middleware: Vec<String> = Vec::new();
    for cap in express_mw_re.captures_iter(src) {
        if let Some(m) = cap.get(1) {
            file_middleware.push(m.as_str().to_string());
        }
    }

    // Express: app.get/post/put/delete/use patterns via regex on source
    let re = once_cell::sync::Lazy::force(&crate::patterns::http::EXPRESS_ROUTE_RE);
    for cap in re.captures_iter(src) {
        let method = cap.get(1).map_or("", |m| m.as_str()).to_uppercase();
        let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
        let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
        if !handler.is_empty() {
            hints.push(EntryPointHint {
                fn_name: handler, file: file_str.clone(), line: None,
                kind: "route".into(), framework: Some("express".into()),
                path: Some(path), method: Some(method),
                confidence: 0.90, heuristic: "route_decorator".into(),
                middleware: file_middleware.clone(),
            });
        }
    }

    // Fastify routes
    let _ = plugin_name; // suppress unused warning
    let fastify_re = once_cell::sync::Lazy::force(&crate::patterns::http::FASTIFY_ROUTE_RE);
    for cap in fastify_re.captures_iter(src) {
        let method = cap.get(1).map_or("get", |m| m.as_str()).to_uppercase();
        let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
        let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
        if !handler.is_empty() {
            hints.push(EntryPointHint {
                fn_name: handler, file: file_str.clone(), line: None,
                kind: "route".into(), framework: Some("fastify".into()),
                path: Some(path), method: Some(method),
                confidence: 0.90, heuristic: "route_decorator".into(),
                middleware: file_middleware.clone(),
            });
        }
    }

    Ok(hints)
}
