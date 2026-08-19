//! Minimal MCP-shaped stdio stub (tools/list). Full rmcp wiring in later slice.
//! Two-phase confirm is documented here; mutating tools arrive in slice 2.

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn tools_list() -> Value {
    json!({
        "jsonrpc": "2.0",
        "result": {
            "tools": [
                {
                    "name": "klarbog_health",
                    "description": "Health check (read-only)",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "journal_post_preview",
                    "description": "Phase-1: validate entry and return confirmation token (no write)",
                    "inputSchema": { "type": "object", "properties": { "company": {"type":"string"} } }
                },
                {
                    "name": "journal_post_commit",
                    "description": "Phase-2: commit with confirmation token (ADR two-phase confirm)",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "company": {"type":"string"},
                            "confirm_token": {"type":"string"}
                        },
                        "required": ["confirm_token"]
                    }
                }
            ]
        }
    })
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = serde_json::from_str(&line)?;
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "klarbog-mcp", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Klarbog DEV MCP. Writes require two-phase confirm tokens."
                }
            }),
            "tools/list" => {
                let mut r = tools_list();
                r["id"] = id;
                r
            }
            "tools/call" => {
                let name = req
                    .pointer("/params/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let body = if name == "klarbog_health" {
                    json!({"ok": true, "service": "klarbog-mcp"})
                } else {
                    json!({
                        "ok": false,
                        "errors": ["not implemented in slice 0/1 stub — see journal_post_preview/commit in slice 2"]
                    })
                };
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": body.to_string() }]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("method not found: {method}") }
            }),
        };
        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}
