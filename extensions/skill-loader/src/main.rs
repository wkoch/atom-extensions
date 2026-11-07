//! `atom/skill-loader`: handler `kind=skill`, provider `ai.context@0`.
//! `kind.load {package, dir, config}` lê `[contributes.skill] file`.
//! `collect` agrega todas as skills como partes `system`.
//! `shortcut: scripts/ com runtime ignorados na v0.4 (só descrição).`

use atom_sdk::{Client, Declare, Invoke, KindPackage, ProvidesEntry, RpcError};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

struct H {
    skills: Mutex<HashMap<String, String>>,
}

impl atom_sdk::Handler for H {
    fn declare(&self) -> Declare {
        Declare {
            provides: vec![ProvidesEntry {
                contract: "ai.context@0".to_string(),
                version: "0.1.0".to_string(),
            }],
            handles: vec!["skill".to_string()],
            intercepts: vec![],
            subscribes: vec![],
            flow: None,
        }
    }

    async fn on_kind_load(&self, _client: &Client, pkg: KindPackage) -> Result<(), RpcError> {
        let manifest = &pkg.manifest;
        let dir = &pkg.dir;
        let name = manifest
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string();
        let file = manifest
            .get("contributes")
            .and_then(|c| c.get("skill"))
            .and_then(|s| s.get("file"))
            .and_then(|f| f.as_str())
            .unwrap_or("SKILL.md");
        if manifest
            .get("contributes")
            .and_then(|c| c.get("skill"))
            .and_then(|s| s.get("scripts"))
            .is_some()
        {
            eprintln!("skill {name}: scripts ignorados na v0.4 (só descrição)");
        }
        match std::fs::read_to_string(format!("{dir}/{file}")) {
            Ok(text) => {
                self.skills.lock().unwrap().insert(name, text);
                Ok(())
            }
            Err(e) => Err(RpcError::new(
                atom_sdk::ErrorCode::PACKAGE_ERROR,
                format!("skill: {e}"),
            )),
        }
    }

    async fn on_kind_unload(&self, _client: &Client, package: &str) {
        let name = package.split('@').next().unwrap_or(package);
        self.skills.lock().unwrap().remove(name);
    }

    async fn on_invoke(&self, _client: &Client, inv: Invoke) -> Result<Value, RpcError> {
        if inv.method != "collect" {
            return Err(RpcError::new(
                atom_sdk::ErrorCode::METHOD_NOT_FOUND,
                format!("método {} desconhecido", inv.method),
            ));
        }
        let skills = self.skills.lock().unwrap();
        let mut names: Vec<&String> = skills.keys().collect();
        names.sort();
        let system: Vec<Value> = names
            .iter()
            .map(|n| {
                let body = skills.get(*n).cloned().unwrap_or_default();
                json!({"type": "text", "text": format!("# skill {n}\n{body}")})
            })
            .collect();
        Ok(json!({"system": system, "messages": []}))
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(H {
        skills: Mutex::new(HashMap::new()),
    })
    .await;
}
