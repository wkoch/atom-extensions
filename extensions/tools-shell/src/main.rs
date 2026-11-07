//! `atom/tools-shell`: provê `ai.tools@0` com `shell_exec`.
//! Bloqueante com timeout via `wait_timeout` manual (poll).
//! `shortcut: sem sandbox (caps na v0.6); shell real do usuário.`

use atom_sdk::{Client, Declare, Invoke, ProvidesEntry, RpcError};
use serde_json::{Value, json};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn tool_defs() -> Value {
    json!([
        {"name": "shell_exec", "description": "Executa comando via sh -c.",
         "input_schema": {"type": "object", "required": ["command"],
            "properties": {"command": {"type": "string"}, "workdir": {"type": "string"},
                           "timeout_ms": {"type": "integer", "minimum": 100}}}}
    ])
}

fn execute(input: &Value) -> Value {
    let cmd = match input.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            return json!({"output": [{"type": "text", "text": "command ausente"}], "is_error": true});
        }
    };
    let timeout = Duration::from_millis(
        input
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000)
            .clamp(100, 300_000),
    );
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(w) = input.get("workdir").and_then(|v| v.as_str()) {
        c.current_dir(w);
    }
    let mut child = match c.spawn() {
        Ok(c) => c,
        Err(e) => {
            return json!({"output": [{"type": "text", "text": format!("spawn: {e}")}], "is_error": true});
        }
    };
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                return json!({"output": [{"type": "text", "text": format!("wait: {e}")}], "is_error": true});
            }
        }
    };
    let mut out = child
        .wait_with_output()
        .map(|o| {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.is_empty() {
                s.push_str("\n[stderr]\n");
                s.push_str(&err);
            }
            s
        })
        .unwrap_or_default();
    let (code, timed_out) = match status {
        Some(s) => (s.code().unwrap_or(-1), false),
        None => {
            out.push_str("\n[timeout]");
            (-1, true)
        }
    };
    let is_error = timed_out || code != 0;
    json!({"output": [{"type": "text", "text": out}], "is_error": is_error, "exit_code": code})
}

struct H;

impl atom_sdk::Handler for H {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.tools@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            ..Default::default()
        }
    }

    async fn on_invoke(&self, _client: &Client, inv: Invoke) -> Result<Value, RpcError> {
        match (inv.service.as_str(), inv.method.as_str()) {
            ("ai.tools@0", "list") => Ok(tool_defs()),
            ("ai.tools@0", "execute") => {
                let input = inv.params.get("input").cloned().unwrap_or(Value::Null);
                tokio::task::spawn_blocking(move || execute(&input))
                    .await
                    .map_err(|e| {
                        RpcError::new(atom_sdk::ErrorCode::INTERNAL_ERROR, format!("shell: {e}"))
                    })
            }
            _ => Err(RpcError::new(
                atom_sdk::ErrorCode::METHOD_NOT_FOUND,
                "método service.invoke desconhecido".to_string(),
            )),
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(H).await;
}
