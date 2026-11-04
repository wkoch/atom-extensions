# AGENTS.md — atom-extensions

Leia antes de qualquer código: `docs/architecture.md`, `docs/decisions.md`, `docs/roadmap.md`
e as specs em `docs/specs/`. As specs do kernel (`protocol.md`, `manifest.md`, `capabilities.md`,
`store.md`) ficam no repo `atom` e são **congeladas** (v1.0): este repo se adapta a elas, nunca o contrário.

## Regras de ouro

1. **Nada aqui exige mudar o core.** Se parece exigir, é contrato + extensão. Exceção = ADR no repo `atom`.
2. **Só API pública.** Extensões Rust usam `atom-sdk` por versão/tag (nunca `path` para o repo do core).
   Mods usam só a API do shell e os contratos `ui.*`.
3. **Mods não se conhecem** (ADR-016 do core): só pontos de contribuição e contratos de serviço.
4. **Shell não tem estilo de mod.** Estilo vem de `ui-kit` + tokens de tema (ADR-E003). Nenhum mod
   escreve CSS global.
5. **Contratos `@0` podem quebrar** (ADR-014 do core), mas toda quebra entra no CHANGELOG do contrato
   e dos pacotes afetados no mesmo PR.

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
