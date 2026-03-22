use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::config::SKIP_DIRS;
use crate::plugin::registry::PluginRegistry;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path:      PathBuf,
    pub extension: String,
}

pub fn collect_files(root: &Path, registry: &PluginRegistry) -> Vec<FileEntry> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden and excluded directories (but never prune the root itself)
            let name = e.file_name().to_string_lossy();
            if e.file_type().is_dir() && e.depth() > 0 {
                return !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_ref());
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.into_path();
            let ext = path.extension()?.to_str()?.to_lowercase();
            if registry.contains_key(&ext) {
                Some(FileEntry { path, extension: ext })
            } else {
                None
            }
        })
        .collect()
}
