use std::path::Path;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter::{Query, QueryCursor};
use crate::db::models::*;
use crate::plugin::{LanguagePlugin, node_text};

pub struct JavaPlugin;

static SYMBOLS_SCM:    &str = include_str!("../../queries/java/symbols.scm");
static EDGES_SCM:      &str = include_str!("../../queries/java/edges.scm");
static BOUNDARIES_SCM: &str = include_str!("../../queries/java/boundaries.scm");

// Spring MVC: @GetMapping("/path") ... public ReturnType handlerMethod(
static SPRING_MAPPING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*\(\s*(?:value\s*=\s*)?"([^"]*)"\s*\)[^{]*?(?:public|protected)\s+\S+\s+(\w+)\s*\("#).unwrap()
});

// @KafkaListener(topics = "topic-name") ... public void handlerMethod(
static KAFKA_LISTENER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"@KafkaListener\s*\([^)]*topics\s*=\s*"([^"]*)"[^)]*\)[^{]*?(?:public|protected)\s+\S+\s+(\w+)\s*\("#).unwrap()
});

impl LanguagePlugin for JavaPlugin {
    fn language_name(&self) -> &'static str { "java" }
    fn extensions(&self) -> &[&'static str] { &["java"] }
    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_java::language()
    }

    fn extract_symbols(&self, tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<Symbol>> {
        let lang = self.tree_sitter_language();
        let query = Query::new(&lang, SYMBOLS_SCM)
            .map_err(|e| anyhow::anyhow!("Java symbols: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut symbols = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = &query.capture_names()[m.pattern_index];
            let is_class = matches!(pattern.as_ref(), "class" | "interface" | "enum");
            for cap in m.captures {
                if query.capture_names()[cap.index as usize] == "name" {
                    let name = node_text(&cap.node, source).to_string();
                    if name.is_empty() { continue; }
                    // Default to exported = true; Java package-private is the default
                    // but without deeper parsing we can't easily detect private modifier.
                    let is_exported = true;
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
            .map_err(|e| anyhow::anyhow!("Java edges: {e}"))?;
        let mut cursor = QueryCursor::new();
        let mut edges = Vec::new();
        let file_str = file.to_string_lossy().to_string();

        for m in cursor.matches(&query, tree.root_node(), source) {
            let pattern = query.capture_names()[m.pattern_index].to_string();
            match pattern.as_str() {
                "method_call" | "new_call" => {
                    let cap = m.captures.iter().find(|c| {
                        let n = &query.capture_names()[c.index as usize];
                        *n == "method" || *n == "callee"
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
                query.capture_names()[c.index as usize] == "key"
            });
            if let Some(key_cap) = key_cap {
                let raw_text = node_text(&key_cap.node, source);
                // Strip Java string literal quotes
                let key_raw = raw_text.trim_matches('"').to_string();
                if key_raw.is_empty() { continue; }
                let key_norm = crate::patterns::normalise_key(&key_raw);
                let (medium, direction) = java_classify(&pattern);
                // For SQL: infer read vs write from the SQL verb
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
                // For SQL: use table name as the display key
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
                    prov_plugin: "java".into(), prov_note: None,
                });
            }
        }
        Ok(events)
    }

    fn entry_point_hints(&self, _tree: &tree_sitter::Tree, source: &[u8], file: &Path) -> Result<Vec<EntryPointHint>> {
        let src = std::str::from_utf8(source).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();
        let mut hints = Vec::new();

        // Detect framework from file content
        let framework = if src.contains("@RestController") || src.contains("@Controller") {
            Some("spring-mvc".to_string())
        } else if src.contains("@KafkaListener") {
            Some("spring-kafka".to_string())
        } else {
            None
        };

        // Spring MVC route annotations
        for cap in SPRING_MAPPING_RE.captures_iter(src) {
            let http_verb = cap.get(1).map_or("GET", |m| m.as_str()).to_uppercase();
            let path = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "route".into(),
                    framework: framework.clone().or_else(|| Some("spring-mvc".to_string())),
                    path: Some(path), method: Some(http_verb),
                    confidence: 0.90, heuristic: "route_decorator".into(), middleware: vec![],
                });
            }
        }

        // @KafkaListener entry points
        for cap in KAFKA_LISTENER_RE.captures_iter(src) {
            let topic = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let handler = cap.get(2).map_or("", |m| m.as_str()).to_string();
            if !handler.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: handler, file: file_str.clone(), line: None,
                    kind: "cron".into(), framework: Some("spring-kafka".to_string()),
                    path: Some(topic), method: None,
                    confidence: 0.90, heuristic: "kafka_listener".into(), middleware: vec![],
                });
            }
        }

        // public static void main(
        if src.contains("public static void main(") {
            hints.push(EntryPointHint {
                fn_name: "main".into(), file: file_str.clone(), line: None,
                kind: "main".into(), framework: None, path: None, method: None,
                confidence: 0.95, heuristic: "main_fn".into(), middleware: vec![],
            });
        }

        Ok(hints)
    }

    fn file_meta_counts(&self, _tree: &tree_sitter::Tree, source: &[u8]) -> Result<FileMetaCounts> {
        let lines = source.iter().filter(|&&b| b == b'\n').count() + 1;
        let src = std::str::from_utf8(source).unwrap_or("");
        let exports = src.matches("public ").count();
        let imports = src.matches("import ").count();
        Ok(FileMetaCounts { lines, exports, imports })
    }
}

fn java_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"     => ("env".into(), "read".into()),
        "sql_query"    => ("sql".into(), "read".into()),
        "redis_get"    => ("redis".into(), "read".into()),
        "redis_set"    => ("redis".into(), "write".into()),
        "kafka_write"  => ("kafka".into(), "write".into()),
        "kafka_listen" => ("kafka".into(), "read".into()),
        "http_out"     => ("http_out".into(), "write".into()),
        "route"        => ("http_body".into(), "read".into()),
        _              => ("unknown".into(), "read".into()),
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
