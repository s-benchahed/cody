use std::path::Path;
use anyhow::Result;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct GoPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/go_lang/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/go_lang/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/go_lang/boundaries.scm");

impl LanguagePlugin for GoPlugin {
    fn language_name(&self) -> &'static str { "go" }
    fn extensions(&self) -> &[&'static str] { &["go"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_go::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("Go symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            let is_class = pattern == "struct";
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    // Go: exported = starts with uppercase letter
                    let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    symbols.push(Symbol {
                        name,
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
            .map_err(|e| anyhow::anyhow!("Go edges: {e}"))?;
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
                "import" => {
                    let cap = m.captures.iter().find(|c| {
                        query.capture_names()[c.index as usize] == "import_path"
                    });
                    if let Some(cap) = cap {
                        let path_raw = node_text(&cap.node, source).trim_matches('"').to_string();
                        if !path_raw.is_empty() {
                            edges.push(Edge {
                                src_file: Some(file_str.clone()), src_symbol: None,
                                rel: "imports".into(),
                                dst_file: Some(path_raw),
                                dst_symbol: None, context: None,
                                line: Some(cap.node.start_position().row as i64 + 1),
                            });
                        }
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
                *n == "key" || *n == "path"
            });
            if let Some(key_cap) = key_cap {
                let raw_text = node_text(&key_cap.node, source);
                // Strip Go interpreted string literal quotes
                let key_raw = raw_text.trim_matches('"').to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = go_classify(&pattern);
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
                } else {
                    (medium, direction)
                };
                // For SQL: use table name as the display key, not the full query string
                let key_norm = if medium == "sql" {
                    let t = sql_table_name_go(&key_raw);
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
                    prov_plugin: "go".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // func main()
        if src.contains("func main(") {
            hints.push(EntryPointHint {
                fn_name: "main".into(), file: file_str.clone(), line: None,
                kind: "main".into(), framework: None, path: None, method: None,
                confidence: 0.95, heuristic: "main_fn".into(), middleware: vec![],
            });
        }

        // net/http: http.HandleFunc("/path", handler)
        let http_re = once_cell::sync::Lazy::force(&crate::patterns::http::GO_HTTP_HANDLE_RE);
        for cap in http_re.captures_iter(src) {
            let path = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(2).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("net/http".into()),
                    path: Some(path), method: None,
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // gin/chi/echo/fiber: .GET("/path", handler) / .Post("/path", handler) etc.
        let fw_re = once_cell::sync::Lazy::force(&crate::patterns::http::GO_FRAMEWORK_ROUTE_RE);
        for cap in fw_re.captures_iter(src) {
            let method = cap.get(1).map_or("GET", |m| m.as_str()).to_uppercase();
            let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("gin/chi/echo/fiber".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("\nfunc ").count();
        let imports = src.matches("import ").count();
        Ok(FileMetaCounts { lines, exports, imports })
    }
}

/// Extract the primary table name from a SQL string for display purposes.
/// e.g. "SELECT id FROM users WHERE ..." → "users"
///      "INSERT INTO pacts (...) VALUES ..." → "pacts"
fn sql_table_name_go(sql: &str) -> String {
    let s = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let upper = s.to_uppercase();

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

    after.split_whitespace().next()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
        .to_lowercase()
}

fn go_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"   => ("env".into(), "read".into()),
        "sql_query"  => ("sql".into(), "read".into()),
        "redis_get"  => ("redis".into(), "read".into()),
        "redis_set"  => ("redis".into(), "write".into()),
        "route"      => ("http_body".into(), "read".into()),
        "grpc_decode" => ("grpc".into(), "read".into()),
        _            => ("unknown".into(), "read".into()),
    }
}
