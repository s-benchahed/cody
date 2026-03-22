use std::path::Path;
use crate::db::models::{BoundaryEvent, Symbol};
use crate::patterns::{self, normalise_key};
use crate::patterns::sql;

/// Compute 1-based line number of a byte offset within a UTF-8 source string.
fn line_of(src: &str, byte_offset: usize) -> i64 {
    src[..byte_offset.min(src.len())].chars().filter(|&c| c == '\n').count() as i64 + 1
}

/// Run all regex-based boundary patterns against raw source.
/// Returns events not already found by AST queries.
pub fn extract(source: &[u8], file: &Path, language: &str, _symbols: &[Symbol]) -> Vec<BoundaryEvent> {
    let Ok(src) = std::str::from_utf8(source) else { return vec![]; };
    let file_str = file.to_string_lossy().to_string();
    let mut events: Vec<BoundaryEvent> = Vec::new();

    // ── ENV vars ───────────────────────────────────────────────────────────
    let env_re = match language {
        "javascript" | "typescript" => Some(&*patterns::env::JS_ENV_RE),
        "ruby"  => Some(&*patterns::env::RUBY_ENV_RE),
        "rust"  => Some(&*patterns::env::RUST_ENV_RE),
        _ => None,
    };
    if let Some(re) = env_re {
        for cap in re.captures_iter(src) {
            let key_raw = first_group(&cap).to_string();
            if key_raw.is_empty() { continue; }
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "env", &key_raw, language, None));
        }
    }
    if language == "python" {
        for cap in patterns::env::PY_ENV_RE.captures_iter(src) {
            let key_raw = first_group(&cap).to_string();
            if !key_raw.is_empty() {
                let line = cap.get(0).map(|m| line_of(src, m.start()));
                events.push(make_event("<module>", &file_str, line, "read", "env", &key_raw, language, None));
            }
        }
    }

    // ── Redis ──────────────────────────────────────────────────────────────
    for cap in patterns::redis::REDIS_GET_RE.captures_iter(src) {
        if let Some(key) = cap.get(2) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "redis", key.as_str(), language, None));
        }
    }
    for cap in patterns::redis::REDIS_SET_RE.captures_iter(src) {
        if let Some(key) = cap.get(2) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "redis", key.as_str(), language, None));
        }
    }

    // ── Kafka ──────────────────────────────────────────────────────────────
    for cap in patterns::kafka::KAFKA_SEND_JS_RE.captures_iter(src) {
        if let Some(topic) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "kafka", topic.as_str(), language, None));
        }
    }
    for cap in patterns::kafka::KAFKA_SUBSCRIBE_JS_RE.captures_iter(src) {
        if let Some(topic) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "kafka", topic.as_str(), language, None));
        }
    }
    for cap in patterns::kafka::KAFKA_PRODUCE_PY_RE.captures_iter(src) {
        if let Some(topic) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "kafka", topic.as_str(), language, None));
        }
    }
    for cap in patterns::kafka::KAFKA_SUBSCRIBE_PY_RE.captures_iter(src) {
        if let Some(topic) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "kafka", topic.as_str(), language, None));
        }
    }

    // ── SQL ────────────────────────────────────────────────────────────────
    for cap in sql::DB_QUERY_RE.captures_iter(src) {
        if let Some(query_str) = cap.get(2) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            for table in sql::extract_tables(query_str.as_str()) {
                let direction = if query_str.as_str().to_uppercase().contains("SELECT") { "read" } else { "write" };
                events.push(make_event("<module>", &file_str, line, direction, "sql", &table, language, None));
            }
        }
    }
    for cap in sql::PRISMA_RE.captures_iter(src) {
        if let (Some(table), Some(op)) = (cap.get(1), cap.get(2)) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            let direction = if op.as_str().starts_with("find") || op.as_str() == "count" { "read" } else { "write" };
            events.push(make_event("<module>", &file_str, line, direction, "sql", table.as_str(), language, None));
        }
    }
    for cap in sql::DJANGO_RE.captures_iter(src) {
        if let Some(model) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            let op = cap.get(2).map_or("", |m| m.as_str());
            let direction = if matches!(op, "filter" | "get" | "all" | "first" | "last") { "read" } else { "write" };
            events.push(make_event("<module>", &file_str, line, direction, "sql", model.as_str(), language, None));
        }
    }
    for cap in sql::SQLALCHEMY_RE.captures_iter(src) {
        if let Some(model) = cap.get(2) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            let op = cap.get(1).map_or("", |m| m.as_str());
            let direction = if op == "query" { "read" } else { "write" };
            events.push(make_event("<module>", &file_str, line, direction, "sql", model.as_str(), language, None));
        }
    }

    // ── HTTP headers (regex fallback) ──────────────────────────────────────
    for cap in patterns::http::SETHEADER_RE.captures_iter(src) {
        if let Some(header) = cap.get(2) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "http_header", header.as_str(), language, None));
        }
    }
    for cap in patterns::http::REQ_HEADER_RE.captures_iter(src) {
        let key = cap.get(1).or_else(|| cap.get(2));
        if let Some(header) = key {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "http_header", header.as_str(), language, None));
        }
    }

    // ── RabbitMQ ───────────────────────────────────────────────────────────
    for cap in patterns::queue::RABBITMQ_PUBLISH_RE.captures_iter(src) {
        let key = cap.get(2).or_else(|| cap.get(1)).map_or("", |m| m.as_str());
        if !key.is_empty() {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "rabbitmq", key, language, None));
        }
    }
    for cap in patterns::queue::RABBITMQ_CONSUME_RE.captures_iter(src) {
        if let Some(q) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "rabbitmq", q.as_str(), language, None));
        }
    }

    // ── SQS ────────────────────────────────────────────────────────────────
    for cap in patterns::queue::SQS_SEND_RE.captures_iter(src) {
        if let Some(url) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "write", "sqs", url.as_str(), language, None));
        }
    }
    for cap in patterns::queue::SQS_RECV_RE.captures_iter(src) {
        if let Some(url) = cap.get(1) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            events.push(make_event("<module>", &file_str, line, "read", "sqs", url.as_str(), language, None));
        }
    }

    // ── Filesystem ─────────────────────────────────────────────────────────
    for cap in patterns::fs::FS_JS_RE.captures_iter(src) {
        if let (Some(method), Some(path)) = (cap.get(1), cap.get(2)) {
            let line = cap.get(0).map(|m| line_of(src, m.start()));
            let dir = if method.as_str().contains("read") { "read" } else { "write" };
            events.push(make_event("<module>", &file_str, line, dir, "filesystem", path.as_str(), language, None));
        }
    }

    // ── gRPC / protobuf ────────────────────────────────────────────────────
    match language {
        "rust" => {
            for cap in patterns::grpc::RUST_PROST_ENCODE_RE.captures_iter(src) {
                if let Some(msg) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", msg.as_str(), language, None));
                }
            }
            for cap in patterns::grpc::RUST_PROST_DECODE_RE.captures_iter(src) {
                if let Some(msg) = cap.get(1) {
                    // Skip stdlib/common types that happen to have ::decode
                    let name = msg.as_str();
                    if matches!(name, "base64" | "hex" | "utf8" | "str" | "String" | "Vec" | "u8" | "i64" | "f64") { continue; }
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "read", "grpc", name, language, None));
                }
            }
            for cap in patterns::grpc::RUST_TONIC_RE.captures_iter(src) {
                if let Some(msg) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", msg.as_str(), language, None));
                }
            }
        }
        "typescript" | "javascript" => {
            for cap in patterns::grpc::TS_PROTO_ENCODE_RE.captures_iter(src) {
                if let Some(msg) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", msg.as_str(), language, None));
                }
            }
            for cap in patterns::grpc::TS_PROTO_DECODE_RE.captures_iter(src) {
                if let Some(msg) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "read", "grpc", msg.as_str(), language, None));
                }
            }
            for cap in patterns::grpc::TS_GRPC_STUB_RE.captures_iter(src) {
                if let Some(method) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", method.as_str(), language, None));
                }
            }
        }
        "python" => {
            for cap in patterns::grpc::PY_GRPC_STUB_RE.captures_iter(src) {
                if let Some(method) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", method.as_str(), language, None));
                }
            }
            for cap in patterns::grpc::PY_GRPC_CHANNEL_RE.captures_iter(src) {
                if let Some(addr) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", addr.as_str(), language, None));
                }
            }
        }
        "ruby" => {
            for cap in patterns::grpc::RUBY_GRPC_STUB_RE.captures_iter(src) {
                if let Some(method) = cap.get(1) {
                    let line = cap.get(0).map(|m| line_of(src, m.start()));
                    events.push(make_event("<module>", &file_str, line, "write", "grpc", method.as_str(), language, None));
                }
            }
        }
        _ => {}
    }

    events
}

fn make_event(
    fn_name: &str,
    file: &str,
    line: Option<i64>,
    direction: &str,
    medium: &str,
    key_raw: &str,
    plugin: &str,
    local_var: Option<&str>,
) -> BoundaryEvent {
    BoundaryEvent {
        id: None,
        fn_name: fn_name.to_string(),
        file: file.to_string(),
        line,
        direction: direction.to_string(),
        medium: medium.to_string(),
        key_raw: key_raw.to_string(),
        key_norm: normalise_key(key_raw),
        local_var: local_var.map(String::from),
        raw_context: None,
        prov_source: "regex".to_string(),
        prov_confidence: 0.65,
        prov_plugin: plugin.to_string(),
        prov_note: None,
    }
}

fn first_group<'t>(cap: &regex::Captures<'t>) -> &'t str {
    for i in 1..cap.len() {
        if let Some(m) = cap.get(i) {
            return m.as_str();
        }
    }
    ""
}
