# 2026-11-07 — ui-web calado após migrar p/ SDK: arranque por evento exige `subscribes` no manifesto

## Decided
- Extensão que arranca em `on_event("atom.extension.started")` precisa de
  `subscribes = ["atom.extension.started"]` no `atom.toml`.
- O `Declare.subscribes` do SDK (resposta do `atom.initialize`) NÃO é
  registrado no `registry` pelo core — só manifesto (`refresh`, `bus.rs:416`)
  e `atom.register`/`atom.events.subscribe` registram. (Futuro: registrar os
  do `InitializeResult` no `ensure_started`, ou documentar que `Declare`
  sozinho não assina.)
- Com gate por capability (ADR-026: `atom.ui.serve` exige `ui.serve`), o
  manifesto precisa declarar a capability E o harness de teste precisa
  aprovar (`missing_grants`/`approve_grants`, padrão do `guard_e2e`).

## Tried and ruled out
- Só adicionar `capabilities = ["ui.serve"]` + approve no teste: `ui_url`
  continuou `None` (diag: grant ok, `boot()` ok, sem anúncio). O evento
  `started` nunca era entregue — faltava a assinatura no manifesto.
- Depurar via `RUST_LOG`: stderr do ext vai p/ `tracing::info!`, mas nada
  aparecia porque o processo nunca recebia o evento (não era crash).

## Watch out
- `In the code:` `extensions/ui-web/atom.toml` (`capabilities`,
  `subscribes`); `tests/e2e-core/tests/web_e2e.rs::install_data` (approve);
  `atom/crates/atom-core/src/bus.rs:416` (manifesto→registry),
  `host.rs::ensure_started` (handshake sem registrar subscribes).
- `atom ext install --dev` num workspace com `Cargo.toml` de pacote membro
  falha (`cargo build --manifest-path <membro>` não acha o binário) —
  pré-existente, fora deste escopo; teste linka o binário manual (`link_bin`).
