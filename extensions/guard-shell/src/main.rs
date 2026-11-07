//! `atom/guard-shell`: interceptor `before ai.tools@0/execute`.
//! Pede confirmação via `ui.prompt@0/confirm` antes de rodar shell.
//! Sem prompt disponível = rejeita (fail-closed).

use atom_protocol::InterceptDecl;
use atom_sdk::{Client, Declare, Handler, Intercept, InterceptAction};
use serde_json::Value;
struct GuardShell;

/// Extrai comando shell de `params` de `ai.tools@0/execute`.
fn shell_command(p: &Value) -> Option<String> {
    let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
    if !name.contains("shell") && !name.contains("exec") {
        return None;
    }
    let input = p.get("input").unwrap_or(&Value::Null);
    input
        .get("command")
        .and_then(|c| c.as_str())
        .or_else(|| {
            input
                .get("input")
                .and_then(|i| i.get("command"))
                .and_then(|c| c.as_str())
        })
        .map(|s| s.to_string())
}

impl Handler for GuardShell {
    fn declare(&self) -> Declare {
        Declare {
            intercepts: vec![InterceptDecl {
                match_: "ai.tools@0/execute".to_string(),
                phase: "before".to_string(),
                priority: 50,
            }],
            ..Default::default()
        }
    }

    async fn on_intercept(&self, client: &Client, it: Intercept) -> InterceptAction {
        if it.phase != "before" || it.service != "ai.tools@0" || it.method != "execute" {
            return InterceptAction::Continue;
        }
        let cmd = match shell_command(&it.params) {
            Some(c) => c,
            None => return InterceptAction::Continue,
        };
        let ok = match client
            .call(
                "ui.prompt@0",
                "confirm",
                serde_json::json!({"title": "Executar shell?", "detail": cmd}),
                None,
                false,
            )
            .await
        {
            Ok(v) => v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false),
            Err(_) => false,
        };
        if ok {
            InterceptAction::Continue
        } else {
            InterceptAction::Reject {
                code: -32020,
                message: "shell bloqueado pelo guard-shell".to_string(),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    atom_sdk::run(GuardShell).await;
}
