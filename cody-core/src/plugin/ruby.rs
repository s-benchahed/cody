use std::path::Path;
use anyhow::Result;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct RubyPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/ruby/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/ruby/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/ruby/boundaries.scm");

impl LanguagePlugin for RubyPlugin {
    fn language_name(&self) -> &'static str { "ruby" }
    fn extensions(&self) -> &[&'static str] { &["rb"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_ruby::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("Ruby symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            let is_class = pattern == "class" || pattern == "module";
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    let is_exported = !name.starts_with('_');
                    symbols.push(Symbol {
                        id: None, name,
                        kind: if is_class { "class" } else { "function" }.to_string(),
                        file: file_str.clone(),
                        line: Some(cap.node.start_position().row as i64 + 1),
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
            .map_err(|e| anyhow::anyhow!("Ruby edges: {e}"))?;
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
                            id: None, src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "calls".into(), dst_file: None,
                            dst_symbol: Some(node_text(&cap.node, source).to_string()),
                            context: None, line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                "require" => {
                    let cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "path");
                    if let Some(cap) = cap {
                        let raw = node_text(&cap.node, source);
                        let path = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
                        edges.push(Edge {
                            id: None, src_file: Some(file_str.clone()), src_symbol: None,
                            rel: "imports".into(), dst_file: Some(path), dst_symbol: None,
                            context: None, line: Some(cap.node.start_position().row as i64 + 1),
                        });
                    }
                }
                "extends" => {
                    let child = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "child");
                    let parent = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "parent");
                    if let (Some(c), Some(p)) = (child, parent) {
                        edges.push(Edge {
                            id: None, src_file: Some(file_str.clone()),
                            src_symbol: Some(node_text(&c.node, source).to_string()),
                            rel: "extends".into(), dst_file: None,
                            dst_symbol: Some(node_text(&p.node, source).to_string()),
                            context: None, line: Some(c.node.start_position().row as i64 + 1),
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
            let key_cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "key");
            if let Some(key_cap) = key_cap {
                let key_raw = node_text(&key_cap.node, source)
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = ruby_classify(&pattern);
                events.push(BoundaryEvent {
                    id: None, fn_name: "<module>".into(), file: file_str.clone(),
                    line: Some(key_cap.node.start_position().row as i64 + 1),
                    direction, medium, key_raw, key_norm,
                    local_var: None, raw_context: None,
                    prov_source: "ast".into(), prov_confidence: 0.85,
                    prov_plugin: "ruby".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // Collect file-level middleware from before_action
        let before_action_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_BEFORE_ACTION_RE);
        let file_middleware: Vec<String> = before_action_re.captures_iter(src)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();

        // Rails routes: get/post/put/delete/patch at file level
        let re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_ROUTE_RE);
        for cap in re.captures_iter(src) {
            let method = cap.get(1).map_or("get", |m| m.as_str()).to_uppercase();
            let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let action = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !action.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: action, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("rails".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.88, heuristic: "route_decorator".into(),
                    middleware: file_middleware.clone(),
                });
            }
        }
        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() as i64 + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("def ").count() as i64;
        let imports = src.matches("require").count() as i64;
        Ok(FileMetaCounts { line_count: lines, export_count: exports, import_count: imports })
    }
}

fn ruby_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"          => ("env".into(), "read".into()),
        "redis_get"         => ("redis".into(), "read".into()),
        "redis_set"         => ("redis".into(), "write".into()),
        "redis_op"          => ("redis".into(), "read".into()),
        "http_header_write" => ("http_header".into(), "write".into()),
        "job_enqueue"       => ("kafka".into(), "write".into()),
        "grpc_encode"       => ("grpc".into(), "write".into()),
        "grpc_decode"       => ("grpc".into(), "read".into()),
        _                   => ("unknown".into(), "read".into()),
    }
}
