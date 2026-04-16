use once_cell::sync::Lazy;
use regex::Regex;

// ── Rust / prost + tonic ───────────────────────────────────────────────────

// encode_to_vec() on an inline struct literal: SomeMessage { ... }.encode_to_vec()
// Requires PascalCase name + struct literal body to avoid matching variable.encode_to_vec()
pub static RUST_PROST_ENCODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"([A-Z]\w+)\s*\{[^}]*\}\s*\.encode_to_vec\s*\(\s*\)"#).unwrap()
});

// SomeType::decode(&buf) or decode(&mut buf)
pub static RUST_PROST_DECODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(\w+)::decode\s*\("#).unwrap()
});

// tonic Request/Response new: tonic::Request::new(SomeType { ... })
pub static RUST_TONIC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:tonic::)?Request::new\s*\(\s*(\w+)\s*[{\(]"#).unwrap()
});

// ── TypeScript / protobufjs  ───────────────────────────────────────────────

// MyMessage.encode(value).finish()
pub static TS_PROTO_ENCODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(\w+)\.encode\s*\([^)]+\)\.finish\s*\(\s*\)"#).unwrap()
});

// MyMessage.decode(reader) or fromJSON
pub static TS_PROTO_DECODE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(\w+)\.decode\s*\(\s*\w+"#).unwrap()
});

// grpc-js: client.MethodName(request, ...) — detects stub calls
pub static TS_GRPC_STUB_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:client|stub|grpcClient)\s*\.\s*([A-Z][a-zA-Z]+)\s*\("#).unwrap()
});

// ── Python / grpc ──────────────────────────────────────────────────────────

// stub.MethodName(request) — capitalised method = gRPC convention
pub static PY_GRPC_STUB_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:stub|channel|client)\s*\.\s*([A-Z][a-zA-Z]+)\s*\("#).unwrap()
});

// grpc.insecure_channel or grpc.secure_channel
pub static PY_GRPC_CHANNEL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"grpc\.(?:insecure_channel|secure_channel)\s*\(\s*['"]([^'"]+)['"]"#).unwrap()
});

// ── Ruby / grpc gem ────────────────────────────────────────────────────────

pub static RUBY_GRPC_STUB_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:stub|client)\s*\.\s*([a-z_]+(?:_call|_request)?)\s*\("#).unwrap()
});
