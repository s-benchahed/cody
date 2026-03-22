use once_cell::sync::Lazy;
use regex::Regex;

// JS: producer.send({ topic: 'name', ... })
pub static KAFKA_SEND_JS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"producer\.send\s*\(\s*\{[^}]*topic:\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// JS: consumer.subscribe({ topics: ['name'] })
pub static KAFKA_SUBSCRIBE_JS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"consumer\.subscribe\s*\(\s*\{[^}]*topics?:\s*\[?['"`]([^'"`]+)['"`]"#).unwrap()
});

// Python: producer.produce('topic', ...)
pub static KAFKA_PRODUCE_PY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"producer\.produce\s*\(\s*['"]([^'"]+)['"]\s*,"#).unwrap()
});

// Python: consumer.subscribe(['topic'])
pub static KAFKA_SUBSCRIBE_PY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"consumer\.subscribe\s*\(\s*\[['"]([^'"]+)['"]\]"#).unwrap()
});

// Ruby Sidekiq-like: SomeWorker.perform_async(...)
pub static SIDEKIQ_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(\w+Worker|Job)\.(perform_async|perform_in|perform_at)"#).unwrap()
});
