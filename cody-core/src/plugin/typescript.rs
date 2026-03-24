use std::path::Path;
use anyhow::Result;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct TypeScriptPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/typescript/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/typescript/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/typescript/boundaries.scm");

impl LanguagePlugin for TypeScriptPlugin {
    fn language_name(&self) -> &'static str { "typescript" }
    fn extensions(&self) -> &[&'static str] { &["ts", "tsx"] }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::language_typescript()
    }

    fn parse(&self, source: &[u8], path: &Path) -> Result<tree_sitter::Tree> {
        let lang = if path.extension().and_then(|e| e.to_str()) == Some("tsx") {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        };
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).map_err(|e| anyhow::anyhow!("{e}"))?;
        parser.parse(source, None).ok_or_else(|| anyhow::anyhow!("TS parse failed"))
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        };
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("TS symbols query: {e}"))?;
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
                    let kind = if cap_name.contains("class") || cap_name.contains("interface") {
                        "class"
                    } else {
                        "function"
                    }.to_string();
                    symbols.push(Symbol {
                        name, kind, file: file_str.clone(),
                        line: Some(cap.node.start_position().row as i64 + 1),
                        signature: None, is_exported: is_export,
                        prov_source: "ast".into(), prov_confidence: 0.95,
                    });
                }
            }
        }
        Ok(symbols)
    }

    fn extract_edges(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Edge>> {
        let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        };
        let query = Query::new(&lang, EDGES_SCM)
            .map_err(|e| anyhow::anyhow!("TS edges: {e}"))?;
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
                        edges.push(Edge {
                            src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "calls".into(), dst_file: None,
                            dst_symbol: Some(node_text(&cap.node, source).to_string()),
                            context: None,
                            line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                "import" => {
                    let cap = m.captures.iter().find(|c| {
                        query.capture_names()[c.index as usize] == "import_path"
                    });
                    if let Some(cap) = cap {
                        let raw = node_text(&cap.node, source);
                        let path = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
                        edges.push(Edge {
                            src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "imports".into(), dst_file: Some(path), dst_symbol: None,
                            context: None, line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(edges)
    }

    fn extract_boundary_events(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<BoundaryEvent>> {
        // TS boundary patterns are the same as JS — reuse JS extractor with TS SCM
        let lang = if file.extension().and_then(|e| e.to_str()) == Some("tsx") {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        };
        let query = match Query::new(&lang, BOUNDARIES_SCM) {
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
                let (medium, direction) = classify(&pattern_name);
                events.push(BoundaryEvent {
                    fn_name: "<module>".into(), file: file_str.clone(),
                    line: Some(key_cap.node.start_position().row as i64 + 1),
                    direction, medium, key_raw, key_norm,
                    local_var: None, raw_context: None,
                    prov_source: "ast".into(), prov_confidence: 0.90,
                    prov_plugin: "typescript".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // Collect file-level middleware (NestJS guards + Express use middleware)
        let nestjs_guard_re = once_cell::sync::Lazy::force(&crate::patterns::http::NESTJS_GUARD_RE);
        let express_mw_re = once_cell::sync::Lazy::force(&crate::patterns::http::EXPRESS_USE_MW_RE);
        let mut file_middleware: Vec<String> = Vec::new();
        for cap in nestjs_guard_re.captures_iter(src) {
            if let Some(g) = cap.get(1) {
                file_middleware.push(g.as_str().to_string());
            }
        }
        for cap in express_mw_re.captures_iter(src) {
            if let Some(m) = cap.get(1) {
                file_middleware.push(m.as_str().to_string());
            }
        }

        // Express routes
        let express_re = once_cell::sync::Lazy::force(&crate::patterns::http::EXPRESS_ROUTE_RE);
        for cap in express_re.captures_iter(src) {
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

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() as i64 + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("export ").count() as i64;
        let imports = src.matches("import ").count() as i64;
        Ok(FileMetaCounts { lines: lines as usize, exports: exports as usize, imports: imports as usize })
    }
}

fn classify(pattern: &str) -> (String, String) {
    match pattern {
        "http_header_write" => ("http_header".into(), "write".into()),
        "env_read"          => ("env".into(), "read".into()),
        "redis_get"         => ("redis".into(), "read".into()),
        "redis_set"         => ("redis".into(), "write".into()),
        "redis_op"          => ("redis".into(), "read".into()),
        "ws_op"             => ("websocket".into(), "write".into()),
        "grpc_encode"       => ("grpc".into(), "write".into()),
        "grpc_decode"       => ("grpc".into(), "read".into()),
        _                   => ("unknown".into(), "read".into()),
    }
}
