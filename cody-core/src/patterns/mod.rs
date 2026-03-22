pub mod http;
pub mod redis;
pub mod kafka;
pub mod sql;
pub mod env;
pub mod queue;
pub mod fs;
pub mod grpc;

use crate::db::models::BoundaryEvent;
use once_cell::sync::Lazy;
use regex::Regex;

// ── Key normalisation ──────────────────────────────────────────────────────
// Strip all interpolation forms to `{}`
// "session:{userId}" → "session:{}"
// `user:${id}`       → "user:{}"
// f"user:{uid}"      → "user:{}"
// "user:" + id       → "user:{}"

static INTERP_JS:  Lazy<Regex> = Lazy::new(|| Regex::new(r#"\$\{[^}]*\}"#).unwrap());
static INTERP_PY:  Lazy<Regex> = Lazy::new(|| Regex::new(r#"\{[^}]+\}"#).unwrap());
static INTERP_FMT: Lazy<Regex> = Lazy::new(|| Regex::new(r#"%[dsfv]|%\([^)]+\)[dsfv]"#).unwrap());
static CONCAT_RE:  Lazy<Regex> = Lazy::new(|| Regex::new(r#"\s*\+\s*\w+\s*$"#).unwrap());

pub fn normalise_key(raw: &str) -> String {
    let s = raw.to_string();
    let s = INTERP_JS.replace_all(&s, "{}");
    let s = INTERP_PY.replace_all(&s, "{}");
    let s = INTERP_FMT.replace_all(&s, "{}");
    let s = CONCAT_RE.replace_all(&s, "{}");
    s.trim().to_string()
}

// ── Cross-language stitching ───────────────────────────────────────────────

pub fn stitch_boundary_events(
    events: &[BoundaryEvent],
    min_confidence: f64,
) -> Vec<crate::db::models::BoundaryFlow> {
    use std::collections::HashMap;
    use crate::db::models::BoundaryFlow;

    // Group by (medium, key_norm, direction)
    let mut writes: HashMap<(String, String), Vec<&BoundaryEvent>> = HashMap::new();
    let mut reads:  HashMap<(String, String), Vec<&BoundaryEvent>> = HashMap::new();

    for e in events {
        if e.prov_confidence < min_confidence { continue; }
        let k = (e.medium.clone(), e.key_norm.clone());
        if e.direction == "write" {
            writes.entry(k).or_default().push(e);
        } else {
            reads.entry(k).or_default().push(e);
        }
    }

    let mut flows = Vec::new();
    for (key, writers) in &writes {
        if let Some(readers) = reads.get(key) {
            for w in writers {
                for r in readers {
                    // Don't stitch same file to itself (unless cross-function)
                    if w.file == r.file && w.fn_name == r.fn_name { continue; }
                    let confidence = (w.prov_confidence + r.prov_confidence) / 2.0;
                    flows.push(BoundaryFlow {
                        id: None,
                        write_fn:   w.fn_name.clone(),
                        write_file: w.file.clone(),
                        read_fn:    r.fn_name.clone(),
                        read_file:  r.file.clone(),
                        medium:     key.0.clone(),
                        key_norm:   key.1.clone(),
                        confidence,
                    });
                }
            }
        }
    }
    flows
}
