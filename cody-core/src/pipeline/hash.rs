use std::fs;
use sha2::{Sha256, Digest};
use rayon::prelude::*;
use rusqlite::Connection;
use crate::pipeline::walk::FileEntry;
use crate::db::store::get_file_hash;

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

pub fn hash_files(entries: &[FileEntry], conn: &Connection) -> Vec<HashedFile> {
    // Pre-load all known hashes to avoid per-file DB queries in parallel
    let known: std::collections::HashMap<String, String> = {
        let mut m = std::collections::HashMap::new();
        for e in entries {
            let key = e.path.to_string_lossy().to_string();
            if let Ok(Some(h)) = get_file_hash(conn, &key) {
                m.insert(key, h);
            }
        }
        m
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
