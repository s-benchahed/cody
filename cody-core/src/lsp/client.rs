use std::path::Path;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub struct LspClient {
    _child:  Child,
    stdin:   ChildStdin,
    stdout:  BufReader<ChildStdout>,
    next_id: u64,
}

impl LspClient {
    /// Spawn the LSP server binary, send `initialize`, wait for the result.
    pub async fn spawn(binary: &str, args: &[&str], workspace: &Path) -> Result<Self> {
        let mut child = Command::new(binary)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("spawn {binary}: {e}"))?;

        let stdin  = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let mut client = LspClient {
            _child:  child,
            stdin,
            stdout:  BufReader::new(stdout),
            next_id: 1,
        };

        let root_uri = path_to_uri(workspace);
        let id = client.next_id();
        client.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "hover": { "contentFormat": ["markdown", "plaintext"] }
                    }
                },
                "initializationOptions": {}
            }
        })).await?;

        // Wait for the initialize result (skip any notifications that arrive first)
        loop {
            let msg = client.recv().await?;
            if msg.get("id").and_then(|v| v.as_u64()) == Some(id) { break; }
        }

        // Send initialized notification (required by LSP spec)
        client.send(json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        })).await?;

        // Drain notifications until the server signals it's ready ($/progress end) or timeout.
        // rust-analyzer emits $/progress notifications while loading the workspace; we need to
        // wait for the final "end" before hover queries will return useful results.
        let drain_timeout = tokio::time::Duration::from_secs(30);
        let _ = tokio::time::timeout(drain_timeout, async {
            loop {
                let msg = client.recv().await?;
                // $/progress { value: { kind: "end" } } signals workspace load complete
                if let Some("$/progress") = msg.get("method").and_then(|v| v.as_str()) {
                    if msg.pointer("/params/value/kind").and_then(|v| v.as_str()) == Some("end") {
                        // Check if this is a workspace/project loading completion
                        let token = msg.pointer("/params/token")
                            .and_then(|v| v.as_str().map(str::to_string)
                                .or_else(|| v.as_u64().map(|n| n.to_string())));
                        if let Some(tok) = token {
                            // rust-analyzer uses tokens like "rustAnalyzer/roots scanned" or numeric ids
                            // We wait until we see any "end" progress for the first numeric token (workspace loading)
                            if tok == "0" || tok.contains("root") || tok.contains("workspace")
                                || tok.contains("cargo") || tok.contains("index")
                            {
                                return Ok::<(), anyhow::Error>(());
                            }
                        }
                    }
                }
            }
        }).await;

        Ok(client)
    }

    /// Open a file so the server can analyse it.
    pub async fn open_file(&mut self, path: &Path, text: &str, language_id: &str) -> Result<()> {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri":        path_to_uri(path),
                    "languageId": language_id,
                    "version":    1,
                    "text":       text,
                }
            }
        })).await
    }

    /// Request hover information at (line, col) — both 0-indexed.
    /// Returns the plain-text content of the hover, or None if no hover available.
    pub async fn hover(&mut self, path: &Path, line: u32, col: u32) -> Result<Option<String>> {
        let id = self.next_id();
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "textDocument/hover",
            "params": {
                "textDocument": { "uri": path_to_uri(path) },
                "position": { "line": line, "character": col }
            }
        })).await?;

        // Collect messages until we get the response for this id
        let timeout = tokio::time::Duration::from_secs(5);
        let result = tokio::time::timeout(timeout, async {
            loop {
                let msg = self.recv().await?;
                if msg.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    return Ok::<Value, anyhow::Error>(msg);
                }
            }
        }).await.map_err(|_| anyhow!("hover timeout"))??;

        // Extract plain text from result.contents
        let contents = result.pointer("/result/contents");
        let text = match contents {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => o.get("value").and_then(|v| v.as_str()).map(str::to_string),
            Some(Value::Array(arr)) => {
                // MarkedString array: take first element
                arr.first().and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(o) => o.get("value").and_then(|v| v.as_str()).map(str::to_string),
                    _ => None,
                })
            }
            _ => None,
        };
        Ok(text)
    }

    /// Graceful shutdown.
    pub async fn shutdown(&mut self) -> Result<()> {
        let id = self.next_id();
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"shutdown","params":{}})).await?;
        let timeout = tokio::time::Duration::from_secs(3);
        let _ = tokio::time::timeout(timeout, async {
            loop {
                let msg = self.recv().await?;
                if msg.get("id").and_then(|v| v.as_u64()) == Some(id) { break; }
            }
            Ok::<(), anyhow::Error>(())
        }).await;
        let _ = self.send(json!({"jsonrpc":"2.0","method":"exit","params":{}})).await;
        Ok(())
    }

    // ── internal ────────────────────────────────────────────────────────────

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    async fn send(&mut self, msg: Value) -> Result<()> {
        let body = msg.to_string();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.stdin.write_all(header.as_bytes()).await?;
        self.stdin.write_all(body.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Value> {
        // Read headers until blank line; extract Content-Length
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).await?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() { break; }
            if let Some(rest) = line.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse().unwrap_or(0);
            }
        }
        if content_length == 0 { return Err(anyhow!("LSP: missing Content-Length")); }
        let mut buf = vec![0u8; content_length];
        self.stdout.read_exact(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(Into::into)
    }
}

fn path_to_uri(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(path)
    };
    format!("file://{}", abs.display())
}
