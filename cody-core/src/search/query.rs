use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use rusqlite::Connection;
use crate::db::store;

#[derive(Serialize)]
struct Message {
    role:    String,
    content: String,
}

const SYSTEM_PROMPT: &str = r#"You are a codebase navigation assistant with access to a pre-built semantic index.

Tools available:
- get_skeleton(fn_name): compact semantic skeleton of one function
- get_source(fn_name): raw source code for a function
- get_carriers(value): all components that handle a value
- get_traces_touching(medium, key?): traces touching a resource

Strategy:
1. Read the provided traces first — they often contain the full answer
2. For detail on a specific function, call get_skeleton
3. Only call get_source if you need the exact implementation
4. Never speculate — use the tools

Notation:
- $v1, $v2 = value identities persisting across component boundaries
- ~~► = data_flow edge (value crosses via shared state)
- via: medium["key"] = boundary crossing point
"#;

pub async fn run_query(
    question: &str,
    conn: &Connection,
    api_key: &str,
) -> Result<String> {
    // Find relevant traces (simple text search — no embedding required)
    let traces = store::load_all_traces(conn)?;
    let relevant: Vec<_> = traces.iter()
        .filter(|t| {
            let q = question.to_lowercase();
            t.root_fn.to_lowercase().contains(&q)
                || t.text.to_lowercase().contains(&q)
                || t.fn_names.iter().any(|f| f.to_lowercase().contains(&q))
                || t.media.iter().any(|m| q.contains(m.as_str()))
        })
        .take(3)
        .collect();

    let traces_text = if relevant.is_empty() {
        "No pre-built traces matched your query. Use get_skeleton or get_source to explore.".to_string()
    } else {
        relevant.iter().map(|t| t.text.clone()).collect::<Vec<_>>().join("\n\n---\n\n")
    };

    let user_content = format!(
        "Question: {question}\n\nRelevant traces:\n{traces_text}"
    );

    // Simple agentic loop — call Anthropic API
    let http = reqwest::Client::new();
    let mut messages: Vec<Value> = vec![json!({"role":"user","content":user_content})];

    for _turn in 0..6 {
        let body = json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 2048,
            "system": SYSTEM_PROMPT,
            "tools": build_tools(),
            "messages": messages,
        });

        let resp = http.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Anthropic API error {status}: {text}"));
        }

        let resp_json: Value = resp.json().await?;
        let stop_reason = resp_json["stop_reason"].as_str().unwrap_or("");

        if stop_reason == "end_turn" {
            // Extract text content
            let text = resp_json["content"].as_array()
                .and_then(|c| c.iter().find(|b| b["type"] == "text"))
                .and_then(|b| b["text"].as_str())
                .unwrap_or("")
                .to_string();
            return Ok(text);
        }

        // Handle tool calls
        if stop_reason == "tool_use" {
            let content = resp_json["content"].as_array().cloned().unwrap_or_default();
            messages.push(json!({"role":"assistant","content":content}));

            let mut tool_results: Vec<Value> = Vec::new();
            for block in &content {
                if block["type"] == "tool_use" {
                    let tool_name = block["name"].as_str().unwrap_or("");
                    let input = &block["input"];
                    let tool_id = block["id"].as_str().unwrap_or("");
                    let result = dispatch_tool(conn, tool_name, input)?;
                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": tool_id,
                        "content": result,
                    }));
                }
            }
            messages.push(json!({"role":"user","content":tool_results}));
        }
    }
    Err(anyhow!("Max turns reached without final answer"))
}

fn dispatch_tool(conn: &Connection, name: &str, input: &Value) -> Result<String> {
    match name {
        "get_skeleton" => {
            let fn_name = input["fn_name"].as_str().unwrap_or("");
            let syms = store::lookup_symbol(conn, fn_name)?;
            let bounds = store::boundaries_for_fn(conn, fn_name)?;
            if syms.is_empty() {
                return Ok(format!("Symbol not found: {fn_name}"));
            }
            let s = &syms[0];
            let mut out = format!("FN {} {}:{}\n", s.name, s.file, s.line.unwrap_or(0));
            for be in &bounds {
                out.push_str(&format!("  {:>5}  {:12} \"{}\"\n",
                    be.direction.to_uppercase(), be.medium, be.key_raw));
            }
            Ok(out)
        }
        "get_source" => {
            let fn_name = input["fn_name"].as_str().unwrap_or("");
            let syms = store::lookup_symbol(conn, fn_name)?;
            if let Some(s) = syms.first() {
                let line = s.line.unwrap_or(0) as usize;
                if let Ok(content) = std::fs::read_to_string(&s.file) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = line.saturating_sub(2);
                    let end = (line + 30).min(lines.len());
                    let snippet = lines[start..end].join("\n");
                    return Ok(format!("// {}:{}-{}\n{}", s.file, start+1, end, snippet));
                }
            }
            Ok(format!("Source not found for {fn_name}"))
        }
        "get_carriers" => {
            let value = input["value"].as_str().unwrap_or("");
            let events = store::boundaries_for_medium(conn, value)
                .or_else(|_| store::load_all_boundary_events(conn)
                    .map(|all| all.into_iter().filter(|e| e.key_norm.contains(value)).collect()))?;
            let mut out = format!("Carriers of '{value}':\n");
            for e in events.iter().take(20) {
                out.push_str(&format!("  {:>5}  {:30} {}:{}\n",
                    e.direction, e.fn_name, e.file, e.line.unwrap_or(0)));
            }
            Ok(out)
        }
        "get_traces_touching" => {
            let medium = input["medium"].as_str().unwrap_or("");
            let events = store::boundaries_for_medium(conn, medium)?;
            let mut fn_names: Vec<String> = events.iter().map(|e| e.fn_name.clone()).collect();
            fn_names.sort(); fn_names.dedup();
            let mut out = format!("Functions touching medium '{medium}':\n");
            for f in fn_names.iter().take(20) {
                out.push_str(&format!("  {f}\n"));
            }
            Ok(out)
        }
        _ => Ok(format!("Unknown tool: {name}")),
    }
}

fn build_tools() -> Value {
    json!([
        {
            "name": "get_skeleton",
            "description": "Compact semantic skeleton of a function: boundaries, reads, writes",
            "input_schema": {
                "type": "object",
                "properties": { "fn_name": { "type": "string" } },
                "required": ["fn_name"]
            }
        },
        {
            "name": "get_source",
            "description": "Raw source code for a function from disk",
            "input_schema": {
                "type": "object",
                "properties": { "fn_name": { "type": "string" } },
                "required": ["fn_name"]
            }
        },
        {
            "name": "get_carriers",
            "description": "All components that handle a value or boundary key",
            "input_schema": {
                "type": "object",
                "properties": { "value": { "type": "string" } },
                "required": ["value"]
            }
        },
        {
            "name": "get_traces_touching",
            "description": "All traces/functions touching a boundary medium",
            "input_schema": {
                "type": "object",
                "properties": {
                    "medium": { "type": "string" },
                    "key":    { "type": "string" }
                },
                "required": ["medium"]
            }
        }
    ])
}
