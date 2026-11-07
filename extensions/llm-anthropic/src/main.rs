//! `atom/llm-anthropic`: `ai.llm@0` sobre a API Anthropic (SSE).
//! Mesmo esqueleto do `llm-openai-compat`: `complete` com stream roda o HTTP
//! bloqueante em `spawn_blocking` e emite `atom.stream.*` entre chunks.

use atom_sdk::{Client, Declare, ErrorCode, Handler, Invoke, ProvidesEntry, RpcError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

struct H {
    cfg: Mutex<Cfg>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Clone)]
struct Cfg {
    base_url: String,
    model: String,
    api_key_from: String,
    max_tokens: u64,
}

impl Cfg {
    fn from(v: &Value) -> Self {
        Self {
            base_url: v
                .get("base_url")
                .and_then(|x| x.as_str())
                .unwrap_or("https://api.anthropic.com")
                .trim_end_matches('/')
                .to_string(),
            model: v
                .get("model")
                .and_then(|x| x.as_str())
                .unwrap_or("claude-sonnet-4-5")
                .to_string(),
            api_key_from: v
                .get("api_key_from")
                .and_then(|x| x.as_str())
                .unwrap_or("anthropic_api_key")
                .to_string(),
            max_tokens: v.get("max_tokens").and_then(|x| x.as_u64()).unwrap_or(4096),
        }
    }
}

impl Default for Cfg {
    fn default() -> Self {
        Self::from(&Value::Null)
    }
}

impl H {
    async fn current_cfg(&self, client: &Client) -> Cfg {
        match client.request("atom.config.get", json!({})).await {
            Ok(v) => {
                let c = Cfg::from(&v);
                *self.cfg.lock().unwrap() = c.clone();
                c
            }
            Err(_) => self.cfg.lock().unwrap().clone(),
        }
    }
}

// --- tradução neutro ↔ Anthropic ---

