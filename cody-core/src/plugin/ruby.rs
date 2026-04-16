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
                            src_file: Some(file_str.clone()), src_symbol: None,
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
                            src_file: Some(file_str.clone()), src_symbol: None,
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
                            src_file: Some(file_str.clone()),
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
                let raw_text = node_text(&key_cap.node, source);
                // Strip string quotes and leading : from symbols
                let key_raw = raw_text
                    .trim_matches(|c| c == '"' || c == '\'')
                    .trim_start_matches(':')
                    .to_string();
                if key_raw.is_empty() { continue; }

                let (medium, direction) = ruby_classify(&pattern);

                // For ActiveRecord, refine read/write from the method name captured as @method
                let (medium, direction) = if matches!(pattern.as_str(), "ar_read" | "ar_write" | "sql_raw") {
                    let method_cap = m.captures.iter().find(|c| query.capture_names()[c.index as usize] == "method");
                    let method_name = method_cap.map(|c| node_text(&c.node, source)).unwrap_or("");
                    let is_write = matches!(method_name,
                        "create" | "create!" | "insert" | "insert!" | "insert_all" | "insert_all!" |
                        "upsert" | "upsert_all" | "update_all" | "delete_all" | "destroy_all" |
                        "destroy_by" | "delete_by" | "exec_update" | "exec_insert" | "exec_delete"
                    ) || (pattern == "sql_raw" && key_raw.trim_start().to_uppercase().starts_with("INSERT"))
                      || (pattern == "sql_raw" && key_raw.trim_start().to_uppercase().starts_with("UPDATE"))
                      || (pattern == "sql_raw" && key_raw.trim_start().to_uppercase().starts_with("DELETE"));
                    // For sql_raw, extract table name from SQL string
                    if pattern == "sql_raw" {
                        let table = sql_table_name_ruby(&key_raw);
                        let k = if table.is_empty() { key_raw.clone() } else { table };
                        let norm = crate::patterns::normalise_key(&k);
                        events.push(BoundaryEvent {
                            fn_name: "<module>".into(), file: file_str.clone(),
                            line: Some(key_cap.node.start_position().row as i64 + 1),
                            direction: if is_write { "write".into() } else { "read".into() },
                            medium: "sql".into(), key_raw: k.clone(), key_norm: norm,
                            local_var: None, raw_context: None,
                            prov_source: "ast".into(), prov_confidence: 0.85,
                            prov_plugin: "ruby".into(), prov_note: None,
                        });
                        continue;
                    }
                    ("sql".to_string(), if is_write { "write".to_string() } else { "read".to_string() })
                } else {
                    (medium, direction)
                };

                let key_norm = crate::patterns::normalise_key(&key_raw);
                events.push(BoundaryEvent {
                    fn_name: "<module>".into(), file: file_str.clone(),
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

        // ── Collect file-level middleware from before_action ───────────────────
        let before_action_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_BEFORE_ACTION_RE);
        let file_middleware: Vec<String> = before_action_re.captures_iter(src)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
            .collect();

        // ── Rails explicit routes (colon style): get '/path', to: 'c#action' ──
        let re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_ROUTE_RE);
        for cap in re.captures_iter(src) {
            let method = cap.get(1).map_or("GET", |m| m.as_str()).to_uppercase();
            let path   = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let action = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !action.is_empty() {
                hints.push(EntryPointHint {
                    fn_name: action, file: file_str.clone(), line: None,
                    kind: "route".into(), framework: Some("rails".into()),
                    path: Some(path), method: Some(method),
                    confidence: 0.90, heuristic: "route_decorator".into(),
                    middleware: file_middleware.clone(),
                });
            }
        }

        // ── Rails hash-rocket routes: get '/path' => 'c#action' ───────────────
        let rocket_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_ROUTE_ROCKET_RE);
        for cap in rocket_re.captures_iter(src) {
            let method = cap.get(1).map_or("GET", |m| m.as_str()).to_uppercase();
            let path   = cap.get(2).map_or("", |m| m.as_str()).to_string();
            let action = cap.get(3).map_or("", |m| m.as_str()).to_string();
            if !action.is_empty() {
                let already = hints.iter().any(|h| h.path.as_deref() == Some(&path) && h.fn_name == action);
                if !already {
                    hints.push(EntryPointHint {
                        fn_name: action, file: file_str.clone(), line: None,
                        kind: "route".into(), framework: Some("rails".into()),
                        path: Some(path), method: Some(method),
                        confidence: 0.90, heuristic: "route_decorator".into(),
                        middleware: file_middleware.clone(),
                    });
                }
            }
        }

        // ── Rails resources expansion: resources :users / resource :session ───
        // Expands to 7 (or 6 for singular) RESTful routes against the controller.
        let res_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_RESOURCES_RE);
        for cap in res_re.captures_iter(src) {
            let is_plural = cap.get(1).map_or("", |m| m.as_str()) == "resources";
            let model = cap.get(2).map_or("", |m| m.as_str());
            if model.is_empty() { continue; }
            let ctrl = format!("{}#", model); // prefix like "users#"
            let base = format!("/{}", model);
            // Standard RESTful actions
            let mut routes: Vec<(&str, String, &str)> = vec![
                ("POST",   base.clone(),               "create"),
                ("GET",    format!("{}/new", base),    "new"),
                ("GET",    format!("{}/edit", base),   "edit"),  // singular: /session/edit
                ("GET",    base.clone(),               "show"),  // singular: /session
                ("PATCH",  base.clone(),               "update"),
                ("DELETE", base.clone(),               "destroy"),
            ];
            if is_plural {
                // Replace singular paths with plural + :id variants
                routes = vec![
                    ("GET",    base.clone(),                    "index"),
                    ("POST",   base.clone(),                    "create"),
                    ("GET",    format!("{}/new", base),         "new"),
                    ("GET",    format!("{}/:id/edit", base),    "edit"),
                    ("GET",    format!("{}/:id", base),         "show"),
                    ("PATCH",  format!("{}/:id", base),         "update"),
                    ("DELETE", format!("{}/:id", base),         "destroy"),
                ];
            }
            for (method, path, action) in routes {
                let fn_name = format!("{}{}", ctrl, action);
                let already = hints.iter().any(|h| h.fn_name == fn_name);
                if !already {
                    hints.push(EntryPointHint {
                        fn_name, file: file_str.clone(), line: None,
                        kind: "route".into(), framework: Some("rails".into()),
                        path: Some(path), method: Some(method.into()),
                        confidence: 0.85, heuristic: "resources".into(),
                        middleware: file_middleware.clone(),
                    });
                }
            }
        }

        // ── Controller action detection ────────────────────────────────────────
        // When processing a controller file, detect public def action_name methods
        // as route entry points. Heuristic: file path contains "controllers" and
        // class inherits from ApplicationController or *Controller.
        let is_controller = file_str.contains("controllers") || src.contains("< ApplicationController");
        let has_controller_class = src.contains("< ApplicationController")
            || src.contains("< Api::")
            || src.contains("Controller");
        if is_controller && has_controller_class {
            let class_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_CONTROLLER_CLASS_RE);
            let action_re = once_cell::sync::Lazy::force(&crate::patterns::http::RAILS_ACTION_DEF_RE);

            // Infer controller prefix from class name (UsersController → users)
            let ctrl_prefix = class_re.captures(src)
                .and_then(|c| c.get(1))
                .map(|m| {
                    let name = m.as_str();
                    // UsersController → users, Api::UsersController → users
                    let base = name.split("::").last().unwrap_or(name);
                    base.trim_end_matches("Controller")
                        .chars()
                        .enumerate()
                        .map(|(i, c)| {
                            if i > 0 && c.is_uppercase() { format!("_{}", c.to_lowercase()) }
                            else { c.to_lowercase().to_string() }
                        })
                        .collect::<String>()
                })
                .unwrap_or_default();

            // Scan lines before `private` keyword for public action defs
            let private_pos = src.find("\n  private").or_else(|| src.find("\nprivate")).unwrap_or(src.len());
            let public_section = &src[..private_pos];

            for cap in action_re.captures_iter(public_section) {
                let action = cap.get(1).map_or("", |m| m.as_str());
                // Skip initialize, before_action helpers, and private-looking names
                if action.starts_with('_') || matches!(action, "initialize" | "new" | "helper_method") {
                    continue;
                }
                let fn_name = if ctrl_prefix.is_empty() {
                    action.to_string()
                } else {
                    format!("{}#{}", ctrl_prefix, action)
                };
                let already = hints.iter().any(|h| h.fn_name == fn_name || h.fn_name == action);
                if !already {
                    hints.push(EntryPointHint {
                        fn_name, file: file_str.clone(), line: None,
                        kind: "route".into(), framework: Some("rails".into()),
                        path: None, method: None,
                        confidence: 0.80, heuristic: "controller_action".into(),
                        middleware: file_middleware.clone(),
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
        let imports = src.matches("require").count() as i64;
        Ok(FileMetaCounts { lines: lines as usize, exports: exports as usize, imports: imports as usize })
    }
}

fn ruby_classify(pattern: &str) -> (String, String) {
    match pattern {
        "env_read"          => ("env".into(),        "read".into()),
        "redis_get"         => ("redis".into(),      "read".into()),
        "redis_set"         => ("redis".into(),      "write".into()),
        "redis_op"          => ("redis".into(),      "read".into()),
        "http_header_write" => ("http_header".into(),"write".into()),
        "http_out"          => ("http_out".into(),   "write".into()),
        "ar_read"           => ("sql".into(),        "read".into()),
        "ar_write"          => ("sql".into(),        "write".into()),
        "sql_raw"           => ("sql".into(),        "read".into()),  // direction refined in extract_boundary_events
        "job_enqueue"       => ("queue".into(),      "write".into()),
        "job_queue"         => ("queue".into(),      "write".into()),
        "kafka_write"       => ("kafka".into(),      "write".into()),
        "grpc_encode"       => ("grpc".into(),       "write".into()),
        "grpc_decode"       => ("grpc".into(),       "read".into()),
        _                   => ("unknown".into(),    "read".into()),
    }
}

/// Extract the primary table name from a SQL string (same logic as Rust plugin).
fn sql_table_name_ruby(sql: &str) -> String {
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
