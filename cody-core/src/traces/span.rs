use crate::db::models::BoundaryEvent;

#[derive(Debug, Clone)]
pub struct Span {
    pub id:           String,
    pub trace_id:     String,
    pub parent_id:    Option<String>,
    pub fn_name:      String,
    pub service:      String,
    pub file:         String,
    pub line:         Option<i64>,
    pub edge_kind:    EdgeKind,
    pub depth:        usize,
    pub reads:        Vec<String>,
    pub writes:       Vec<String>,
    pub boundary_in:  Vec<BoundaryEvent>,
    pub boundary_out: Vec<BoundaryEvent>,
    pub baggage:      Vec<String>,
    pub confidence:   f64,
    pub children:     Vec<Span>,
    pub truncated:    Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeKind {
    Root,
    Call,
    DataFlow,
    BoundaryFlow,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeKind::Root         => write!(f, "root"),
            EdgeKind::Call         => write!(f, "call"),
            EdgeKind::DataFlow     => write!(f, "data_flow"),
            EdgeKind::BoundaryFlow => write!(f, "boundary_flow"),
        }
    }
}

/// Derive a service name from a file path.
/// For a path like `/home/user/project/service/src/main.rs` → `service`.
/// For a path like `./cody-core/src/lib.rs` → `cody-core`.
pub fn service_from_path(file: &str) -> String {
    let path = std::path::Path::new(file);
    // Collect only the directory components (not the filename itself)
    let dir_parts: Vec<String> = path.parent()
        .unwrap_or(std::path::Path::new("."))
        .components()
        .filter_map(|c| {
            let s = c.as_os_str().to_string_lossy().to_string();
            if s == "." || s == "/" || s == ".." || s.is_empty() { None } else { Some(s) }
        })
        .collect();

    if dir_parts.is_empty() {
        return "root".to_string();
    }
    if dir_parts.len() <= 2 {
        return dir_parts[0].clone();
    }

    // From the right, skip common source-layout dir names to find the service name
    const SKIP: &[&str] = &[
        "src", "lib", "app", "test", "tests", "spec", "pkg",
        "internal", "cmd", "bin", "scripts", "utils", "helpers",
        "handlers", "services", "controllers", "routes", "middleware",
        "views", "models", "serializers", "resolvers",
    ];
    dir_parts.iter().rev()
        .find(|c| !SKIP.contains(&c.as_str()))
        .or_else(|| dir_parts.last())
        .cloned()
        .unwrap_or_else(|| "root".to_string())
}
