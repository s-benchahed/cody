pub mod javascript;
pub mod typescript;
pub mod python;
pub mod ruby;
pub mod rust_lang;
pub mod registry;

use std::path::Path;
use anyhow::Result;
use crate::db::models::{Symbol, Edge, BoundaryEvent, EntryPointHint, FileMetaCounts};

pub trait LanguagePlugin: Send + Sync {
    fn language_name(&self) -> &'static str;
    fn extensions(&self) -> &[&'static str];
    fn tree_sitter_language(&self) -> tree_sitter::Language;

    fn parse(&self, source: &[u8], path: &Path) -> Result<tree_sitter::Tree> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&self.tree_sitter_language())
            .map_err(|e| anyhow::anyhow!("set language: {e}"))?;
        parser.parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("tree-sitter parse returned None for {}", path.display()))
    }

    fn extract_symbols(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file: &Path,
    ) -> Result<Vec<Symbol>>;

    fn extract_edges(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file: &Path,
    ) -> Result<Vec<Edge>>;

    fn extract_boundary_events(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file: &Path,
    ) -> Result<Vec<BoundaryEvent>>;

    fn entry_point_hints(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file: &Path,
    ) -> Result<Vec<EntryPointHint>> {
        let _ = (tree, source, file);
        Ok(vec![])
    }

    fn file_meta_counts(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
    ) -> Result<FileMetaCounts>;
}

// ── helper: get text of a node ─────────────────────────────────────────────

pub fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.byte_range()]).unwrap_or("")
}
