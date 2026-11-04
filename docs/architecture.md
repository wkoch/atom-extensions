# Arquitetura do atom-extensions

## 1. Os três repositórios

| Repo | Conteúdo | Ritmo | Segredos |
|---|---|---|---|
| `atom` | core, `atom-protocol`, `atom-sdk`, `sdks/atom-ts`, `testdata/`, conformance, specs do kernel | lento (congelado em `atom/1`) | chave de release do binário |
| `atom-extensions` (este) | contratos, extensões, mods, ui-kit, distributions | rápido, versão por pacote | chave de **publisher** `atom` |
| `atom-store` | índices `store/v1`, `publishers/` | a cada publicação | chave da **loja** (índice) |

Dependências só em uma direção: `atom-extensions` → `atom` (SDK por versão publicada);
`atom-store` não depende de nenhum (só dados + CLI `atom` no CI).

## 2. Tipos de pacote

| Diretório | `kind` | Artefato | Plataforma |
|---|---|---|---|
| `contracts/*` | `contract` | `atom.toml` + `schemas/` | `any` |
| `extensions/*` | `extension` (`runtime` p/ runtime-python) | binário nativo | uma por alvo (5) |
| `mods/*` | `ui-mod` | `assets/` (JS legado) ou `mod.wasm` + manifesto | `any` |
| `kit/` | `ui-mod` (renderer de `ui.kit` / provedor de tokens) | JS + CSS | `any` |
| `distributions/*` | `distribution` | `atom.toml` | `any` |

## 3. UI web em camadas

```
 core (atom/1, sem UI)
   └─ extensions/ui-web      shell: HTTP + ponte WS + loader de ui-mod + host de Workers WASM
        ├─ kit/              atom/ui-kit: Web Components + tokens (único dono de CSS)
        ├─ contracts/ui-*    pontos de contribuição semânticos (ui.menu, ui.views, ...)
        │                    + ui.tree@0 (vocabulário da árvore) + ui.theme@0 (tokens)
        └─ mods/*            conteúdo (contribui) e layout/container (renderiza)
```

Fluxo de um mod WASM (spec `specs/mod-wasm-abi.md`):

```
mod.wasm (Worker, sem DOM)               shell (main thread)
  view(state)   -> árvore (ui.tree@0) ─►  valida + diff/patch com componentes do kit
  update(event) -> efeitos            ◄─  eventos do usuário
  efeito bus.call / storage           ─►  ponte WS, sujeita a grants do mod
```

Níveis de mod (do preferido ao último recurso):
1. **Declarativo** — só `contributes` no `atom.toml` (itens de menu, campos de settings).
2. **Árvore** — WASM em Worker devolvendo `ui.tree@0`. Padrão para mods oficiais.
3. **Superfície** — elemento com Shadow DOM (oficiais) ou `<iframe sandbox>` (terceiros), para
   editor/canvas. Exige capability `ui.surface`.

## 4. Consistência e estabilidade de layout

Ver `specs/ui-layout.md`, `specs/ui-theme.md`, `specs/ui-kit.md`. Resumo:
- Geometria é do **container**; conteúdo declara só dicas (`min`, `preferred`, `grow`).
- Todo ponto de montagem tem `contain: layout paint` e `min-width: 0`: um mod não empurra outro.
- Tema muda tokens (cor, tipografia, raio), nunca espaçamento estrutural.
- Espaço reservado antes de carregar (skeleton), error boundary por montagem, View Transitions na troca.
- CI faz screenshot da matriz layout × renderers × tema.

## 5. Débitos herdados do monorepo

- Extensões reimplementam o protocolo (só `serde_json`) em vez de usar `atom-sdk`. → roadmap E0.
- `extensions/ui-web/assets/style.css` contém estilos de mods (`.menu`, `.chat-log`, `.settings-*`),
  violando ADR-016. → roadmap E1.
- Contratos `ui.*` descrevem renderer como "função JS no módulo"; ganham versão para WASM em E2.
