# Spec: `ui.tree@0` — árvore UI de mods

Status: rascunho (E2). Contrato em `contracts/ui-tree`.

## 1. Formato
JSON. Um nó:
```json
{"t": "atom-button", "k": "send", "p": {"label": "Enviar", "variant": "primary"}, "on": {"press": "send"}, "c": []}
```
| Campo | Obrigatório | Significado |
|---|---|---|
| `t` | sim | tipo = tag do kit (`ui-kit.md` §2). Tag desconhecida → nó substituído por placeholder + aviso. |
| `k` | não | chave estável entre renders (diff de listas, preservação de foco). Única entre irmãos. |
| `p` | não | props, validadas pelo schema do componente. Props desconhecidas são ignoradas com aviso. |
| `on` | não | evento do kit → nome de mensagem do mod (string). Shell envia `{msg, data}` ao `update`. |
| `c` | não | filhos (só componentes de layout/container os aceitam). |

## 2. Limites (defesa contra mod ruim)
- Profundidade ≤ 32, nós ≤ 5000, JSON ≤ 1 MiB por `view`. Excedeu → render anterior mantido + erro no boundary.
- Strings de texto ≤ 64 KiB por nó; para mais, `atom-log` com `append` incremental.

## 3. Atualização
- O shell guarda a última árvore e aplica diff (por `k`, depois por posição). Foco e scroll preservados em nós com mesmo `k`.
- `view` só é chamado após `update` que retorna `dirty: true` (ver ABI). Sem polling.

## 4. Eventos
`data` por evento: `press` → `{}`; `change` → `{value}`; `submit` → `{value}`; `select` → `{id}`; `close` → `{}`; `link` → `{href}` (navegação externa sempre confirmada pelo shell).
