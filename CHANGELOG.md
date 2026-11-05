# Changelog

Formato por pacote: `## <pacote> <versão>`. Mudanças de contrato `@0` listam o que quebrou.

## (repo) não lançado
- Docs: alinhamento ao core v1.1 — ADR-E006 (contribuições de UI como dados, implementa ADR-025 do core) e
  ADR-E007 (extensões sobre `atom-sdk` v1.1: cancelamento, deadline, fluxo, sem socket do daemon, `engine` com teto);
  spec nova `specs/ui-contributions.md`; roadmap E0.1/E0.2; `mod-wasm-abi.md` (capability `events.subscribe:<glob>`,
  `timeout_ms`→deadline, `flow.window`, `if_version`, evento `gap`); `contracts.md` (`points`, erros v1.1);
  `ui-layout.md` (`atom.binding.changed`); `releasing.md` e `extension-authoring.md` com `store/v2`.
  Nenhum pacote mudou de versão.

## (repo) 0.0.0
- Extraído do monorepo `atom` (v1.0): contratos, extensões, mods e distributions das versões 0.2–0.9.
