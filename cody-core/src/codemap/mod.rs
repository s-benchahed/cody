pub mod writer;

use std::collections::{HashMap, BTreeMap, HashSet};
use crate::db::models::{BoundaryEvent, EntryPoint};
use crate::extractor::ExtractedFacts;
use crate::traces::walker::{build_adjacency, collect_io};
use crate::traces::span::service_from_path;
use crate::config::MapConfig;

pub struct Codemap {
    pub project_name: String,
    pub file_count:   usize,
    pub languages:    Vec<String>,
    pub services:     BTreeMap<String, ServiceData>,
    pub topology:     Vec<(String, String, String, String)>, // (src, dst, medium, key)
}

pub struct ServiceData {
    pub language: String,
    pub entries:  Vec<ServiceEntry>,
}

pub struct ServiceEntry {
    pub ep: EntryPoint,
    pub io: Vec<BoundaryEvent>,
}

pub fn build(
    all_facts: &[ExtractedFacts],
    entry_points: &[EntryPoint],
    config: &MapConfig,
) -> Codemap {
    let adj = build_adjacency(all_facts);

    let mut boundary_index: HashMap<String, Vec<BoundaryEvent>> = HashMap::new();
    for facts in all_facts {
        for ev in &facts.boundary_events {
            if ev.prov_confidence >= config.min_confidence {
                boundary_index.entry(ev.fn_name.clone()).or_default().push(ev.clone());
            }
        }
    }

    let mut langs: Vec<String> = all_facts.iter()
        .map(|f| f.language.clone())
        .filter(|l| !l.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    langs.sort();

    let file_lang: HashMap<String, String> = all_facts.iter()
        .map(|f| (f.file.clone(), f.language.clone()))
        .collect();

    let mut services: BTreeMap<String, ServiceData> = BTreeMap::new();

    for ep in entry_points {
        let io = collect_io(&ep.fn_name, &ep.file, &adj, &boundary_index, config.max_depth);
        if io.is_empty() && ep.path.is_none() { continue; }
        let svc = service_from_path(&ep.file);
        let lang = file_lang.get(&ep.file).cloned().unwrap_or_default();
        let data = services.entry(svc.clone()).or_insert(ServiceData {
            language: lang,
            entries: vec![],
        });
        data.entries.push(ServiceEntry { ep: ep.clone(), io });
    }

    // Build topology from grpc/kafka events — only between services that have entry points
    let real_services: HashSet<String> = services.keys().cloned().collect();

    let mut topo_set: HashSet<(String, String, String, String)> = HashSet::new();
    for facts in all_facts {
        let src_svc = service_from_path(&facts.file);
        if !real_services.contains(&src_svc) { continue; }
        for ev in &facts.boundary_events {
            if !["grpc", "kafka"].contains(&ev.medium.as_str()) { continue; }
            if ev.prov_confidence < config.min_confidence { continue; }
            // Find which other real service mentions this type
            for other_facts in all_facts {
                let dst_svc = service_from_path(&other_facts.file);
                if dst_svc == src_svc || !real_services.contains(&dst_svc) { continue; }
                let matches = other_facts.boundary_events.iter()
                    .any(|e| e.key_raw == ev.key_raw)
                    || other_facts.symbols.iter().any(|s| s.name == ev.key_raw);
                if matches {
                    topo_set.insert((src_svc.clone(), dst_svc, ev.medium.clone(), ev.key_raw.clone()));
                    break;
                }
            }
        }
    }

    let topology: Vec<(String, String, String, String)> = topo_set.into_iter().collect();

    Codemap {
        project_name: config.root_dir.file_name()
            .and_then(|n| n.to_str()).unwrap_or("project").to_string(),
        file_count: all_facts.len(),
        languages: langs,
        services,
        topology,
    }
}
