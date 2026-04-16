pub mod writer;

use std::collections::{HashMap, BTreeMap, HashSet};
use crate::db::models::{BoundaryEvent, EntryPoint};
use crate::extractor::ExtractedFacts;
use crate::traces::walker::{build_adjacency, collect_io};
use crate::traces::span::service_from_path;
use crate::config::MapConfig;

pub struct Codemap {
    pub project_name: String,
    pub root_dir:     String,
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
    let has_real_grpc = detect_grpc_transport(all_facts);
    if !has_real_grpc {
        tracing::info!(
            "No tonic/gRPC transport detected — treating prost-encoded payloads as protobuf-over-HTTP; skipping grpc topology edges"
        );
    }

    // Detect service name collisions between backend monolith services (files under
    // `*/src/services/<X>/...`) and top-level "app" directories (files directly under
    // `<X>/...`). Without this, a client's outbound HTTP call to its own backend is
    // suppressed by the `src_svc != dst_svc` topology filter.
    let colliding = compute_colliding_services(all_facts);
    if !colliding.is_empty() {
        tracing::info!(
            "Service name collisions resolved with -app suffix: {:?}",
            colliding
        );
    }
    let resolve_svc = |file: &str| resolve_service(file, &colliding);

    let adj = build_adjacency(all_facts);

