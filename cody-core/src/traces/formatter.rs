use crate::traces::span::{Span, EdgeKind};

/// Render the full trace tree into the LLM-optimised text format.
pub fn serialize_trace(root: &Span) -> String {
    let mut out = String::new();
    let baggage_names: Vec<String> = collect_baggage(root);

    out.push_str(&format!(
        "TRACE: {}  [{}]  {}\n",
        root.fn_name, root.service, root.file
    ));
    if !baggage_names.is_empty() {
        out.push_str(&format!("baggage: {}\n", baggage_names.join(" ")));
    }
    out.push('\n');
    render_span(root, &mut out, "", true);
    out
}

fn render_span(span: &Span, out: &mut String, prefix: &str, is_last: bool) {
    let connector = if prefix.is_empty() { "├─" } else if is_last { "└─" } else { "├─" };
    let edge_label = match span.edge_kind {
        EdgeKind::Root         => "".to_string(),
        EdgeKind::Call         => "call → ".to_string(),
        EdgeKind::DataFlow     => "data_flow ~~► ".to_string(),
        EdgeKind::BoundaryFlow => "boundary ~~► ".to_string(),
    };

    let baggage_str = if span.baggage.is_empty() { String::new() }
                      else { format!("({})", span.baggage.join(", ")) };

    out.push_str(&format!(
        "{}{}{}{} {}  [{}]  {}:{}\n",
        prefix, connector, edge_label,
        span.fn_name, baggage_str,
        span.service, span.file,
        span.line.map(|l| l.to_string()).unwrap_or_default()
    ));

    let child_prefix = format!("{}│  ", prefix);

    // Boundary reads
    for be in &span.boundary_in {
        let bname = format!("${}:{}", be.medium, &be.key_norm[..be.key_norm.len().min(20)]);
        out.push_str(&format!("{}│  reads:  {}[\"{}\"] → {}\n",
            prefix, be.medium, be.key_raw, bname));
    }
    // Boundary writes
    for be in &span.boundary_out {
        out.push_str(&format!("{}│  writes: {}[\"{}\"] ← {}\n",
            prefix, be.medium, be.key_raw, be.local_var.as_deref().unwrap_or("?")));
    }

    // Truncation marker
    if let Some(n) = span.truncated {
        out.push_str(&format!("{}│  [truncated: {} children omitted]\n", prefix, n));
    }

    let n = span.children.len();
    for (i, child) in span.children.iter().enumerate() {
        let last = i == n - 1;
        let next_prefix = if last {
            format!("{}   ", prefix)
        } else {
            format!("{}│  ", prefix)
        };
        render_span(child, out, &next_prefix, last);
    }
}

/// Compact one-line-per-span format for embedding
pub fn serialize_compact(root: &Span) -> String {
    let mut lines = Vec::new();
    flatten_compact(root, &root.fn_name, &mut lines);
    lines.join("\n")
}

fn flatten_compact(span: &Span, root_name: &str, lines: &mut Vec<String>) {
    for be in span.boundary_in.iter().chain(span.boundary_out.iter()) {
        lines.push(format!(
            "{} [{}] → {}:{} {}",
            root_name, span.service, be.medium, be.key_norm, be.direction.to_uppercase()
        ));
    }
    for child in &span.children {
        lines.push(format!(
            "{} [{}] →{} {} [{}]",
            root_name, span.service,
            match child.edge_kind {
                EdgeKind::DataFlow     => "~~",
                EdgeKind::BoundaryFlow => "~~",
                _ => "",
            },
            child.fn_name, child.service
        ));
        flatten_compact(child, root_name, lines);
    }
}

fn collect_baggage(span: &Span) -> Vec<String> {
    let mut names: Vec<String> = span.baggage.clone();
    for child in &span.children {
        for n in collect_baggage(child) {
            if !names.contains(&n) { names.push(n); }
        }
    }
    names
}

pub fn collect_fn_names(span: &Span) -> Vec<String> {
    let mut names = vec![span.fn_name.clone()];
    for child in &span.children {
        names.extend(collect_fn_names(child));
    }
    names.sort();
    names.dedup();
    names
}

pub fn collect_media(span: &Span) -> Vec<String> {
    let mut media: Vec<String> = span.boundary_in.iter().chain(span.boundary_out.iter())
        .map(|be| be.medium.clone())
        .collect();
    for child in &span.children {
        media.extend(collect_media(child));
    }
    media.sort();
    media.dedup();
    media
}

pub fn min_confidence(span: &Span) -> f64 {
    let mut min = span.confidence;
    for child in &span.children {
        min = min.min(min_confidence(child));
    }
    min
}

pub fn count_spans(span: &Span) -> i64 {
    1 + span.children.iter().map(count_spans).sum::<i64>()
}
