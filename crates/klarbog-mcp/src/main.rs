//! Minimal MCP-shaped stdio server (tools/list + tools/call). Two-phase confirm.

mod tools;

use klarbog_core::{default_registry, ConfirmStore};
use klarbog_plugin::Registry;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::sync::{Arc, OnceLock};

fn confirm_store() -> Arc<ConfirmStore> {
    static STORE: OnceLock<Arc<ConfirmStore>> = OnceLock::new();
    STORE
        .get_or_init(|| Arc::new(ConfirmStore::default()))
        .clone()
}

fn plugin_registry() -> Arc<Registry> {
    static REG: OnceLock<Arc<Registry>> = OnceLock::new();
    REG.get_or_init(|| Arc::new(default_registry())).clone()
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("tokio runtime"))
}

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let store = confirm_store();
    let registry = plugin_registry();
    let allowlist = tools::default_allowlist_root();
    let rt = runtime();
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
                let mut r = tools::tools_list();
                r["id"] = id;
                r
            }
            "tools/call" => {
                let name = req
                    .pointer("/params/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = req
                    .pointer("/params/arguments")
                    .cloned()
                    .unwrap_or(json!({}));
                let body = tools::handle_tool_call(name, &args, &store, &allowlist, &registry, rt);
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": serde_json::to_string(&body)? }]
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
