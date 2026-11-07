//! `atom/agent-loop-basic`: `ai.agent@0` em loop simples (llm ↔ tools).
//! Um `run` ativo por vez; máquina de estados sobre o atom-sdk.
//! O loop do `run` roda dentro do `on_invoke` (a resposta do invoke sai no
//! fim, como no original manual); eventos ao vivo vão no stream do chamador.
//! Só o `complete` do LLM usa `stream=true` — os chunks chegam em
//! `on_stream_chunk/end` e são re-seriados no turno via canal interno.

use atom_sdk::{
    Client, Declare, ErrorCode, Handler, Invoke, ProvidesEntry, RpcError, StreamChunk, StreamEnd,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Default)]
struct Cfg {
    model: Option<String>,
    llm_target: Option<String>,
    max_turns: u64,
}

impl Cfg {
    fn from(v: &Value) -> Self {
        Self {
            model: v
                .get("model")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            llm_target: v
                .get("llm_target")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            max_turns: v
                .get("max_turns")
                .and_then(|x| x.as_u64())
                .unwrap_or(10)
                .clamp(1, 50),
        }
    }
}

#[derive(Clone)]
struct Run {
    session: String,
    stream: Option<String>,
    history: Vec<Value>,
    /// tool → (provedor, definição completa do `list`).
    tools: HashMap<String, (String, Value)>,
    llm_stream: Option<String>,
    turns: u64,
    max_turns: u64,
    model: Option<String>,
    workdir: Option<String>,
    cancelled: bool,
    llm_target: Option<String>,
}

enum LlmEvent {
    Chunk(Value),
    End {
        result: Option<Value>,
        error: Option<RpcError>,
    },
}

#[derive(Default)]
struct State {
    sessions: HashMap<String, Vec<Value>>,
    run: Option<Run>,
    /// llm_stream → entrega de chunks/fim ao turno em espera.
    llm_tx: HashMap<String, mpsc::UnboundedSender<LlmEvent>>,
}

#[derive(Default)]
struct AgentLoop {
    state: Mutex<State>,
}

async fn config_of(client: &Client) -> Cfg {
    let v = client
        .request("atom.config.get", json!({}))
        .await
        .unwrap_or(Value::Null);
    Cfg::from(&v)
}

/// Emite evento do agente no próprio stream (só se o chamador pediu stream).
fn emit(client: &Client, stream: &Option<String>, seq: &mut u64, data: Value) {
    if let Some(s) = stream {
        *seq += 1;
        client.stream_chunk(s, *seq, data);
    }
}

fn system_text(workdir: &Option<String>) -> String {
    let mut s = String::from(
        "You are an agent operating on the user's machine. Use tools to act; \
         answer concisely with what was done. Always use absolute paths.",
    );
    if let Some(w) = workdir {
        s.push_str(&format!(" Working directory: {w}."));
    }
    s
}