    let mut boundary_index: HashMap<(String, String), Vec<BoundaryEvent>> = HashMap::new();
    for facts in all_facts {
        for ev in &facts.boundary_events {
            if ev.prov_confidence >= config.min_confidence {
                boundary_index
                    .entry((ev.fn_name.clone(), ev.file.clone()))
                    .or_default()
                    .push(ev.clone());
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
        let svc = resolve_svc(&ep.file);
        let lang = file_lang.get(&ep.file).cloned().unwrap_or_default();
        let data = services.entry(svc.clone()).or_insert(ServiceData {
            language: lang,
            entries: vec![],
        });
        data.entries.push(ServiceEntry { ep: ep.clone(), io });
    }

    // Build topology from grpc/kafka events — only between services that have entry points
    let real_services: HashSet<String> = services.keys().cloned().collect();

    // Route index: path → service_name (for http_out resolution)
    // Paths registered by more than one service are ambiguous and removed.
    let mut route_count: HashMap<String, usize> = HashMap::new();
    let mut route_index: HashMap<String, String> = HashMap::new();
    for ep in entry_points {
        if let Some(path) = &ep.path {
            let svc = resolve_svc(&ep.file);
            *route_count.entry(path.clone()).or_default() += 1;
            route_index.insert(path.clone(), svc);
        }
    }
    route_index.retain(|path, _| route_count.get(path).copied().unwrap_or(0) <= 1);

    let mut topo_set: HashSet<(String, String, String, String)> = HashSet::new();
    for facts in all_facts {
        let src_svc = resolve_svc(&facts.file);
        for ev in &facts.boundary_events {
            if ev.prov_confidence < config.min_confidence { continue; }

            if ["grpc", "kafka"].contains(&ev.medium.as_str()) {
                if !real_services.contains(&src_svc) { continue; }
                // Skip grpc topology when the project uses prost without tonic transport.
                // The proto types are just the codec for HTTP bodies — endpoint annotations
                // (`body{Type}`) already capture this, cross-service grpc edges would be phantom.
                if ev.medium == "grpc" && !has_real_grpc { continue; }
                // For gRPC, only use keys that look like proto type names (PascalCase).
                // Generic lowercase names like `response` or `auth_token` are variable
                // names captured from .encode_to_vec() calls, not shared message types.
                if ev.medium == "grpc" && !ev.key_raw.chars().next().map_or(false, |c| c.is_uppercase()) {
                    continue;
                }
                // Find which other real service mentions this type
                for other_facts in all_facts {
                    let dst_svc = resolve_svc(&other_facts.file);
                    if dst_svc == src_svc || !real_services.contains(&dst_svc) { continue; }
                    let matches = other_facts.boundary_events.iter()
                        .any(|e| e.key_raw == ev.key_raw)
                        // Only match against type symbols (structs/classes), not functions/variables
                        || other_facts.symbols.iter().any(|s| s.kind == "class" && s.name == ev.key_raw);
                    if matches {
                        topo_set.insert((src_svc.clone(), dst_svc, ev.medium.clone(), ev.key_raw.clone()));
                        break;
                    }
                }
            } else if ev.medium == "http_out" {
                // Resolve outbound HTTP call to backend service
                if let Some(dst_svc) = resolve_http_path(&ev.key_raw, &route_index, &real_services) {
                    if dst_svc != src_svc {
                        topo_set.insert((src_svc.clone(), dst_svc, "http".to_string(), ev.key_raw.clone()));
                    }
                }
            }
        }
    }

    let topology: Vec<(String, String, String, String)> = topo_set.into_iter().collect();

    Codemap {
        project_name: config.root_dir.file_name()
            .and_then(|n| n.to_str()).unwrap_or("project").to_string(),
        root_dir: config.root_dir.to_string_lossy().to_string(),
        file_count: all_facts.len(),
        languages: langs,
        services,
        topology,
    }
}

/// Detect whether a file lives inside a backend monolith layout: `*/src/services/<X>/<file>`
/// (requires at least one directory between `services/` and the file so shared utilities
/// like `src/services/mod.rs` are not misclassified).
fn is_monolith_path(file: &str) -> bool {
    let parts: Vec<&str> = file.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 4 { return false; }
    for i in 0..parts.len().saturating_sub(3) {
        if parts[i] == "src" && parts[i + 1] == "services" {
            return true;
        }
    }
    false
}

/// Resolve a file to a service name, applying `-app` suffix when the file lives in
/// a top-level directory whose name collides with a backend monolith service.
fn resolve_service(file: &str, colliding: &HashSet<String>) -> String {
    let svc = service_from_path(file);
    if !is_monolith_path(file) && colliding.contains(&svc) {
        format!("{}-app", svc)
    } else {
        svc
    }
}

/// Identify service names that are produced from BOTH a backend monolith structure
/// (file path contains `/src/services/<X>/<file>`) and a top-level "app" directory.
fn compute_colliding_services(all_facts: &[ExtractedFacts]) -> HashSet<String> {
    let mut monolith: HashSet<String> = HashSet::new();
    let mut app: HashSet<String> = HashSet::new();
    for facts in all_facts {
        let svc = service_from_path(&facts.file);
        if is_monolith_path(&facts.file) {
            monolith.insert(svc);
        } else {
            app.insert(svc);
        }
    }
    monolith.intersection(&app).cloned().collect()
}

/// Detect whether the project uses real gRPC transport (tonic server/client) vs
/// just protobuf encoding over plain HTTP (prost alone, with tonic_build for codegen).
///
/// Uses two signals:
/// 1. An import edge mentioning `tonic::transport`, `tonic::Request`, or `tonic::Response`
/// 2. A call edge to known transport entry points (`serve_with_incoming`, `serve`)
///
/// If neither is present, the project uses protobuf-over-HTTP and `grpc` boundary
/// events should be treated as body codec hints rather than RPC edges.
fn detect_grpc_transport(all_facts: &[ExtractedFacts]) -> bool {
    for facts in all_facts {
        if facts.language != "rust" { continue; }
        for edge in &facts.edges {
            if edge.rel == "imports" {
                if let Some(path) = &edge.dst_file {
                    if path.starts_with("tonic::transport")
                        || path == "tonic::Request"
                        || path == "tonic::Response"
                        || path.starts_with("tonic::transport::")
                    {
                        return true;
                    }
                }
            }
        }
    }
    // Fallback: scan source files for tonic::transport:: usage (catches grouped `use tonic::...`
    // imports and direct fully-qualified references that the edges query may miss).
    for facts in all_facts {
        if facts.language != "rust" { continue; }
        let Ok(source) = std::fs::read_to_string(&facts.file) else { continue };
        if source.contains("tonic::transport::Server")
            || source.contains("tonic::transport::Channel")
            || source.contains("tonic::Request::new")
            || source.contains(".serve_with_incoming")
        {
            return true;
        }
    }
    false
}

/// Resolve an outbound HTTP path to a destination service name.
///
/// Tries in order:
/// 1. Exact match in route_index (path → service)
/// 2. Strip leading `/<service>` component and check if that service exists
///    e.g. `/users/login` → candidate `users` (exists) → returns `users`
/// 3. Longest prefix match in route_index
fn resolve_http_path(
    path: &str,
    route_index: &HashMap<String, String>,
    real_services: &HashSet<String>,
) -> Option<String> {
    // 1. Exact match
    if let Some(svc) = route_index.get(path) {
        return Some(svc.clone());
    }
    // 2. First path segment as service name: "/users/login" → "users"
    if let Some(rest) = path.strip_prefix('/') {
        let candidate_svc = rest.split('/').next().unwrap_or("");
        if !candidate_svc.is_empty() && real_services.contains(candidate_svc) {
            return Some(candidate_svc.to_string());
        }
    }
    // 3. Longest prefix match in route_index (skip bare "/" to avoid false positives)
    route_index.iter()
        .filter(|(route, _)| route.len() > 1 && path.starts_with(route.as_str()))
        .max_by_key(|(route, _)| route.len())
        .map(|(_, svc)| svc.clone())
}
