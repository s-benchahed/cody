use std::path::Path;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct PhpPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/php/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/php/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/php/boundaries.scm");

/// Match Laravel Route facade registrations:
/// Route::get('/path', ...) or Route::post("/path", ...)
static PHP_LARAVEL_ROUTE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Route::(get|post|put|delete|patch|any|match)\s*\(\s*(?:'([^']*)'|"([^"]*)")"#)
        .unwrap()
});

/// Match Artisan command signature: protected $signature = 'command:name';
static PHP_ARTISAN_SIGNATURE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"protected\s+\$signature\s*="#).unwrap()
});

impl LanguagePlugin for PhpPlugin {
    fn language_name(&self) -> &'static str { "php" }
    fn extensions(&self) -> &[&'static str] { &["php"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_php::language_php()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("PHP symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = &query.capture_names()[m.pattern_index];
            let is_class = *pattern == "class" || *pattern == "interface";
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    // PHP: heuristic — default to exported (public is common)
                    // We could inspect visibility modifiers but default true is safe
                    symbols.push(Symbol {
                        name,
                        kind: if is_class { "class" } else { "function" }.to_string(),
                        file: file_str.clone(),
                        line: Some(cap.node.start_position().row as i64 + 1),
                        signature: None,
                        is_exported: true,
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
            .map_err(|e| anyhow::anyhow!("PHP edges: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut edges = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            match pattern.as_str() {
                "call" | "method_call" | "static_call" => {
                    let cap = m.captures.iter().find(|c| {
                        let n = &query.capture_names()[c.index as usize];
                        *n == "callee" || *n == "method"
                    });
                    if let Some(cap) = cap {
                        let name = node_text(&cap.node, source).to_string();
                        if !name.is_empty() {
                            edges.push(Edge {
                                src_file: Some(file_str.clone()), src_symbol: None,
                                rel: "calls".into(), dst_file: None,
                                dst_symbol: Some(name),
                                context: None,
                                line: Some(cap.node.start_position().row as i64 + 1),
                            });
                        }
                    }
                }
                "import" => {
                    let cap = m.captures.iter().find(|c| {
                        query.capture_names()[c.index as usize] == "import_path"
                    });
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
                *n == "key" || *n == "path"
            });

            if let Some(key_cap) = key_cap {
                let raw_text = node_text(&key_cap.node, source);
                let key_raw = raw_text
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string();
                if key_raw.is_empty() { continue; }

                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = php_classify(&pattern);

                // For SQL: refine read/write from verb, and extract table name as key
                let (medium, direction, key_norm) = if medium == "sql" && pattern == "sql_query" {
                    let sql_upper = key_raw.trim_start().to_uppercase();
                    let dir = if sql_upper.starts_with("SELECT") || sql_upper.starts_with("WITH") {
                        "read"
                    } else if sql_upper.starts_with("INSERT") || sql_upper.starts_with("UPDATE")
                        || sql_upper.starts_with("DELETE") || sql_upper.starts_with("MERGE")
                    {
                        "write"
                    } else {
                        direction.as_str()
                    };
                    let table = sql_table_name(&key_raw);
                    let norm = if table.is_empty() { key_norm } else { table };
                    ("sql".to_string(), dir.to_string(), norm)
                } else {
                    (medium, direction, key_norm)
                };

                events.push(BoundaryEvent {
                    fn_name: "<module>".into(),
                    file: file_str.clone(),
                    line: Some(key_cap.node.start_position().row as i64 + 1),
                    direction,
                    medium,
                    key_raw,
                    key_norm,
                    local_var: None,
                    raw_context: None,
                    prov_source: "ast".into(),
                    prov_confidence: 0.90,
                    prov_plugin: "php".into(),
                    prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // Laravel Route facade: Route::get('/path', ...) etc.
        for cap in PHP_LARAVEL_ROUTE_RE.captures_iter(src) {
            let method = cap.get(1).map_or("get", |m| m.as_str()).to_uppercase();
            // Path may be single-quoted (group 2) or double-quoted (group 3)
            let path = cap.get(2)
                .or_else(|| cap.get(3))
                .map_or("", |m| m.as_str())
                .to_string();
            if !path.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: "<route>".into(),
                    file: file_str.clone(),
                    line: None,
                    kind: "route".into(),
                    framework: Some("laravel".into()),
                    path: Some(path),
                    method: Some(method),
                    confidence: 0.90,
                    heuristic: "route_facade".into(),
                    middleware: vec![],
                });
            }
        }

        // Artisan commands: files with `protected $signature`
        if PHP_ARTISAN_SIGNATURE_RE.is_match(src) {
            hints.push(EntryPointHint {
                fn_name: "<artisan_command>".into(),
                file: file_str.clone(),
                line: None,
                kind: "cron".into(),
                framework: Some("artisan".into()),
                path: None,
                method: None,
                confidence: 0.85,
                heuristic: "artisan_signature".into(),
                middleware: vec![],
            });
        }

        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("public function ").count() + src.matches("function ").count();
        let imports = src.matches("use ").count() + src.matches("require").count();
        Ok(FileMetaCounts { lines, exports, imports })
    }
}

fn php_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"              => ("env".into(), "read".into()),
        "sql_query" | "sql_table" => ("sql".into(), "read".into()),
        "redis_get"             => ("redis".into(), "read".into()),
        "redis_set"             => ("redis".into(), "write".into()),
        "http_out"              => ("http_out".into(), "write".into()),
        "route"                 => ("http_body".into(), "read".into()),
        "kafka_write"           => ("kafka".into(), "write".into()),
        _                       => ("unknown".into(), "read".into()),
    }
}

/// Extract the primary table name from a SQL string for display purposes.
fn sql_table_name(sql: &str) -> String {
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