fn text_of_parts(parts: &[Value]) -> String {
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

fn text_of_result(v: &Value) -> String {
    v.get("output")
        .and_then(|o| o.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| serde_json::to_string(v).unwrap_or_default())
}

/// Prefixa `path` relativo com workdir (tools de arquivo).
fn absolutize(input: &mut Value, workdir: &Option<String>) {
    if let (Some(w), Some(obj)) = (workdir, input.as_object_mut())
        && let Some(p) = obj.get("path").and_then(|x| x.as_str())
        && !p.starts_with('/')
        && !p.is_empty()
    {
        obj.insert("path".to_string(), Value::String(format!("{w}/{p}")));
    }
}

impl AgentLoop {
    fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .map(|st| st.run.as_ref().is_some_and(|r| r.cancelled))
            .unwrap_or(false)
    }

    fn take_run(&self) -> Option<Run> {
        self.state.lock().ok().and_then(|mut st| st.run.take())
    }

    /// Checkpoint de cancelamento: finaliza como cancelado se sinalizado.
    fn cancelled_err(&self, client: &Client) -> Option<RpcError> {
        if self.is_cancelled() {
            match self.take_run() {
                Some(run) => Some(self.finish_cancelled(client, run)),
                None => Some(RpcError::new(ErrorCode::REQUEST_CANCELLED, "cancelado")),
            }
        } else {
            None
        }
    }

    fn persist(&self, run: &Run) {
        if let Ok(mut st) = self.state.lock() {
            st.sessions.insert(run.session.clone(), run.history.clone());
        }
    }

    fn finish_ok(&self, client: &Client, run: Run, text: String) -> Value {
        self.persist(&run);
        if let Some(s) = &run.stream {
            client.stream_end(s, json!({"message": text}));
        }
        json!({"message": text})
    }

    fn finish_failed(&self, client: &Client, run: Run, text: String) -> RpcError {
        self.persist(&run);
        if let Some(s) = &run.stream {
            client.stream_error(s, RpcError::new(ErrorCode::INTERNAL_ERROR, text.clone()));
        }
        RpcError::new(ErrorCode::INTERNAL_ERROR, text)
    }

    fn finish_cancelled(&self, client: &Client, run: Run) -> RpcError {
        self.persist(&run);
        if let Some(s) = &run.stream {
            client.stream_error(s, RpcError::new(ErrorCode::REQUEST_CANCELLED, "cancelado"));
        }
        RpcError::new(ErrorCode::REQUEST_CANCELLED, "cancelado")
    }

    fn fail_take(&self, client: &Client, text: String) -> RpcError {
        match self.take_run() {
            Some(run) => self.finish_failed(client, run, text),
            None => RpcError::new(ErrorCode::INTERNAL_ERROR, text),
        }
    }

    /// Loop do `run`: união de tools + turnos llm ↔ tools até stop.
    /// Roda dentro do `on_invoke`; a resposta do invoke sai no retorno,
    /// como o `end_run`/`finish_cancelled` do original.
    async fn drive_run(&self, client: &Client) -> Result<Value, RpcError> {
        let mut seq = 0u64;
        // União de tools (sequencial; o original era dirigido a eventos).
        let reg = client
            .request("atom.registry.list", json!({}))
            .await
            .unwrap_or(Value::Null);
        let providers: Vec<String> = reg
            .get("providers")
            .and_then(|x| x.get("ai.tools@0"))
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect();
        let mut tools: HashMap<String, (String, Value)> = HashMap::new();
        for pn in providers {
            if let Some(e) = self.cancelled_err(client) {
                return Err(e);
            }
            let defs = client
                .call("ai.tools@0", "list", json!({}), Some(&pn), false)
                .await
                .unwrap_or(Value::Null);
            for d in defs.as_array().cloned().unwrap_or_default() {
                if let Some(n) = d.get("name").and_then(|x| x.as_str()) {
                    tools.entry(n.to_string()).or_insert((pn.clone(), d));
                }
            }
        }
        let snap0: Option<Run> = self.state.lock().ok().and_then(|mut st| {
            st.run.as_mut().map(|r| {
                r.tools = tools;
                r.clone()
            })
        });
        let Some(stream0) = snap0.map(|r| r.stream.clone()) else {
            return Err(RpcError::new(ErrorCode::INTERNAL_ERROR, "run perdido"));
        };
        emit(
            client,
            &stream0,
            &mut seq,
            json!({"type": "status", "text": "pensando..."}),
        );

        loop {
            if let Some(e) = self.cancelled_err(client) {
                return Err(e);
            }
            // Foto do turno + conta (era o `start_turn`).
            let snap: Run = match self.state.lock().ok().and_then(|mut st| {
                st.run.as_mut().map(|r| {
                    r.turns += 1;
                    r.clone()
                })
            }) {
                Some(r) => r,
                None => {
                    return Err(RpcError::new(ErrorCode::INTERNAL_ERROR, "run perdido"));
                }
            };
            let mut messages = vec![
                json!({"role": "system", "content": [{"type": "text", "text": system_text(&snap.workdir)}]}),
            ];
            messages.extend(snap.history.clone());
            let mut p = json!({"messages": messages});
            if let Some(m) = &snap.model {
                p["model"] = json!(m);
            }
            // tools no formato neutro do contrato (definições reais da união).
            let defs: Vec<Value> = snap.tools.values().map(|(_, d)| d.clone()).collect();
            if !defs.is_empty() {
                p["tools"] = Value::Array(defs);
            }
            let imm = match client
                .call("ai.llm@0", "complete", p, snap.llm_target.as_deref(), true)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    let text = format!("llm: {e}");
                    emit(
                        client,
                        &snap.stream,
                        &mut seq,
                        json!({"type": "done", "message": text}),
                    );
                    return Err(self.fail_take(client, text));
                }
            };
            let ls = imm
                .get("stream")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if ls.is_empty() {
                let text = "llm: resposta sem stream".to_string();
                emit(
                    client,
                    &snap.stream,
                    &mut seq,
                    json!({"type": "done", "message": text}),
                );
                return Err(self.fail_take(client, text));
            }
            let (tx, mut rx) = mpsc::unbounded_channel::<LlmEvent>();
            if let Ok(mut st) = self.state.lock() {
                if let Some(r) = st.run.as_mut() {
                    r.llm_stream = Some(ls.clone());
                }
                st.llm_tx.insert(ls.clone(), tx);
            }
            // Turno: acumula deltas (eco ao vivo) até o fim do stream do LLM.
            let mut streamed_text = false;
            let mut tool_acc: HashMap<String, (String, String)> = HashMap::new();
            let (result, llm_error) = loop {
                match rx.recv().await {
                    Some(LlmEvent::Chunk(data)) => {
                        if let Some(t) = data.get("delta_text").and_then(|x| x.as_str()) {
                            streamed_text = true;
                            emit(
                                client,
                                &snap.stream,
                                &mut seq,
                                json!({"type": "text", "text": t}),
                            );
                        }
                        if let Some(dt) = data.get("delta_tool_call") {
                            let cid = dt
                                .get("id")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            if !cid.is_empty() {
                                let e = tool_acc
                                    .entry(cid)
                                    .or_insert((String::new(), String::new()));
                                if let Some(n) = dt.get("name").and_then(|x| x.as_str())
                                    && !n.is_empty()
                                {
                                    e.0 = n.to_string();
                                }
                                if let Some(f) = dt.get("input_delta").and_then(|x| x.as_str()) {
                                    e.1.push_str(f);
                                }
                            }
                        }
                    }
                    Some(LlmEvent::End { result, error }) => break (result, error),
                    None => {
                        break (
                            None,
                            Some(RpcError::new(
                                ErrorCode::INTERNAL_ERROR,
                                "llm: stream interrompido",
                            )),
                        );
                    }
                }
            };
            if let Ok(mut st) = self.state.lock() {
                if let Some(r) = st.run.as_mut() {
                    r.llm_stream = None;
                }
                st.llm_tx.remove(&ls);
            }
            let _ = tool_acc;
            if let Some(e) = self.cancelled_err(client) {
                return Err(e);
            }
            if let Some(err) = llm_error {
                let text = format!("llm: {}", err.message);
                emit(
                    client,
                    &snap.stream,
                    &mut seq,
                    json!({"type": "done", "message": text}),
                );
                return Err(self.fail_take(client, text));
            }
            let msg = result
                .as_ref()
                .and_then(|r| r.get("message"))
                .cloned()
                .unwrap_or(Value::Null);
            let parts = msg
                .get("content")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let calls: Vec<(String, String, Value)> = parts
                .iter()
                .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("tool_call"))
                .map(|p| {
                    let t = &p["tool_call"];
                    (
                        t.get("id")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        t.get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        t.get("input").cloned().unwrap_or(Value::Null),
                    )
                })
                .collect();
            // texto final do turno (só se não foi repassado ao vivo)
            if !streamed_text {
                for q in &parts {
                    if q.get("type").and_then(|t| t.as_str()) == Some("text")
                        && let Some(t) = q.get("text").and_then(|x| x.as_str())
                    {
                        emit(
                            client,
                            &snap.stream,
                            &mut seq,
                            json!({"type": "text", "text": t}),
                        );
                    }
                }
            }
            if let Ok(mut st) = self.state.lock()
                && let Some(r) = st.run.as_mut()
            {
                r.history.push(msg);
            }
            if let Some(e) = self.cancelled_err(client) {
                return Err(e);
            }
            if calls.is_empty() {
                let text = text_of_parts(&parts);
                emit(
                    client,
                    &snap.stream,
                    &mut seq,
                    json!({"type": "done", "message": text}),
                );
                return match self.take_run() {
                    Some(run) => Ok(self.finish_ok(client, run, text)),
                    None => Ok(json!({"message": text})),
                };
            }
            let (turns, max_turns) = self
                .state
                .lock()
                .map(|st| {
                    st.run
                        .as_ref()
                        .map(|r| (r.turns, r.max_turns))
                        .unwrap_or((0, 0))
                })
                .unwrap_or((0, 0));
            if turns >= max_turns {
                let text = "limite de turnos".to_string();
                emit(
                    client,
                    &snap.stream,
                    &mut seq,
                    json!({"type": "done", "message": text}),
                );
                return match self.take_run() {
                    Some(run) => Ok(self.finish_ok(client, run, text)),
                    None => Ok(json!({"message": text})),
                };
            }
            // dispara as tools em sequência (era `fire_tool` + encadeamento).
            for (cid, name, input) in calls {
                if let Some(e) = self.cancelled_err(client) {
                    return Err(e);
                }
                let (prov, workdir, stream) = self
                    .state
                    .lock()
                    .map(|st| {
                        st.run.as_ref().map(|r| {
                            (
                                r.tools.get(&name).map(|(p, _)| p.clone()),
                                r.workdir.clone(),
                                r.stream.clone(),
                            )
                        })
                    })
                    .unwrap_or(None)
                    .unwrap_or((None, None, None));
                emit(
                    client,
                    &stream,
                    &mut seq,
                    json!({"type": "tool_call", "name": name, "input": input}),
                );
                let mut call_input = input;
                absolutize(&mut call_input, &workdir);
                let cp = json!({"name": name, "input": call_input});
                let res = client
                    .call("ai.tools@0", "execute", cp, prov.as_deref(), false)
                    .await
                    .unwrap_or(Value::Null);
                let text = text_of_result(&res);
                let is_error = res
                    .get("is_error")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                if let Ok(mut st) = self.state.lock()
                    && let Some(r) = st.run.as_mut()
                {
                    r.history.push(json!({"role": "tool", "content": [
                        {"type": "tool_result", "tool_result": {"call_id": cid, "content": text, "is_error": is_error}}
                    ]}));
                }
                emit(
                    client,
                    &stream,
                    &mut seq,
                    json!({"type": "tool_result", "name": name, "output": text, "is_error": is_error}),
                );
            }
        }
    }
}

