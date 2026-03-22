use serde_json::{json, Value};
use crate::traces::span::Span;

pub fn serialize_otlp(root: &Span) -> Value {
    json!({
        "resourceSpans": [{
            "resource": { "attributes": [] },
            "scopeSpans": [{
                "scope": { "name": "cody" },
                "spans": flatten_spans(root)
            }]
        }]
    })
}

fn flatten_spans(span: &Span) -> Vec<Value> {
    let mut spans = vec![span_to_otlp(span)];
    for child in &span.children {
        spans.extend(flatten_spans(child));
    }
    spans
}

fn span_to_otlp(span: &Span) -> Value {
    json!({
        "traceId": span.trace_id,
        "spanId":  span.id,
        "parentSpanId": span.parent_id,
        "name":    span.fn_name,
        "kind":    span.edge_kind.to_string(),
        "attributes": [
            { "key": "service", "value": { "stringValue": span.service } },
            { "key": "file",    "value": { "stringValue": span.file } },
            { "key": "depth",   "value": { "intValue": span.depth } },
        ],
        "events": span.boundary_in.iter().chain(span.boundary_out.iter()).map(|be| {
            json!({
                "name": format!("{}.{}", be.medium, be.direction),
                "attributes": [
                    { "key": "medium",    "value": { "stringValue": be.medium } },
                    { "key": "key",       "value": { "stringValue": be.key_norm } },
                    { "key": "direction", "value": { "stringValue": be.direction } },
                ]
            })
        }).collect::<Vec<_>>(),
    })
}
