# Roadmap do atom-extensions

Ordem obrigatória. Item só fecha com CI verde (Linux/macOS/Windows). Legenda `[ ]`/`[x]`.

## E0 — Extração e base
- [x] Repo criado no GitHub; CI (`ci.yml`) verde com o código extraído.
- [x] `atom-sdk` referenciado em `[workspace.dependencies]`.
      Fixado em `rev = aea211f` (pré-v1.1, anterior a deadline/flow/credit/gap — ver `Cargo.toml`);
      trocar pela tag v1.1 do core (`atom-sdk`, `atom-core`, `atom-protocol` juntos) assim que publicada
      (hoje só existe a tag `v1.0.0`; o `main` remoto é o merge da extração, sem o v1.1 parcial local).
- [x] Migradas p/ `atom-sdk` (protocolo manual removido): `tools-fs`, `tools-shell`, `guard-shell`, `skill-loader`,
      `mcp-bridge`, `llm-openai-compat`, `llm-anthropic`, `agent-loop-basic`, `ui-web`
      (todas verdes em `cargo check --workspace`; e2e `guard`/`kind`/`web` verdes no rev fixado).
      Exceção permanente (fio manual; SDK fixado não cobre): `ui-cli` (`command.run` com `params.io`,
      fora do modelo request/response — usa tipos do SDK no anúncio/erros). O `ui-web` usa `subscribes`
      a `atom.extension.started` como gancho de arranque (o SDK não despacha `atom.initialized`).
- [x] Testes e2e em `tests/` na raiz (`guard`, `kind`, `web`), usando o binário `atom` de release
      (`ATOM_BIN`; `web_e2e` sobe o daemon real). `guard`/`kind` rodam no `cargo test`; `web` com `ATOM_BIN`.
- [x] `release.yml`: tag `<pacote>@<versão>` → artefatos 5 plataformas → `atom ext pack` → PR no `atom-store`
      (`v1/`; `v2/` em E0.1, quando o core tiver a ferramenta pub v2).

## E0.1 — Alinhamento com o core v1.1 (ADR-E007)
Pré-requisitos no core (`atom/docs/roadmap.md`, v1.1): SDKs tratando `$/cancelRequest` no provider; tag v1.1;
integração `store/v2` na CLI + ferramenta de publicação v2; decisão sobre `atom.ui.session`.

- [ ] Todas as extensões Rust sobre `atom-sdk` v1.1 (consequência de E0): cancelamento (`$/cancelRequest` e
      `atom.stream.cancel`) e `$ctx.deadline` propagado nas sub-chamadas (`agent-loop-basic` → `ai.llm@0`/`ai.tools@0`,
      `guard-shell` → `ui.prompt@0`). Hoje só `atom.stream.cancel` é tratado, à mão.
      BLOQUEADO: SDK fixado (`rev aea211f`) não tem `$/cancelRequest` no provider nem `$ctx.deadline`
      (`CallerCtx` sem `deadline`; sem `call_in_ctx`/`call_with_deadline`); depende da tag v1.1 do core.
      Exceção permanente (fio manual): `ui-cli` (SDK não despacha `command.run` com `params.io`) —
      reavaliar se o SDK v1.1 ganhar esse gancho. (`ui-web` migrado via `atom.extension.started`.)
- [ ] `llm-openai-compat`, `llm-anthropic`, `agent-loop-basic`: `flow = "credit"` no `Declare` e respeito a
      `atom.stream.credit` (`[P-STREAM-3]`); erros `-32050`/`-32022` repassados sem virar `-32603`.
      BLOQUEADO: SDK fixado sem `Declare.flow`, `on_stream_credit`, nem `-32050`/`-32022`; depende da tag v1.1.
- [ ] `ui-web`: consumidor WS usa `"flow": {"window": N}` + `atom.stream.ack` nas streams do chat; reconexão de eventos
      com `from` e tratamento de `atom.events.gap`; estado de mod salvo com `if_version` (`-32041` = recarregar e reaplicar).
      BLOQUEADO: SDK fixado sem `call_with_window`, `stream_ack`, `subscribe_from`, `on_events_gap`, `on_storage_changed`;
      depende da tag v1.1.
- [ ] `ui-web`: deixar de depender de `atom.ui.session` sem capability (pedir a capability que o core definir;
      `protocol.md §9.0`) e documentar que o shell é o ponto de aplicação de grants dos mods (a conexão WS do shell
      tem autoridade de cliente confiável).
      BLOQUEADO: decisão pendente no core (roadmap v1.1: gate de capability p/ `atom.ui.session`).
