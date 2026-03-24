pub mod http;
pub mod redis;
pub mod kafka;
pub mod sql;
pub mod env;
pub mod queue;
pub mod fs;
pub mod grpc;

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
