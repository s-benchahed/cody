use std::sync::Arc;
use rayon::prelude::*;
use crate::pipeline::hash::HashedFile;
use crate::plugin::{LanguagePlugin, registry::PluginRegistry};

pub struct ParsedFile {
    pub hashed: HashedFile,
    pub tree:   tree_sitter::Tree,
    pub plugin: Arc<dyn LanguagePlugin>,
}

pub fn parse_files(files: &[HashedFile], registry: &PluginRegistry) -> Vec<ParsedFile> {
    files.par_iter().filter_map(|hf| {
        let plugin = registry.get(&hf.entry.extension)?.clone();
        match plugin.parse(&hf.source, &hf.entry.path) {
            Ok(tree) => Some(ParsedFile { hashed: hf.clone(), tree, plugin }),
            Err(e) => {
                tracing::warn!("parse error {}: {e}", hf.entry.path.display());
                None
            }
        }
    }).collect()
}
