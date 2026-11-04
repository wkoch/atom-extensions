# atom-extensions

Extensões, contratos, mods de UI e distributions **oficiais** do [Atom](https://github.com/wkoch/atom).

O core (`atom`) é um microkernel congelado em `atom/1`; tudo que tem domínio
(IA, UI, temas, layouts) vive aqui e é publicado na loja (`atom-store`).

```
contracts/       pacotes kind="contract" (ai.*, ui.*) — JSON Schemas
extensions/      extensões executáveis em Rust (binário nativo por plataforma)
mods/            pacotes kind="ui-mod" (JS hoje; Rust→WASM a partir do roadmap E2)
distributions/   kind="distribution" (perfis prontos)
crates/          (E2+) bibliotecas compartilhadas: atom-ui (árvore UI p/ mods WASM)
kit/             (E1+) atom/ui-kit: design system (Web Components) + tokens
docs/            arquitetura deste repo, ADRs, specs de UI, release
```

## Desenvolvimento

```sh
cargo check --workspace
cargo build -p atom-tools-fs
atom ext install --dev extensions/tools-fs     # precisa do CLI `atom` instalado
atom ext install --dev mods/ui-menu-top
```

Leia antes de codar: `AGENTS.md`, `docs/architecture.md`, `docs/decisions.md`, `docs/roadmap.md`.

## Publicação

Cada pacote tem versão própria. Tag `<pacote>@<versão>` (ex.: `tools-fs@0.2.1`)
dispara `release.yml`: build por plataforma → `atom ext pack` → PR no `atom-store`.
Detalhes em `docs/releasing.md`.
