//! `atom/llm-openai-compat`: `ai.llm@0` sobre API OpenAI-compatível.
//! Cada `complete` com stream roda o HTTP bloqueante em `spawn_blocking` e
//! emite `atom.stream.chunk/end` progressivamente; `atom.stream.cancel`
//! sinaliza via `AtomicBool` por stream.

use atom_sdk::{Client, Declare, ErrorCode, Handler, Invoke, ProvidesEntry, RpcError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

struct Llm {
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Clone)]
struct Cfg {
    base_url: String,
    model: String,
    api_key_from: String,
}

impl Cfg {
    fn from(v: &Value) -> Self {
        Self {
            base_url: v
                .get("base_url")
                .and_then(|x| x.as_str())
                .unwrap_or("https://api.openai.com/v1")
                .trim_end_matches('/')
                .to_string(),
            model: v
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("gpt-4o-mini")
                .to_string(),
            api_key_from: v
                .get("api_key_from")
                .and_then(|x| x.as_str())
                .unwrap_or("openai_api_key")
                .to_string(),
        }
    }

    fn local(&self) -> bool {
        self.base_url.contains("localhost") || self.base_url.contains("127.0.0.1")
    }
}

async fn config_of(client: &Client) -> Cfg {
    let v = client
        .request("atom.config.get", json!({}))
        .await
        .unwrap_or(Value::Null);
    Cfg::from(&v)
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

// --- tradução neutro ↔ OpenAI ---

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

fn to_openai_msg(m: &Value) -> Value {
    let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
    let parts = m
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    match role {
        "tool" => {
            let tr = parts
                .iter()
                .find(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_result"));
            let (call_id, content) = tr
                .map(|p| {
                    let t = &p["tool_result"];
                    (
                        t.get("call_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        t.get("content").cloned().unwrap_or(Value::Null),
                    )
                })
                .unwrap_or_default();
            let content = match content {
                Value::String(s) => s,
                v => serde_json::to_string(&v).unwrap_or_default(),
            };
            json!({"role": "tool", "tool_call_id": call_id, "content": content})
        }
        "assistant" => {
            let calls: Vec<Value> = parts
                .iter()
                .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_call"))
                .map(|p| {
                    let t = &p["tool_call"];
                    json!({"id": t.get("id"), "type": "function",
                        "function": {"name": t.get("name"), "arguments": serde_json::to_string(&t.get("input").cloned().unwrap_or(Value::Null)).unwrap_or_default()}})
                })
                .collect();
            let mut o = json!({"role": "assistant", "content": text_of(&parts)});
            if !calls.is_empty() {
                o["tool_calls"] = Value::Array(calls);
            }
            o
        }
        _ => json!({"role": role, "content": text_of(&parts)}),
    }
}

fn to_openai_tools(tools: &[Value]) -> Vec<Value> {
    tools
        .iter()
        .map(|t| {
            json!({"type": "function", "function": {
                "name": t.get("name"),
                "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "parameters": t.get("input_schema").cloned().unwrap_or(json!({"type": "object"})),
            }})
        })
        .collect()
}

fn from_openai_msg(m: &Value) -> (Value, String) {
    let mut parts = vec![];
    if let Some(t) = m.get("content").and_then(|c| c.as_str())
        && !t.is_empty()
    {
        parts.push(json!({"type": "text", "text": t}));
    }
    for tc in m
        .get("tool_calls")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default()
    {
        let f = &tc["function"];
        let input: Value = f
            .get("arguments")
            .and_then(|a| a.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null);
        parts.push(json!({"type": "tool_call", "tool_call": {
            "id": tc.get("id").and_then(|v| v.as_str()).unwrap_or(""),
            "name": f.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            "input": input}}));
    }
    let stop = "stop".to_string();
    (json!({"role": "assistant", "content": parts}), stop)
}

fn map_stop(finish: &str) -> &str {
    match finish {
        "tool_calls" => "tool_call",
        "length" => "length",
        "content_filter" | "error" => "error",
        _ => "stop",
    }
}

fn stream_fail(client: &Client, stream: &str, code: i32, msg: String) -> Result<Value, RpcError> {
    client.stream_error(stream, RpcError::new(code, msg.clone()));
    Err(RpcError::new(code, msg))
}

fn fetch_models(cfg: &Cfg, key: Option<&str>) -> Value {
    let mut req = reqwest::blocking::Client::new().get(format!("{}/models", cfg.base_url));
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    req.send()
        .ok()
        .and_then(|r| r.json::<Value>().ok())
        .and_then(|v| v.get("data").cloned())
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|x| json!({"id": x.get("id")}))
        .collect()
}

fn do_complete(cfg: &Cfg, key: Option<&str>, p: &Value) -> Result<Value, String> {
    let model = p
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(&cfg.model)
        .to_string();
    let messages: Vec<Value> = p
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(to_openai_msg)
        .collect();
    let mut body = json!({"model": model, "messages": messages, "stream": false});
    if let Some(t) = p.get("tools").and_then(|t| t.as_array())
        && !t.is_empty()
    {
        body["tools"] = Value::Array(to_openai_tools(t));
    }
    if let Some(extra) = p.get("params")
        && let (Some(b), Some(e)) = (body.as_object_mut(), extra.as_object())
    {
        for (k, v) in e {
            b.insert(k.clone(), v.clone());
        }
    }
    let client = reqwest::blocking::Client::new();
    let mut req = client
        .post(format!("{}/chat/completions", cfg.base_url))
        .json(&body);
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let resp = req.send().map_err(|e| format!("http: {e}"))?;
    let v: Value = resp.json().map_err(|e| format!("json: {e}"))?;
    if let Some(e) = v.get("error") {
        return Err(e
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("erro da API")
            .to_string());
    }
    let choice = &v["choices"][0];
    let (message, _) = from_openai_msg(&choice["message"]);
    let finish = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("stop");
    Ok(json!({
        "message": message,
        "usage": v.get("usage"),
        "stop_reason": map_stop(finish),
    }))
}

/// Laço SSE bloqueante; emite chunks ao vivo via `client`. Roda em
/// `spawn_blocking`; o resultado final (ou erro) é retornado para o
/// `on_invoke` responder o invoke.
fn run_stream(
    cfg: Cfg,
    key: Option<String>,
    p: Value,
    stream: String,
    cancel: Arc<AtomicBool>,
    client: Client,
) -> Result<Value, RpcError> {
    let model = p
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or(&cfg.model)
        .to_string();
    let messages: Vec<Value> = p
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(to_openai_msg)
        .collect();
    let mut body = json!({"model": model, "messages": messages, "stream": true, "stream_options": {"include_usage": true}});
    if let Some(t) = p.get("tools").and_then(|t| t.as_array())
        && !t.is_empty()
    {
        body["tools"] = Value::Array(to_openai_tools(t));
    }
    let client_http = reqwest::blocking::Client::new();
    let mut req = client_http
        .post(format!("{}/chat/completions", cfg.base_url))
        .json(&body);
    if let Some(k) = &key {
        req = req.bearer_auth(k);
    }
    let mut resp = match req.send() {
        Ok(r) => r,
        Err(e) => {
            return stream_fail(
                &client,
                &stream,
                ErrorCode::INTERNAL_ERROR,
                format!("http: {e}"),
            );
        }
    };
    if !resp.status().is_success() {
        let st = resp.status();
        let t = resp.text().unwrap_or_default();
        return stream_fail(
            &client,
            &stream,
            ErrorCode::INTERNAL_ERROR,
            format!("http {st}: {t}"),
        );
    }
    let mut seq = 0u64;
    let mut buf = String::new();
    let mut text_acc = String::new();
    // (id, name, args acumulados) por índice de tool_call.
    let mut calls: HashMap<String, (String, String, String)> = HashMap::new();
    let mut order: Vec<String> = vec![];
    let mut usage = Value::Null;
    let mut finish = "stop".to_string();
    let mut tmp = [0u8; 8192];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return stream_fail(
                &client,
                &stream,
                ErrorCode::REQUEST_CANCELLED,
                "cancelado".to_string(),
            );
        }
        let n = match std::io::Read::read(&mut resp, &mut tmp) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                return stream_fail(&client, &stream, ErrorCode::INTERNAL_ERROR, e.to_string());
            }
        };
        buf.push_str(&String::from_utf8_lossy(&tmp[..n]));
        while let Some(pos) = buf.find('\n') {
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                continue;
            }
            let ev: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(u) = ev.get("usage") {
                usage = u.clone();
            }
            let choice = &ev["choices"][0];
            if let Some(f) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                finish = f.to_string();
            }
            let delta = &choice["delta"];
            if let Some(t) = delta.get("content").and_then(|v| v.as_str())
                && !t.is_empty()
            {
                text_acc.push_str(t);
                seq += 1;
                client.stream_chunk(&stream, seq, json!({"delta_text": t}));
            }
            for tc in delta
                .get("tool_calls")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
            {
                let idx = tc
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .to_string();
                let entry = calls
                    .entry(idx.clone())
                    .or_insert_with(|| (String::new(), String::new(), String::new()));
                if let Some(id) = tc.get("id").and_then(|v| v.as_str())
                    && !id.is_empty()
                    && entry.0.is_empty()
                {
                    entry.0 = id.to_string();
                }
                if let Some(n) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|v| v.as_str())
                    && !n.is_empty()
                {
                    entry.1 = n.to_string();
                }
                let frag = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !frag.is_empty() {
                    if entry.0.is_empty() {
                        entry.0 = format!("call_{idx}");
                    }
                    entry.2.push_str(frag);
                    if !order.contains(&idx) {
                        order.push(idx.clone());
                    }
                    seq += 1;
                    client.stream_chunk(
                        &stream,
                        seq,
                        json!({"delta_tool_call": {"id": entry.0, "name": entry.1, "input_delta": frag}}),
                    );
                }
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
            parts.push(json!({"type": "tool_call", "tool_call": {
                "id": cid, "name": name, "input": input}}));
        }
    }
    let stop_reason = if order.is_empty() {
        map_stop(&finish).to_string()
    } else {
        "tool_call".to_string()
    };
    let result = json!({
        "message": {"role": "assistant", "content": parts},
        "usage": usage,
        "stop_reason": stop_reason,
    });
    client.stream_end(&stream, result.clone());
    Ok(result)
}

