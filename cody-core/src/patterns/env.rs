use once_cell::sync::Lazy;
use regex::Regex;

// JS: process.env.NAME
pub static JS_ENV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"process\.env\.([A-Z_][A-Z0-9_]+)"#).unwrap()
});

// Python: os.environ.get('NAME') / os.environ['NAME'] / os.getenv('NAME')
pub static PY_ENV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"os\.environ(?:\.get)?\s*\(\s*['"]([^'"]+)['"]\)|os\.getenv\s*\(\s*['"]([^'"]+)['"]\)|os\.environ\[['"]([^'"]+)['"]\]"#).unwrap()
});

// Ruby: ENV['NAME'] / ENV.fetch('NAME')
pub static RUBY_ENV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"ENV\[['"]([^'"]+)['"]\]|ENV\.fetch\s*\(\s*['"]([^'"]+)['"]\)"#).unwrap()
});

// Rust: std::env::var("NAME") / env::var("NAME")
pub static RUST_ENV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:std::env|env)::var\s*\(\s*"([^"]+)"\s*\)"#).unwrap()
});
