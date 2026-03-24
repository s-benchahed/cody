use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MapConfig {
    pub root_dir:       PathBuf,
    pub out_path:       String,
    pub max_depth:      usize,
    pub use_lsp:        bool,
    pub min_confidence: f64,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            root_dir:       PathBuf::from("."),
            out_path:       "codemap.md".into(),
            max_depth:      6,
            use_lsp:        false,
            min_confidence: 0.5,
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
