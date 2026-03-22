use once_cell::sync::Lazy;
use regex::Regex;

// JS: fs.readFile/writeFile
pub static FS_JS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:fs|promises)\.(readFile|writeFile|appendFile|readFileSync|writeFileSync)\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// Python: open(path, mode)
pub static OPEN_PY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\bopen\s*\(\s*['"]([^'"]+)['"]\s*,\s*['"]([^'"]+)['"]\s*\)"#).unwrap()
});

// Rust: File::open("path") / File::create("path")
pub static FILE_RUST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"File::(open|create)\s*\(\s*"([^"]+)"\s*\)"#).unwrap()
});