fn text_of(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|p| {
            if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                p.get("text").and_then(|t| t.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Retorna (system, messages) — system vai no campo próprio da API.
fn to_anthropic(messages: &[Value]) -> (String, Vec<Value>) {
    let mut system = vec![];
    let mut out = vec![];
    for m in messages {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let parts = m
            .get("content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        if role == "system" {
            system.push(text_of(&parts));
            continue;
        }
        if role == "tool" {
            for p in &parts {
                if p.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                    continue;
                }
                let t = &p["tool_result"];
                out.push(json!({"role": "user", "content": [{
                    "type": "tool_result",
                    "tool_use_id": t.get("call_id"),
                    "content": match t.get("content").cloned().unwrap_or(Value::Null) {
                        Value::String(s) => Value::String(s),
                        v => Value::String(serde_json::to_string(&v).unwrap_or_default()),
                    },
                    "is_error": t.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
                }]}));
            }
            continue;
        }
        if role == "assistant" {
            let mut content = vec![];
            let t = text_of(&parts);
            if !t.is_empty() {
                content.push(json!({"type": "text", "text": t}));
            }
            for p in &parts {
                if p.get("type").and_then(|x| x.as_str()) != Some("tool_call") {
                    continue;
                }
                let t = &p["tool_call"];
                content.push(json!({"type": "tool_use",
                    "id": t.get("id"), "name": t.get("name"), "input": t.get("input").cloned().unwrap_or(json!({}))}));
            }
            out.push(json!({"role": "assistant", "content": content}));
            continue;
        }
        out.push(json!({"role": "user", "content": text_of(&parts)}));
    }
    (system.join("\n"), out)
}

fn to_anthropic_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({"name": t.get("name"),
                "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "input_schema": t.get("input_schema").cloned().unwrap_or(json!({"type": "object"}))})
        })
        .collect()
}

async fn api_key(client: &Client, cfg: &Cfg) -> Option<String> {
    match client
        .request("atom.secrets.get", json!({"name": cfg.api_key_from}))
        .await
    {
        Ok(Value::String(s)) if !s.is_empty() => Some(s),
        _ => None,
    }
}

fn body_for(cfg: &Cfg, model: &str, p: &Value, stream: bool) -> Value {
    let messages: Vec<Value> = p
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let (system, msgs) = to_anthropic(&messages);
    let mut body =
        json!({"model": model, "max_tokens": cfg.max_tokens, "messages": msgs, "stream": stream});
    if !system.is_empty() {
        body["system"] = Value::String(system);
    }
    if let Some(t) = p.get("tools").and_then(|t| t.as_array())
        && !t.is_empty()
    {
        body["tools"] = Value::Array(to_anthropic_tools(t));
    }
    body
}

fn send_req(cfg: &Cfg, key: &str, body: &Value) -> Result<reqwest::blocking::Response, String> {
    reqwest::blocking::Client::new()
        .post(format!("{}/v1/messages", cfg.base_url))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(body)
        .send()
        .map_err(|e| format!("http: {e}"))
}

fn do_complete(cfg: &Cfg, key: &str, p: &Value) -> Result<Value, String> {
    let model = p
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(&cfg.model)
        .to_string();
    let resp = send_req(cfg, key, &body_for(cfg, &model, p, false))?;
    let v: Value = resp.json().map_err(|e| format!("json: {e}"))?;
    if v.get("type").and_then(|t| t.as_str()) == Some("error") {
        return Err(v["error"]
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("erro da API")
            .to_string());
    }
    let mut parts = vec![];
    let mut stop = "stop".to_string();
    for b in v
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
    {
        match b.get("type").and_then(|t| t.as_str()) {
            Some("text") => {
                if let Some(t) = b.get("text").and_then(|x| x.as_str()) {
                    parts.push(json!({"type": "text", "text": t}));
                }
            }
            Some("tool_use") => {
                stop = "tool_call".to_string();
                parts.push(json!({"type": "tool_call", "tool_call": {
                    "id": b.get("id"), "name": b.get("name"),
                    "input": b.get("input").cloned().unwrap_or(json!({}))}}));
            }
            _ => {}
        }
    }
    Ok(json!({
        "message": {"role": "assistant", "content": parts},
        "usage": {"input_tokens": v["usage"].get("input_tokens"), "output_tokens": v["usage"].get("output_tokens")},
        "stop_reason": stop,
    }))
}

fn stream_run(
    client: &Client,
    cfg: &Cfg,
    key: &str,
    p: &Value,
    stream: &str,
    cancel: &AtomicBool,
) -> Result<Value, RpcError> {
    let mut seq = 0u64;
    let fail = |client: &Client, msg: String| {
        client.stream_error(
            stream,
            RpcError::new(ErrorCode::INTERNAL_ERROR, msg.clone()),
        );
        Err(RpcError::new(ErrorCode::INTERNAL_ERROR, msg))
    };
    let model = p
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(&cfg.model)
        .to_string();
    let mut resp = match send_req(cfg, key, &body_for(cfg, &model, p, true)) {
        Ok(r) => r,
        Err(e) => return fail(client, e),
    };
    if !resp.status().is_success() {
        let st = resp.status();
        let t = resp.text().unwrap_or_default();
        return fail(client, format!("http {st}: {t}"));
    }
    let mut buf = String::new();
    let mut ev_type = String::new();
    let mut text_acc = String::new();
    // tool_use em curso: (id, name, args acumulados), por índice de bloco.
    let mut calls: HashMap<u64, (String, String, String)> = HashMap::new();
    let mut order: Vec<u64> = vec![];
    let mut usage = json!({});
    let mut stop = "stop".to_string();
    let mut tmp = [0u8; 8192];
    loop {
        if cancel.load(Ordering::Relaxed) {
            let msg = "cancelado".to_string();
            client.stream_error(
                stream,
                RpcError::new(ErrorCode::REQUEST_CANCELLED, msg.clone()),
            );
            return Err(RpcError::new(ErrorCode::REQUEST_CANCELLED, msg));
        }
        let n = match std::io::Read::read(&mut resp, &mut tmp) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let msg = e.to_string();
                client.stream_error(
                    stream,
                    RpcError::new(ErrorCode::INTERNAL_ERROR, msg.clone()),
                );
                return Err(RpcError::new(ErrorCode::INTERNAL_ERROR, msg));
            }
        };
        buf.push_str(&String::from_utf8_lossy(&tmp[..n]));
        while let Some(pos) = buf.find('\n') {
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim();
            if let Some(t) = line.strip_prefix("event:") {
                ev_type = t.trim().to_string();
                continue;
            }
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let ev: Value = match serde_json::from_str(data.trim()) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match ev_type.as_str() {
                "content_block_delta" => {
                    let d = &ev["delta"];
                    match d.get("type").and_then(|t| t.as_str()) {
                        Some("text_delta") => {
                            if let Some(t) = d.get("text").and_then(|v| v.as_str()) {
                                text_acc.push_str(t);
                                seq += 1;
                                client.stream_chunk(stream, seq, json!({"delta_text": t}));
                            }
                        }
                        Some("input_json_delta") => {
                            let idx = ev.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
                            let frag = d.get("partial_json").and_then(|v| v.as_str()).unwrap_or("");
                            if !frag.is_empty() {
                                let e = calls.entry(idx).or_insert_with(|| {
                                    (String::new(), String::new(), String::new())
                                });
                                e.2.push_str(frag);
                                if !order.contains(&idx) {
                                    order.push(idx);
                                }
                                seq += 1;
                                client.stream_chunk(
                                    stream,
                                    seq,
                                    json!({"delta_tool_call": {"id": e.0, "name": e.1, "input_delta": frag}}),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                "content_block_start" => {
                    let b = &ev["content_block"];
                    if b.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let idx = ev.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
                        calls.insert(
                            idx,
                            (
                                b.get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                b.get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                String::new(),
                            ),
                        );
                    }
                }
                "message_delta" => {
                    if let Some(s) = ev["delta"].get("stop_reason").and_then(|v| v.as_str()) {
                        stop = if s == "tool_use" {
                            "tool_call".to_string()
                        } else {
                            s.to_string()
                        };
                    }
                    if ev.get("usage").is_some() {
                        usage = ev["usage"].clone();
                    }
                }
                "message_stop" => {}
                _ => {}
            }
        }
    }
    let mut parts = vec![];
    if !text_acc.is_empty() {
        parts.push(json!({"type": "text", "text": text_acc}));
    }
    for idx in &order {
        if let Some((cid, name, args)) = calls.get(idx) {
            let input: Value = serde_json::from_str(args).unwrap_or(Value::Null);
            parts.push(json!({"type": "tool_call", "tool_call": {"id": cid, "name": name, "input": input}}));
        }
    }
    if !order.is_empty() {
        stop = "tool_call".to_string();
    }
    let result = json!({"message": {"role": "assistant", "content": parts}, "usage": usage, "stop_reason": stop});
    client.stream_end(stream, result.clone());
    Ok(result)
}

impl Handler for H {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.llm@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            ..Declare::default()
        }
    }

    async fn on_invoke(&self, client: &Client, inv: Invoke) -> Result<Value, RpcError> {
        match (inv.service.as_str(), inv.method.as_str()) {
            ("ai.llm@0", "models") => {
                let cfg = self.current_cfg(client).await;
                Ok(json!([{"id": cfg.model}]))
            }
            ("ai.llm@0", "complete") => {
                let cfg = self.current_cfg(client).await;
                let Some(key) = api_key(client, &cfg).await else {
                    let msg = format!(
                        "secret {} ausente (`atom secret set {}`)",
                        cfg.api_key_from, cfg.api_key_from
                    );
                    if let Some(s) = inv.stream.clone() {
                        client.stream_error(
                            &s,
                            RpcError::new(ErrorCode::CAPABILITY_DENIED, msg.clone()),
                        );
                    }
                    return Err(RpcError::new(ErrorCode::CAPABILITY_DENIED, msg));
                };
                match inv.stream.clone() {
                    Some(s) => {
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancels.lock().unwrap().insert(s.clone(), flag.clone());
                        let c2 = client.clone();
                        let (cfg_c, p_c) = (cfg.clone(), inv.params.clone());
                        let out = tokio::task::spawn_blocking(move || {
                            stream_run(&c2, &cfg_c, &key, &p_c, &s, &flag)
                        })
                        .await
                        .map_err(|e| RpcError::new(ErrorCode::INTERNAL_ERROR, e.to_string()))?;
                        self.cancels.lock().unwrap().remove(&inv.stream.unwrap());
                        out
                    }
                    None => do_complete(&cfg, &key, &inv.params)
                        .map_err(|e| RpcError::new(ErrorCode::INTERNAL_ERROR, e)),
                }
            }
            (s, m) => Err(RpcError::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("método {s}/{m} desconhecido"),
            )),
        }
    }

    async fn on_stream_cancel(&self, _client: &Client, stream: &str) {
        if let Some(f) = self.cancels.lock().unwrap().get(stream) {
            f.store(true, Ordering::Relaxed);
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(H {
        cfg: Mutex::new(Cfg::default()),
        cancels: Mutex::new(HashMap::new()),
    })
    .await;
}