impl Handler for AgentLoop {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.agent@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            ..Default::default()
        }
    }

    async fn on_invoke(&self, client: &Client, inv: Invoke) -> Result<Value, RpcError> {
        match (inv.service.as_str(), inv.method.as_str()) {
            ("ai.agent@0", "sessions.list") => {
                let names: Vec<Value> = self
                    .state
                    .lock()
                    .map(|st| {
                        st.sessions
                            .keys()
                            .map(|s| Value::String(s.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Value::Array(names))
            }
            ("ai.agent@0", "sessions.get") => {
                let s = inv
                    .params
                    .get("session")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                let turns = self
                    .state
                    .lock()
                    .map(|st| st.sessions.get(s).cloned().unwrap_or_default())
                    .unwrap_or_default();
                Ok(json!({"session": s, "turns": turns}))
            }
            ("ai.agent@0", "sessions.delete") => {
                let s = inv
                    .params
                    .get("session")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                let had = self
                    .state
                    .lock()
                    .map(|mut st| st.sessions.remove(s).is_some())
                    .unwrap_or(false);
                Ok(json!({"deleted": had}))
            }
            ("ai.agent@0", "run") => {
                let cfg = config_of(client).await;
                let session = inv
                    .params
                    .get("session")
                    .and_then(|x| x.as_str())
                    .unwrap_or("chat")
                    .to_string();
                let input = inv
                    .params
                    .get("input")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let opts = inv.params.get("options").cloned().unwrap_or(Value::Null);
                {
                    let mut st = match self.state.lock() {
                        Ok(g) => g,
                        Err(_) => {
                            return Err(RpcError::new(ErrorCode::INTERNAL_ERROR, "estado travado"));
                        }
                    };
                    if st.run.is_some() {
                        return Err(RpcError::new(
                            ErrorCode::INTERNAL_ERROR,
                            "run já em andamento",
                        ));
                    }
                    let mut history = st.sessions.get(&session).cloned().unwrap_or_default();
                    history.push(
                        json!({"role": "user", "content": [{"type": "text", "text": input}]}),
                    );
                    st.run = Some(Run {
                        session,
                        stream: inv.stream.clone(),
                        history,
                        tools: HashMap::new(),
                        llm_stream: None,
                        turns: 0,
                        max_turns: opts
                            .get("max_turns")
                            .and_then(|x| x.as_u64())
                            .unwrap_or(cfg.max_turns),
                        model: opts
                            .get("model")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                            .or(cfg.model),
                        workdir: opts
                            .get("workdir")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                        cancelled: false,
                        llm_target: cfg.llm_target,
                    });
                }
                self.drive_run(client).await
            }
            _ => Err(RpcError::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("método {}/{} desconhecido", inv.service, inv.method),
            )),
        }
    }

    async fn on_stream_chunk(&self, _client: &Client, chunk: StreamChunk) {
        let tx = self
            .state
            .lock()
            .ok()
            .and_then(|st| st.llm_tx.get(&chunk.stream).cloned());
        if let Some(tx) = tx {
            let _ = tx.send(LlmEvent::Chunk(chunk.data));
        }
    }

    async fn on_stream_end(&self, _client: &Client, end: StreamEnd) {
        let tx = self
            .state
            .lock()
            .ok()
            .and_then(|mut st| st.llm_tx.remove(&end.stream));
        if let Some(tx) = tx {
            let _ = tx.send(LlmEvent::End {
                result: end.result,
                error: end.error,
            });
        }
    }

    async fn on_stream_cancel(&self, client: &Client, stream: &str) {
        let ls = self
            .state
            .lock()
            .ok()
            .and_then(|mut st| match st.run.as_mut() {
                Some(r) if r.stream.as_deref() == Some(stream) => {
                    r.cancelled = true;
                    // Limpa antes de repassar: chunks/end atrasados do LLM
                    // são ignorados (sem virar turno nem fim de run).
                    let ls = r.llm_stream.take();
                    if let Some(ls) = &ls {
                        st.llm_tx.remove(ls);
                    }
                    ls
                }
                _ => None,
            });
        if let Some(ls) = ls {
            client.stream_cancel(&ls);
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(AgentLoop::default()).await;
}
