# Spec: contribuições de UI como dados (ADR-025 do core, ADR-E006)

Status: rascunho (E0.2). Contratos `ui.*@0` (podem quebrar, ADR-014). Implementação: contratos em
`contracts/ui-*`, shell `extensions/ui-web`, mods em `mods/`.

## 1. Regra

Toda contribuição em um ponto `ui.*` é um **descritor serializável** (JSON/TOML), validado pelo schema do
ponto no contrato. Componente (JS ou WASM) é **opcional** e **por superfície**. Um renderer de uma superfície
que não recebe componente específico renderiza o descritor de forma genérica. Assim web, TUI (`ui-cli`) e
frontends futuros (Tauri, mobile) usam as mesmas contribuições.

## 2. Descritor base

Campos comuns a todo ponto (cada contrato pode acrescentar campos próprios, ex. `fields` em settings):

| Campo | Obrigatório | Significado |
|---|---|---|
| `id` | sim | único dentro do mod; chave do estado (`(mod, id)`, ADR-016 §7) |
| `title` / `label` | sim (um dos dois, conforme o ponto) | texto visível |
| `icon` | não | nome do set de ícones do kit (`ui-kit.md`) |
| `order` | não | número; menor primeiro; empate = ordem de nome do mod |
| `when` | não | expressão simples sobre contexto (`config.<chave>`, `surface == "web"`); falsa = item oculto |
| `action` | não | o que acontece ao ativar (§3) |
| `points` | não | pontos filhos declarados por esta contribuição (aninhamento, ADR-016 §6) |
| `surfaces` | não | componentes por superfície (§4) |

## 3. Ações

Exatamente uma chave:

```jsonc
{"call": {"service": "ai.agent@0", "method": "run", "params": {...}}}   // chamada no bus, sujeita a grants do mod
{"command": "chat"}                                                   // `atom run <comando>` (superfície tui)
{"view": "chat"}                                                      // abre a view `id` de `ui.views`
```

## 4. Superfícies

```toml
[[contributes."ui.views"]]
id = "chat"
title = "Chat"
order = 10
surfaces.web = { module = "assets/chat-view.js" }     # ou { wasm = "mod.wasm", abi = "atom-mod/0" }
surfaces.tui = { command = "chat" }
```
- Superfícies conhecidas: `web`, `tui`. Nome desconhecido é ignorado por quem não o conhece.
- Sem `surfaces.<s>`, o renderer da superfície `<s>` usa o descritor (título, ícone, ação, campos).
- Renderers também declaram superfície:
  ```toml
  [contributes.renderers."ui.menu.items".web]
  module = "assets/menu-top.js"           # ou wasm = "mod.wasm", abi = "atom-mod/0"
  ```

## 5. Schema do ponto no contrato

Formato de `points` no JSON do contrato (complementa `contracts.md §1`):

```json
"points": {
  "ui.menu.items": {
    "cardinality": "multi",
    "item": { "$ref": "#/$defs/MenuItem" },
    "renderer": { "cardinality": "single", "binding": "[bindings] \"ui.menu.items\" = \"<pkg>\"" }
  }
}
```
- `cardinality` do ponto: `multi` (muitas contribuições) ou `single` (ex.: `ui.root`, `ui.theme.tokens`).
- `item`: JSON Schema 2020-12 do descritor, incluindo os campos da §2. Descritor inválido = item ignorado
  com aviso (não derruba o ponto).
- `renderer.cardinality` é sempre `single` por superfície; o renderer ativo é escolhido no perfil por
  `atom bind <ponto> <pkg>` (cliente confiável; na v1.1 editar `profile.toml [bindings]` à mão é ignorado
  até `atom profile reload`, `[C-AMB-3]` do core).
- O texto livre atual `"renderer": {"contract": "função renderX(el, …) no módulo"}` sai do schema e vira
  documentação da superfície `web` (§6).

## 6. API do componente web (superfície `web`)

Enquanto existir mod JS (legado, E3): o módulo exporta `render(el, ctx)` (renderer) ou `renderView(el, ctx)`
(view). `ctx = {descriptor | items, state, setState, call, subscribe, slot(point)}`. O componente recebe o
descritor já validado; não lê o manifesto. Mods WASM usam `mod-wasm-abi.md` (o `InitIn.items` são descritores).

## 7. Migração do `@0` atual

| Hoje | Passa a ser |
|---|---|
| `ui.views` item `module` / `html` | `surfaces.web.module` / descritor genérico (sem `html` cru) |
| `[contributes.renderers] "<ponto>" = { module }` | `[contributes.renderers."<ponto>".web] module` |
| `ui.menu.items` `target` | `action = { view = "<id>" }` |
| `ui.status.items` `text` | `title` |
| `ui.settings.sections.fields` | mantém (campos do contrato), cada campo vira descritor com `id = key` |

Shell aceita a forma antiga com aviso até E3; contratos sobem a `version` (minor) ao aceitar os dois formatos.
