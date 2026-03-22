use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub root_dir:       PathBuf,
    pub db_path:        String,
    pub max_depth:      usize,
    pub skip_embed:     bool,
    pub use_lsp:        bool,
    pub min_confidence: f64,
    pub all_entrypoints: bool,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub embedding_model: String,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            root_dir:         PathBuf::from("."),
            db_path:          "index.db".into(),
            max_depth:        6,
            skip_embed:       false,
            use_lsp:          false,
            min_confidence:   0.5,
            all_entrypoints:  false,
            openai_api_key:   None,
            anthropic_api_key: None,
            embedding_model:  "text-embedding-3-small".into(),
        }
    }
}

// Directories to skip during file walking
pub const SKIP_DIRS: &[&str] = &[
    ".git", ".svn", ".hg",
    "node_modules", "vendor",
    "target", "__pycache__",
    ".next", "dist", "build",
    ".cache", "coverage",
];
