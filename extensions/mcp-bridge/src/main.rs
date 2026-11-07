//! `atom/mcp-bridge`: handler `kind=mcp`, provider `ai.tools@0`.
//! Cada pacote `kind=mcp` declara `[contributes.mcp]` com
//! `transport = "stdio"|"http"`, `command`/`args` ou `url`.
//! `list` agrega tools com prefixo `{server}__`; `execute` roteia.
//! Conexões são por chamada (spawn/POST + initialize), sem estado.
//! `shortcut: stdio NDJSON e HTTP JSON-RPC simples; sem SSE/sessão/resume.`

use atom_sdk::{Declare, ErrorCode, Invoke, KindPackage, ProvidesEntry, RpcError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Mutex;

struct H {
    servers: Mutex<HashMap<String, (ServerCfg, String)>>,
}

#[derive(Debug, Clone)]
struct ServerCfg {
    transport: String,
    command: Option<String>,
    args: Vec<String>,
    url: Option<String>,
}

fn short_name(pkg: &str) -> String {
    pkg.split('/').next_back().unwrap_or(pkg).replace('-', "_")
}

fn tool_full(pkg: &str, tool: &str) -> String {
    format!("{}__{tool}", short_name(pkg))
}

fn mcp_stdio(cfg: &ServerCfg, dir: &str, method: &str, params: Value) -> Result<Value, String> {
    let cmd = cfg.command.clone().ok_or("mcp stdio sem command")?;
    let prog = if std::path::Path::new(&cmd).is_absolute() {
        cmd
    } else {
        format!("{dir}/{cmd}")
    };
    let mut child = Command::new(&prog)
        .args(&cfg.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn {prog}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("sem stdin")?;
    let stdout = child.stdout.take().ok_or("sem stdout")?;
    let mut reader = BufReader::new(stdout);
    let mut write_line = |v: &Value| -> Result<(), String> {
        writeln!(stdin, "{}", serde_json::to_string(v).unwrap()).map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())
    };
    write_line(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "atom-mcp-bridge", "version": "0.4.0"}}}))?;
    write_line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
    write_line(&json!({"jsonrpc": "2.0", "id": 2, "method": method, "params": params}))?;
    // Respostas: id 1 (initialize) e id 2 (chamada real).
    let mut result: Option<Value> = None;
    let mut line = String::new();
    for _ in 0..50 {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if line.trim().is_empty() {
                    continue;
                }
                let v: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if v.get("id") == Some(&json!(2)) {
                    if let Some(e) = v.get("error") {
                        let _ = child.kill();
                        return Err(format!("mcp erro: {e}"));
                    }
                    result = v.get("result").cloned();
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = child.kill();
    result.ok_or_else(|| "mcp sem resposta".to_string())
}

fn mcp_http(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let client = reqwest::blocking::Client::new();
    for (id, m, p) in [
        (
            1,
            "initialize",
            json!({"protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": {"name": "atom-mcp-bridge", "version": "0.4.0"}}),
        ),
        (2, method, params),
    ] {
        let body = json!({"jsonrpc": "2.0", "id": id, "method": m, "params": p});
        let resp: Value = client
            .post(url)
            .json(&body)
            .send()
            .map_err(|e| format!("http {url}: {e}"))?
            .json()
            .map_err(|e| format!("http json: {e}"))?;
        if id == 2 {
            if let Some(e) = resp.get("error") {
                return Err(format!("mcp erro: {e}"));
            }
            return resp
                .get("result")
                .cloned()
                .ok_or_else(|| "mcp sem result".to_string());
        }
    }
    Err("inacessível".to_string())
}

fn server_call(cfg: &ServerCfg, dir: &str, method: &str, params: Value) -> Result<Value, String> {
    if cfg.transport == "http" {
        let url = cfg.url.clone().ok_or("mcp http sem url")?;
        mcp_http(&url, method, params)
    } else {
        mcp_stdio(cfg, dir, method, params)
    }
}

async fn blocking_call(
    cfg: ServerCfg,
    dir: String,
    method: String,
    params: Value,
) -> Result<Value, String> {
    tokio::task::spawn_blocking(move || server_call(&cfg, &dir, &method, params))
        .await
        .map_err(|e| e.to_string())?
}

impl H {
    async fn list(&self) -> Value {
        let snapshots: Vec<(String, ServerCfg, String)> = {
            let mut names: Vec<String> = self
                .servers
                .lock()
                .map(|s| s.keys().cloned().collect())
                .unwrap_or_default();
            names.sort();
            names
                .into_iter()
                .filter_map(|srv| {
                    self.servers
                        .lock()
                        .ok()?
                        .get(&srv)
                        .cloned()
                        .map(|(cfg, dir)| (srv, cfg, dir))
                })
                .collect()
        };
        let mut tools: Vec<Value> = vec![];
        for (srv, cfg, dir) in snapshots {
            match blocking_call(cfg, dir, "tools/list".to_string(), json!({})).await {
                Ok(r) => {
                    let arr = r
                        .get("tools")
                        .and_then(|t| t.as_array())
                        .cloned()
                        .unwrap_or_default();
                    for t in arr {
                        let tn = t.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                        tools.push(json!({
                            "name": tool_full(&srv, tn),
                            "description": t.get("description").cloned().unwrap_or(json!("")),
                            "input_schema": t.get("inputSchema").cloned().unwrap_or(json!({"type": "object"})),
                        }));
                    }
                }
                Err(e) => eprintln!("mcp {srv} list: {e}"),
            }
        }
        Value::Array(tools)
    }

    async fn execute(&self, p: &Value) -> Result<Value, RpcError> {
        let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let input = p.get("input").cloned().unwrap_or(Value::Null);
        // Roteia pelo prefixo `{server}__`; sem prefixo, primeiro dono.
        let target: Option<(String, ServerCfg, String)> = {
            let guard = self
                .servers
                .lock()
                .map_err(|_| RpcError::new(ErrorCode::INTERNAL_ERROR, "estado inacessível"))?;
            let mut found = None;
            for (srv, (cfg, dir)) in guard.iter() {
                let prefix = format!("{}__", short_name(srv));
                if name.strip_prefix(&prefix).is_some() {
                    found = Some((srv.clone(), cfg.clone(), dir.clone()));
                    break;
                }
            }
            if found.is_none() {
                let mut names: Vec<String> = guard.keys().cloned().collect();
                names.sort();
                if let Some(srv) = names.first() {
                    let (cfg, dir) = guard.get(srv).cloned().unwrap();
                    found = Some((srv.clone(), cfg, dir));
                }
            }
            found
        };
        let tool = match &target {
            Some((srv, _, _)) => {
                let prefix = format!("{}__", short_name(srv));
                name.strip_prefix(&prefix).unwrap_or(name).to_string()
            }
            None => {
                return Err(RpcError::new(ErrorCode::NO_PROVIDER, "sem servidor mcp"));
            }
        };
        let (_, cfg, dir) = target.unwrap();
        match blocking_call(
            cfg,
            dir,
            "tools/call".to_string(),
            json!({"name": tool, "arguments": input}),
        )
        .await
        {
            Ok(r) => {
                let content = r
                    .get("content")
                    .and_then(|c| c.as_array())
                    .cloned()
                    .unwrap_or_default();
                let is_error = r.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
                let output: Vec<Value> = content
                    .iter()
                    .map(|c| {
                        if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                            json!({"type": "text", "text": c.get("text").cloned().unwrap_or(json!(""))})
                        } else {
                            json!({"type": "text", "text": serde_json::to_string(c).unwrap_or_default()})
                        }
                    })
                    .collect();
                Ok(json!({"output": output, "is_error": is_error}))
            }
            Err(e) => Ok(
                json!({"output": [{"type": "text", "text": format!("mcp: {e}")}], "is_error": true}),
            ),
        }
    }
}

