# e2e do core portados do `atom` (ainda NÃO compilam aqui)

Vieram de `atom/crates/atom-core/tests/` (removidos do core pelo ADR-017):
`guard_e2e.rs`, `kind_e2e.rs`, `web_e2e.rs`. Testam extensões oficiais
(`guard-shell`, `tools-shell`, `ui-web`, `mcp-bridge`, ...), por isso moram aqui.

Para virar teste de verdade ainda falta:
1. Um crate de teste neste workspace (ex.: `tests/e2e-core/Cargo.toml`) com
   `atom-core` como dev-dependency git no commit do core que tem o bus/store atual
   (`git = "https://github.com/wkoch/atom", rev = "..."`). A tag `v1.0.0` aponta para
   `8211bbe`, anterior ao HEAD atual do core; prefira `rev`.
2. Os `testdata/` que os testes usam (`prompt-fake`, `skill-fake`, `echo`, `fake-llm`,
   `mcp-fake`, `kind-xyz`, `py-echo`, `ui-hello-view`) copiados para cá. O `workspace_root()`
   aponta para a raiz deste repo, então os caminhos `extensions/...` batem.
3. `bin()` faz `cargo build -p <pkg>`: o pacote precisa estar no workspace deste repo.
