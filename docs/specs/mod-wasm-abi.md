# Spec: ABI de mods WASM (`ui-mod` nível árvore)

Status: rascunho (E2). Implementação: shell (`extensions/ui-web`) + `crates/atom-ui`.

## 1. Empacotamento
```toml
manifest = 1
name = "atom/menu-top"
version = "0.9.0"
kind = "ui-mod"

[contributes.renderers]
"ui.menu.items" = { wasm = "mod.wasm", abi = "atom-mod/0" }

[contributes.layout]           # dicas de tamanho (ui-layout.md)
"ui.menu.items" = { min = "28px", preferred = "32px", grow = 0 }
```
- Alvo: `wasm32-unknown-unknown`, `opt-level="z"`, `lto=true`, `panic="abort"`, `wasm-opt -Oz` no release.
- Sem `wasm-bindgen` obrigatório: a ABI é só memória linear + exports abaixo (funciona igual em wasmtime).
- Artefato `platform = "any"`. Tamanho alvo < 150 KiB (CI avisa acima de 300 KiB).

## 2. Exports do módulo
| Export | Assinatura | Semântica |
|---|---|---|
| `atom_alloc` | `(len: i32) -> i32` | aloca buffer para o host escrever entrada |
| `atom_free` | `(ptr: i32, len: i32)` | libera buffer |
| `atom_init` | `(ptr, len) -> i64` | entrada `InitIn`; retorna `UpdateOut` |
| `atom_update` | `(ptr, len) -> i64` | entrada `Event`; retorna `UpdateOut` |
| `atom_view` | `() -> i64` | retorna a árvore `ui.tree@0` |

Retorno `i64` = `(ptr << 32) | len` de um buffer JSON UTF-8 alocado pelo mod; o host lê e chama `atom_free`.

## 3. Imports (módulo `atom`)
| Import | Assinatura | Uso |
|---|---|---|
| `log` | `(level: i32, ptr, len)` | log no console/journal do shell |
| `now_ms` | `() -> f64` | relógio |

Nada mais: **todo efeito é dado de retorno** (abaixo), não chamada direta. Simplifica sandbox e testes.

## 4. Mensagens (JSON)
```jsonc
// InitIn
{"abi": "atom-mod/0", "mod": "atom/menu-top", "point": "ui.menu.items",
 "items": [/* contribuições do ponto, se renderer */], "state": {/* último estado salvo ou null */},
 "config": {/* config do mod */}, "locale": "pt-BR"}

// Event (vindo do shell)
{"type": "ui", "msg": "send", "data": {"value": "oi"}}
{"type": "items", "items": [...]}                       // contribuições mudaram (mod instalado/removido)
{"type": "reply", "id": 7, "ok": {...}}                 // resposta de efeito
{"type": "reply", "id": 7, "err": {"code": -32003, "message": "..."}}
{"type": "stream", "id": 8, "chunk": {...}} | {"type": "stream", "id": 8, "end": true}
{"type": "event", "topic": "ai.agent.done", "data": {...}}
{"type": "config", "config": {...}}

// UpdateOut (devolvido pelo mod)
{"dirty": true, "state": {/* opcional: persistir */}, "effects": [
  {"id": 7, "call": {"service": "ai.agent@0", "method": "run", "params": {...}, "stream": true}},
  {"id": 9, "cancel": 8},
  {"subscribe": "ai.agent.*"},
  {"navigate": {"view": "chat"}},
  {"notify": {"level": "info", "text": "salvo"}}
]}
```

## 5. Regras do host
- Um Worker por instância de mod; travou > 2s em uma chamada → terminado, boundary mostra erro, botão "recarregar".
- `call`/`subscribe` passam pelos **grants do mod** (`service:<contrato>`, `event:<tópico>`), concedidos como em
  `capabilities.md` do core. Sem grant → `reply` com `-32003`.
- `state` é salvo via storage do bus chaveado por `(mod, contribuição)` (ADR-016 §7); limite 256 KiB.
- Navegação e `notify` são pedidos; o shell decide.
- Panic do mod (`unreachable`) = mesmo tratamento de travamento.

## 6. `atom-ui` (crate)
```rust
use atom_ui::prelude::*;

#[derive(Default, Serialize, Deserialize)]
struct Menu { items: Vec<Item> }

impl Mod for Menu {
    fn update(&mut self, ev: Event, fx: &mut Effects) -> bool { /* ... */ true }
    fn view(&self) -> Node {
        stack().direction(Row).gap(1).children(
            self.items.iter().map(|it| button(&it.label).key(&it.id).on_press(msg("go", &it.target)))
        )
    }
}
export_mod!(Menu);
```
`export_mod!` gera exports, alocador e (de)serialização. Testes de mod rodam nativos (sem WASM) chamando `update`/`view`.
