use std::fs;
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use rayon::prelude::*;
use crate::pipeline::walk::FileEntry;

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeStatus {
    New,
    Changed,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct HashedFile {
    pub entry:   FileEntry,
    pub source:  Vec<u8>,
    pub sha256:  String,
    pub status:  ChangeStatus,
}

pub fn hash_files_cached(entries: &[FileEntry], cache_path: &std::path::Path) -> Vec<HashedFile> {
    // Load cached hashes from JSON file if it exists
    let known: HashMap<String, String> = if cache_path.exists() {
        match fs::read_to_string(cache_path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    } else {
        HashMap::new()
    };

    entries.par_iter().filter_map(|entry| {
        let source = fs::read(&entry.path).ok()?;
        let mut hasher = Sha256::new();
        hasher.update(&source);
        let sha256 = hex::encode(hasher.finalize());
        let key = entry.path.to_string_lossy().to_string();
        let status = match known.get(&key) {
            None => ChangeStatus::New,
            Some(old) if old != &sha256 => ChangeStatus::Changed,
            _ => ChangeStatus::Unchanged,
        };
        Some(HashedFile { entry: entry.clone(), source, sha256, status })
    }).collect()
}

pub fn save_cache(hashed: &[HashedFile], cache_path: &std::path::Path) {
    let map: HashMap<String, String> = hashed.iter()
        .map(|h| (h.entry.path.to_string_lossy().to_string(), h.sha256.clone()))
        .collect();
    if let Ok(json) = serde_json::to_string(&map) {
        let _ = fs::write(cache_path, json);
    }
}

pub fn sha256_file(path: &std::path::Path) -> Option<String> {
    let source = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&source);
    Some(hex::encode(hasher.finalize()))
}
