use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id:              Option<i64>,
    pub name:            String,
    pub kind:            String,
    pub file:            String,
    pub line:            Option<i64>,
    pub signature:       Option<String>,
    pub is_exported:     bool,
    pub prov_source:     String,
    pub prov_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id:         Option<i64>,
    pub src_file:   Option<String>,
    pub src_symbol: Option<String>,
    pub rel:        String,
    pub dst_file:   Option<String>,
    pub dst_symbol: Option<String>,
    pub context:    Option<String>,
    pub line:       Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub file:     String,
    pub language: String,
    pub lines:    i64,
    pub exports:  i64,
    pub imports:  i64,
    pub hash:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryEvent {
    pub id:              Option<i64>,
    pub fn_name:         String,
    pub file:            String,
    pub line:            Option<i64>,
    pub direction:       String,
    pub medium:          String,
    pub key_raw:         String,
    pub key_norm:        String,
    pub local_var:       Option<String>,
    pub raw_context:     Option<String>,
    pub prov_source:     String,
    pub prov_confidence: f64,
    pub prov_plugin:     String,
    pub prov_note:       Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryFlow {
    pub id:         Option<i64>,
    pub write_fn:   String,
    pub write_file: String,
    pub read_fn:    String,
    pub read_file:  String,
    pub medium:     String,
    pub key_norm:   String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    pub id:         Option<i64>,
    pub fn_name:    String,
    pub file:       String,
    pub line:       Option<i64>,
    pub kind:       String,
    pub framework:  Option<String>,
    pub path:       Option<String>,
    pub method:     Option<String>,
    pub confidence: f64,
    pub heuristics: Vec<String>,
    pub middleware: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub id:             Option<i64>,
    pub trace_id:       String,
    pub root_fn:        String,
    pub root_file:      String,
    pub service:        String,
    pub text:           String,
    pub compact:        String,
    pub otlp:           Option<String>,
    pub span_count:     i64,
    pub fn_names:       Vec<String>,
    pub media:          Vec<String>,
    pub value_names:    Vec<String>,
    pub min_confidence: f64,
    pub created_at:     String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPointHint {
    pub fn_name:    String,
    pub file:       String,
    pub line:       Option<i64>,
    pub kind:       String,
    pub framework:  Option<String>,
    pub path:       Option<String>,
    pub method:     Option<String>,
    pub confidence: f64,
    pub heuristic:  String,
    pub middleware: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FileMetaCounts {
    pub export_count: i64,
    pub import_count: i64,
    pub line_count:   i64,
}