- [x] `engine` de todos os `atom.toml` com teto (`">=0.1.0, <2.0.0"`; subir o piso p/ `">=1.1.0"` onde usar v1.1
      quando a tag existir — hoje o core fixado é `0.1.0`, então `>=1.0.0` quebraria o e2e).
- [x] README de cada pacote lista capabilities *advisory* com o aviso `[C-UI-1]`.
- [ ] CI: `atom-conformance ext -- <binário>` (core v1.1) contra cada extensão oficial executável.
      PARCIAL: job `conformance` existe mas é placeholder (core fixado sem `atom-conformance`); vira gate na tag v1.1.
- [ ] `tests/e2e-core`: atualizar rev e reexecutar contra o binário v1.1 (`guard`, `kind`, `web`); incluir um caso de
      cancelamento e um de `deadline` (`-32022`) passando pelo `agent-loop-basic`.
      PARCIAL: `guard`/`kind`/`web` verdes no rev fixado; casos novos e reexecução v1.1 dependem da tag v1.1.
- [ ] `release.yml`/`releasing.md`: publicar também no `v2/` do `atom-store` — publisher assina o delegado
      `publisher:atom` com `ATOM_PUBLISHER_KEY` e manda no PR (`atom-store/docs/store-v2.md §8`).
      BLOQUEADO: core sem ferramenta pub v2 (roadmap v1.1, item pendente); `release.yml` faz `v1/` + documenta o `v2/`.

Aceite: Ctrl+C no `atom run chat` chega ao `llm-*` como cancelamento em < 2s (sem cair na cortesia do core);
perfil limpo instala `atom/code-starter` de uma source `v2/`.

## E0.2 — Contribuições como dados (ADR-025 do core, ADR-E006)
- [ ] Contratos `ui.menu/views/settings/status@0` com `points.*.item` = descritor de `specs/ui-contributions.md`
      (+ `surfaces`, `action`, `when`); `renderer` estruturado (sem texto "função JS").
- [ ] Shell: carrega `surfaces.web` e `[contributes.renderers."<ponto>".web]`; aceita a forma antiga com aviso.
- [ ] Mods oficiais migrados (`target` → `action.view`, `module` → `surfaces.web.module`, sem `html` cru).
- [ ] `ui-cli`: renderer genérico `tui` de `ui.menu.items`/`ui.views`/`ui.settings.sections` (sem componente).

Aceite: o mod `chat-ui` aparece no menu do `ui-web` e no `ui-cli` sem nenhum código específico de TUI.

## E1 — Tokens, kit e shell limpo
- [ ] Contrato `ui.theme@0` (`contracts/ui-theme`) com schema de tokens (spec `ui-theme.md`).
- [ ] `kit/` = `atom/ui-kit`: Web Components da spec `ui-kit.md` + tema padrão claro/escuro.
- [ ] Remover de `extensions/ui-web/assets/style.css` tudo que não for reset; mods JS existentes passam a usar o kit.
- [ ] Ponto de montagem isolado (`specs/ui-layout.md` §2) no shell; error boundary + skeleton por dicas de tamanho.
- [ ] Testes visuais (Playwright) da matriz {layout-basic} × {menu-top, menu-side} × {settings-popup, settings-panel} × {claro, escuro}.

Aceite: trocar qualquer renderer ou tema não muda a posição/tamanho de nenhuma outra área (diff de screenshot).

## E2 — Mods WASM
- [ ] Contrato `ui.tree@0` (vocabulário = componentes do kit) — spec `ui-tree.md`. `InitIn.items` = descritores (E0.2).
- [ ] Shell: host de Worker + ABI `mod-wasm-abi.md` + diff/patch + mediação de efeitos com grants.
- [ ] `crates/atom-ui`: tipos da árvore, builder, `export_mod!` macro, testes de serialização.
- [ ] Migrar `mods/ui-menu-top` para Rust/WASM como prova; mesma aparência que a versão JS.
- [ ] `atom ext pack` de mod WASM (`platform = "any"`) no `release.yml`.

Aceite: `ui-menu-top` WASM e `ui-menu-side` JS trocáveis no perfil sem alterar contribuições.

## E3 — Migração dos mods oficiais
- [ ] chat, layout, settings (popup/painel), status em WASM.
- [ ] Mod JS vira "legado" documentado (aceito, mas não oficial).

## E4 — Superfície livre
- [ ] Capability `ui.surface`; Shadow DOM (`atom/*`) e `<iframe sandbox>` (terceiros); primeiro uso real (editor).

## Depois
Template `cargo-generate` para terceiros; SSR/execução de mods em wasmtime no servidor; extensões backend em WASM
(quando o runtime WASM do core existir).
