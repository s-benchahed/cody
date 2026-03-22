use once_cell::sync::Lazy;
use regex::Regex;

pub static REDIS_GET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:client|redis|cache|r)\.(get|hget|lrange|smembers|zscore|zrange|type|ttl|exists)\s*\(\s*[f`]?['"`]([^'"`]+)['"`]"#).unwrap()
});

pub static REDIS_SET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:client|redis|cache|r)\.(set|hset|setex|psetex|lpush|rpush|sadd|zadd|incr|decr)\s*\(\s*[f`]?['"`]([^'"`]+)['"`]"#).unwrap()
});

pub static REDIS_DEL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:client|redis|cache|r)\.(del|delete|unlink|expire)\s*\(\s*[f`]?['"`]([^'"`]+)['"`]"#).unwrap()
});

// Go redis: redis.Get(ctx, key) / redis.Set(ctx, key, ...)
pub static GO_REDIS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"redis\.(Get|Set|HGet|HSet|Del)\s*\(ctx,\s*"([^"]+)""#).unwrap()
});
