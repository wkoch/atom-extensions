# AGENTS.md — atom-extensions

Leia antes de qualquer código: `docs/architecture.md`, `docs/decisions.md`, `docs/roadmap.md`
e as specs em `docs/specs/`. As specs do kernel (`protocol.md`, `manifest.md`, `capabilities.md`,
`profiles.md`, `store.md`, `store-v2.md`, `conformance.md`) ficam no repo `atom`: `atom/1` congelou na v1.0 e
a v1.1 só adicionou recursos opt-in. Este repo se adapta a elas, nunca o contrário.

## Regras de ouro

1. **Nada aqui exige mudar o core.** Se parece exigir, é contrato + extensão. Exceção = ADR no repo `atom`.
2. **Só API pública.** Extensões Rust usam `atom-sdk` por versão/tag (nunca `path` para o repo do core).
   Mods usam só a API do shell e os contratos `ui.*`.
3. **Mods não se conhecem** (ADR-016 do core): só pontos de contribuição e contratos de serviço.
4. **Shell não tem estilo de mod.** Estilo vem de `ui-kit` + tokens de tema (ADR-E003). Nenhum mod
   escreve CSS global.
5. **Contratos `@0` podem quebrar** (ADR-014 do core), mas toda quebra entra no CHANGELOG do contrato
   e dos pacotes afetados no mesmo PR.
6. **Core v1.1 (ADR-E007):** protocolo só via `atom-sdk` (nada de JSON-RPC à mão); providers tratam
   cancelamento (`$/cancelRequest`, `atom.stream.cancel`) e respeitam `$ctx.deadline`; nunca conectar no socket
   do daemon (`-32005`) nem editar `profile.toml`/`grants.toml` (ignorado pelo core); `engine` com teto `<2.0.0`.
7. **Contribuições de UI são dados** (ADR-E006, `specs/ui-contributions.md`): componente só em `surfaces.<s>`.

## Stack

- Extensões: Rust stable, edition 2024, `#![forbid(unsafe_code)]`, sem dependências C (cross-compile musl).
  Dependências permitidas: as mesmas do `AGENTS.md` do core; outras, justificar no PR.
- Mods: Rust → `wasm32-unknown-unknown`, rodando em Web Worker (spec `mod-wasm-abi.md`).
  JS puro só para shell, `ui-kit` e mods legados.
- Erros: `thiserror` em libs, `anyhow` só em binários. Sem `unwrap()`/`expect()` fora de testes.

## Estrutura e versões

- Um diretório = um pacote = uma versão (`atom.toml` + `Cargo.toml` em sincronia; CI verifica).
- Tag de release: `<dir>@<versão>` (ex.: `ui-menu-top@0.8.1`).
- Artefatos: extensões nativas por plataforma; contratos, mods (JS/WASM) e distributions `platform = "any"`.

## Qualidade

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
validação de manifestos/schemas e (a partir de E1) testes visuais da matriz layout × renderer × tema.
CI em Linux, macOS e Windows.

## Fluxo

Implementar pelo `docs/roadmap.md`, em ordem. Ao concluir um item: marcar no roadmap e no CHANGELOG.

<!-- trail:start -->
## Trail — repo context graph

This repo is indexed in `.trail/cache/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `trail map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `trail ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `trail grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `trail skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `trail callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `.trail/cache/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `trail ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `trail build` (deterministic,
no API key, $0).
<!-- trail:end -->
