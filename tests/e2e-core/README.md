# e2e do core contra extensões oficiais

Portados de `atom/crates/atom-core/tests/` (ADR-017 do core). São 11 testes:
`guard_e2e` (3), `kind_e2e` (6), `web_e2e` (2).

Rodar:

```sh
cargo test -p atom-e2e-core                       # guard + kind
ATOM_BIN=/caminho/para/atom cargo test -p atom-e2e-core --test web_e2e
```

- `web_e2e` sobe o daemon do core. Gere o binário no mesmo commit das deps:
  `cargo install --git https://github.com/wkoch/atom --rev <rev> --locked atom --root /tmp/atom-core-bin`
- Deps do core (`atom-core`, `atom-sdk`, `atom-protocol`) estão fixadas em `rev` no
  `Cargo.toml` da raiz. Atualize junto com o core. A tag `v1.0.0` é anterior ao core atual.
- Repo `atom` é privado: `cargo` precisa de acesso (`CARGO_NET_GIT_FETCH_WITH_CLI=true` com
  credencial do `gh`). Em CI isso exige token ou repo público.
- `testdata/` (echo, fake-llm, mcp-fake, prompt-fake, skill-fake, kind-xyz, py-echo,
  ui-hello-view) veio do core; os crates `echo`, `fake-llm`, `mcp-fake`, `prompt-fake`
  são membros do workspace.