impl atom_sdk::Handler for H {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.tools@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            handles: vec!["mcp".to_string()],
            ..Declare::default()
        }
    }

    async fn on_invoke(&self, _client: &atom_sdk::Client, inv: Invoke) -> Result<Value, RpcError> {
        match inv.method.as_str() {
            "list" => Ok(self.list().await),
            "execute" => self.execute(&inv.params).await,
            m => Err(RpcError::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("método {m} desconhecido"),
            )),
        }
    }

    async fn on_kind_load(
        &self,
        _client: &atom_sdk::Client,
        pkg: KindPackage,
    ) -> Result<(), RpcError> {
        let dir = pkg.dir;
        let manifest = pkg.manifest;
        let name = manifest
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string();
        let mc = manifest
            .get("contributes")
            .and_then(|c| c.get("mcp"))
            .cloned()
            .unwrap_or(Value::Null);
        let cfg = ServerCfg {
            transport: mc
                .get("transport")
                .and_then(|t| t.as_str())
                .unwrap_or("stdio")
                .to_string(),
            command: mc
                .get("command")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            args: mc
                .get("args")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            url: mc
                .get("url")
                .and_then(|u| u.as_str())
                .map(|s| s.to_string()),
        };
        if let Ok(mut servers) = self.servers.lock() {
            servers.insert(name, (cfg, dir));
        }
        Ok(())
    }

    async fn on_kind_unload(&self, _client: &atom_sdk::Client, package: &str) {
        let name = package.split('@').next().unwrap_or(package);
        if let Ok(mut servers) = self.servers.lock() {
            servers.remove(name);
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(H {
        servers: Mutex::new(HashMap::new()),
    })
    .await
}
