#[derive(Debug, Clone)]
pub struct Symbol {
    pub name:            String,
    pub kind:            String,
    pub file:            String,
    pub line:            Option<i64>,
    pub signature:       Option<String>,
    pub is_exported:     bool,
    pub prov_source:     String,
    pub prov_confidence: f64,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub src_file:   Option<String>,
    pub src_symbol: Option<String>,
    pub rel:        String,
    pub dst_file:   Option<String>,
    pub dst_symbol: Option<String>,
    pub context:    Option<String>,
    pub line:       Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct FileMetaCounts {
    pub lines:   usize,
    pub exports: usize,
    pub imports: usize,
}

#[derive(Debug, Clone)]
pub struct BoundaryEvent {
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

#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub fn_name:    String,
    pub file:       String,
    pub line:       Option<i64>,
    pub kind:       String,
    pub framework:  Option<String>,
    pub path:       Option<String>,
    pub method:     Option<String>,
    pub confidence: f64,
    pub heuristics: Vec<String>,
    pub middleware: Vec<String>,
}

#[derive(Debug, Clone)]
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
