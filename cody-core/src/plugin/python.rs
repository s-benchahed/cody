use std::path::Path;
use anyhow::Result;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct PythonPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/python/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/python/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/python/boundaries.scm");

impl LanguagePlugin for PythonPlugin {
    fn language_name(&self) -> &'static str { "python" }
    fn extensions(&self) -> &[&'static str] { &["py"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_python::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("Python symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = &query.capture_names()[m.pattern_index];
            let is_class = pattern.contains("class");
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    // In Python, exported = not starting with _
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
            .map_err(|e| anyhow::anyhow!("Python edges: {e}"))?;
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
                "import" | "from_import" => {
                    let cap = m.captures.iter().find(|c| {
                        let n = &query.capture_names()[c.index as usize];
                        *n == "import_path" || *n == "module"
                    });
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
            let key_cap = m.captures.iter().find(|c| {
                let n = &query.capture_names()[c.index as usize];
                *n == "key" || *n == "route_path" || *n == "url" || *n == "topic"
            });
            if let Some(key_cap) = key_cap {
                let key_raw = node_text(&key_cap.node, source)
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = py_classify(&pattern);
                let fn_cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "handler");
                let fn_name = fn_cap.map(|c| node_text(&c.node, source).to_string())
                    .unwrap_or_else(|| "<module>".to_string());
                events.push(BoundaryEvent {
                    id: None, fn_name, file: file_str.clone(),
                    line: Some(key_cap.node.start_position().row as i64 + 1),
                    direction, medium, key_raw, key_norm,
                    local_var: None, raw_context: None,
                    prov_source: "ast".into(), prov_confidence: 0.90,
                    prov_plugin: "python".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // Detect Flask/FastAPI routes via regex on source
        let re = once_cell::sync::Lazy::force(&crate::patterns::http::FASTAPI_ROUTE_RE);
        for cap in re.captures_iter(src) {
            let method = cap.get(1).map_or("route", |m| m.as_str()).to_uppercase();
            let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("fastapi/flask".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // Django urlpatterns: path('url/', view_fn) or re_path(r'...', view_fn)
        let django_re = once_cell::sync::Lazy::force(&crate::patterns::http::DJANGO_URL_RE);
        for cap in django_re.captures_iter(src) {
            let path = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(2).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("django".into()),
                    path: Some(path), method: None,
                    confidence: 0.88, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // FastAPI Depends middleware detection
        let depends_re = once_cell::sync::Lazy::force(&crate::patterns::http::FASTAPI_DEPENDS_RE);
        for cap in depends_re.captures_iter(src) {
            let fn_name = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let dep_fn = cap.get(2).map_or("", |m| m.as_str()).to_string();
            if fn_name.is_empty() || dep_fn.is_empty() { continue; }
            // Collect all Depends(...) from the full match
            let full_match = cap.get(0).map_or("", |m| m.as_str());
            let mw: Vec<String> = {
                // Re-scan for all Depends(...) dep names in this function signature
                let depends_inner = regex::Regex::new(r#"Depends\s*\(\s*(\w+)\s*\)"#).unwrap();
                depends_inner.captures_iter(full_match)
                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .collect()
            };
            if let Some(existing) = hints.iter_mut().find(|h| h.fn_name == fn_name) {
                existing.middleware = mw;
            } else {
                hints.push(EntryPointHint {
                    fn_name, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("fastapi".into()),
                    path: None, method: None,
                    confidence: 0.80, heuristic: "depends_fn".into(), middleware: mw,
                });
            }
        }

        // Detect def main() and __main__
        if src.contains("def main(") || src.contains("if __name__ == \"__main__\"") {
            hints.push(EntryPointHint {
                fn_name: "main".into(), file: file_str.clone(), line: None,
                kind: "main".into(), framework: None, path: None, method: None,
                confidence: 0.80, heuristic: "main_fn".into(), middleware: vec![],
            });
        }

        // Celery task decorator
        if src.contains("@celery") || src.contains("@app.task") || src.contains("@shared_task") {
            let re2 = once_cell::sync::Lazy::force(&crate::patterns::http::CELERY_TASK_RE);
            for cap in re2.captures_iter(src) {
                let handler = cap.get(1).map_or("", |m| m.as_str()).to_string();
                if !handler.is_empty() {
                    hints.push(EntryPointHint {
                        fn_name: handler, file: file_str.clone(), line: None,
                        kind: "cron".into(), framework: Some("celery".into()),
                        path: None, method: None,
                        confidence: 0.85, heuristic: "cron".into(), middleware: vec![],
                    });
                }
            }
        }
        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() as i64 + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("def ").count() as i64;
        let imports = src.matches("import ").count() as i64 + src.matches("from ").count() as i64;
        Ok(FileMetaCounts { line_count: lines, export_count: exports, import_count: imports })
    }
}

fn py_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read" | "env_read2"  => ("env".into(), "read".into()),
        "redis_get"               => ("redis".into(), "read".into()),
        "redis_set"               => ("redis".into(), "write".into()),
        "redis_op"                => ("redis".into(), "read".into()),
        "http_call"               => ("http_body".into(), "write".into()),
        "route"                   => ("http_body".into(), "read".into()),
        "kafka_write"             => ("kafka".into(), "write".into()),
        "cookie_write"            => ("cookie".into(), "write".into()),
        "grpc_encode"             => ("grpc".into(), "write".into()),
        "grpc_decode"             => ("grpc".into(), "read".into()),
        _                         => ("unknown".into(), "read".into()),
    }
}