fn join_err(e: tokio::task::JoinError) -> RpcError {
    RpcError::new(ErrorCode::INTERNAL_ERROR, format!("join: {e}"))
}

impl Handler for Llm {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.llm@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            ..Default::default()
        }
    }

    async fn on_invoke(&self, client: &Client, inv: Invoke) -> Result<Value, RpcError> {
        match (inv.service.as_str(), inv.method.as_str()) {
            ("ai.llm@0", "models") => {
                let cfg = config_of(client).await;
                let key = api_key(client, &cfg).await;
                let out = tokio::task::spawn_blocking(move || fetch_models(&cfg, key.as_deref()))
                    .await
                    .map_err(join_err)?;
                Ok(out)
            }
            ("ai.llm@0", "complete") => {
                let cfg = config_of(client).await;
                let key = api_key(client, &cfg).await;
                if key.is_none() && !cfg.local() {
                    let msg = format!(
                        "secret {} ausente (`atom secret set {}`)",
                        cfg.api_key_from, cfg.api_key_from
                    );
                    if let Some(s) = inv.stream.as_deref() {
                        client.stream_error(
                            s,
                            RpcError::new(ErrorCode::CAPABILITY_DENIED, msg.clone()),
                        );
                    }
                    return Err(RpcError::new(ErrorCode::CAPABILITY_DENIED, msg));
                }
                match inv.stream.clone() {
                    Some(s) => {
                        let flag = Arc::new(AtomicBool::new(false));
                        self.cancels.lock().unwrap().insert(s.clone(), flag.clone());
                        let c2 = client.clone();
                        let res = tokio::task::spawn_blocking(move || {
                            run_stream(cfg, key, inv.params, s, flag, c2)
                        })
                        .await
                        .map_err(join_err)?;
                        if let Some(sid) = inv.stream.as_deref() {
                            self.cancels.lock().unwrap().remove(sid);
                        }
                        res
                    }
                    None => {
                        let res = tokio::task::spawn_blocking(move || {
                            do_complete(&cfg, key.as_deref(), &inv.params)
                        })
                        .await
                        .map_err(join_err)?;
                        res.map_err(|e| RpcError::new(ErrorCode::INTERNAL_ERROR, e))
                    }
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
    atom_sdk::run(Llm {
        cancels: Mutex::new(HashMap::new()),
    })
    .await;
}
