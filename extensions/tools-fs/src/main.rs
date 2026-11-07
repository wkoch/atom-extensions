//! `atom/tools-fs`: provê `ai.tools@0` com `fs_read`, `fs_write`, `fs_list`.
//! Sem estado; operações síncronas (rápidas).
//! `shortcut: sem sandbox de paths (caps entram na v0.6); confie no chamador local.`

use atom_sdk::{Client, Declare, Handler, Invoke, ProvidesEntry, RpcError};
use serde_json::{Value, json};

struct ToolsFs;

fn tool_defs() -> Value {
    json!([
        {"name": "fs_read", "description": "Lê arquivo (texto).",
         "input_schema": {"type": "object", "required": ["path"],
            "properties": {"path": {"type": "string"}, "offset": {"type": "integer"}, "limit": {"type": "integer"}}}},
        {"name": "fs_write", "description": "Escreve arquivo (cria diretórios).",
         "input_schema": {"type": "object", "required": ["path", "content"],
            "properties": {"path": {"type": "string"}, "content": {"type": "string"}}}},
        {"name": "fs_list", "description": "Lista diretório.",
         "input_schema": {"type": "object", "required": ["path"],
            "properties": {"path": {"type": "string"}}}}
    ])
}

fn text_part(s: String) -> Value {
    json!({"type": "text", "text": s})
}

fn execute(name: &str, input: &Value) -> Value {
    let out = match name {
        "fs_read" => (|| {
            let p = input.get("path").and_then(|v| v.as_str()).ok_or("path")?;
            let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            let lines: Vec<&str> = text.lines().collect();
            let off = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let lim = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;
            let end = (off + lim).min(lines.len());
            if off > lines.len() {
                return Err(format!("offset {off} além de {} linhas", lines.len()));
            }
            Ok(lines[off..end].join("\n"))
        })(),
        "fs_write" => (|| {
            let p = input.get("path").and_then(|v| v.as_str()).ok_or("path")?;
            let c = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("content")?;
            let path = std::path::Path::new(p);
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, c).map_err(|e| e.to_string())?;
            Ok(format!("escrito {} ({} bytes)", p, c.len()))
        })(),
        "fs_list" => (|| {
            let p = input.get("path").and_then(|v| v.as_str()).ok_or("path")?;
            let mut names: Vec<String> = vec![];
            for e in std::fs::read_dir(p).map_err(|e| e.to_string())? {
                let e = e.map_err(|e| e.to_string())?;
                let ft = e.file_type().map_err(|e| e.to_string())?;
                names.push(format!(
                    "{}{}",
                    e.file_name().to_string_lossy(),
                    if ft.is_dir() { "/" } else { "" }
                ));
            }
            names.sort();
            Ok(names.join("\n"))
        })(),
        _ => Err(format!("tool desconhecida: {name}")),
    };
    match out {
        Ok(t) => json!({"output": [text_part(t)], "is_error": false}),
        Err(e) => json!({"output": [text_part(e)], "is_error": true}),
    }
}

impl Handler for ToolsFs {
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
                let n = inv
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let input = inv.params.get("input").cloned().unwrap_or(Value::Null);
                Ok(execute(n, &input))
            }
            (s, m) => Err(RpcError::new(
                atom_sdk::ErrorCode::METHOD_NOT_FOUND,
                format!("método {s}/{m} desconhecido"),
            )),
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(ToolsFs).await;
}
