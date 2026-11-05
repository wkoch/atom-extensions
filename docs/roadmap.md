# Roadmap do atom-extensions

Ordem obrigatória. Item só fecha com CI verde (Linux/macOS/Windows). Legenda `[ ]`/`[x]`.

## E0 — Extração e base
- [ ] Repo criado no GitHub; CI (`ci.yml`) verde com o código extraído.
- [ ] `atom-sdk` publicado pelo repo `atom` (crates.io ou tag git) e referenciado em `[workspace.dependencies]`.
      Hoje está fixado em `rev = aea211f` (anterior ao v1.1); trocar pela tag v1.1 do core (`atom-sdk`, `atom-core`, `atom-protocol` juntos).
- [ ] Migrar cada extensão Rust para `atom-sdk` (remover protocolo feito à mão). Começar por `tools-fs`.
- [ ] Testes e2e que hoje vivem em `atom/crates/atom-core/tests/{guard,kind,...}_e2e.rs` e usam
      `extensions/*` movidos para cá (`tests/` na raiz, usando o binário `atom` de release).
- [ ] `release.yml` funcionando: tag `tools-fs@x.y.z` → artefatos 5 plataformas → PR no `atom-store` (`v1/`; `v2/` em E0.1).

Aceite: perfil limpo instala `atom/code-starter` da loja publicada por este repo.

## E0.1 — Alinhamento com o core v1.1 (ADR-E007)
Pré-requisitos no core (`atom/docs/roadmap.md`, v1.1): SDKs tratando `$/cancelRequest` no provider; tag v1.1;
integração `store/v2` na CLI + ferramenta de publicação v2; decisão sobre `atom.ui.session`.

- [ ] Todas as extensões Rust sobre `atom-sdk` v1.1 (consequência de E0): cancelamento (`$/cancelRequest` e
      `atom.stream.cancel`) e `$ctx.deadline` propagado nas sub-chamadas (`agent-loop-basic` → `ai.llm@0`/`ai.tools@0`,
      `guard-shell` → `ui.prompt@0`). Hoje só `atom.stream.cancel` é tratado, à mão.
- [ ] `llm-openai-compat`, `llm-anthropic`, `agent-loop-basic`: `flow = "credit"` no `Declare` e respeito a
      `atom.stream.credit` (`[P-STREAM-3]`); erros `-32050`/`-32022` repassados sem virar `-32603`.
- [ ] `ui-web`: consumidor WS usa `"flow": {"window": N}` + `atom.stream.ack` nas streams do chat; reconexão de eventos
      com `from` e tratamento de `atom.events.gap`; estado de mod salvo com `if_version` (`-32041` = recarregar e reaplicar).
- [ ] `ui-web`: deixar de depender de `atom.ui.session` sem capability (pedir a capability que o core definir;
      `protocol.md §9.0`) e documentar que o shell é o ponto de aplicação de grants dos mods (a conexão WS do shell
      tem autoridade de cliente confiável).
- [ ] `engine` de todos os `atom.toml` com teto (`">=1.0.0, <2.0.0"`; `">=1.1.0, <2.0.0"` onde usar v1.1). Hoje `">=0.1.0"`.
- [ ] README de cada pacote lista capabilities *advisory* com o aviso `[C-UI-1]`.
- [ ] CI: `atom-conformance ext -- <binário>` (core v1.1) contra cada extensão oficial executável.
- [ ] `tests/e2e-core`: atualizar rev e reexecutar contra o binário v1.1 (`guard`, `kind`, `web`); incluir um caso de
      cancelamento e um de `deadline` (`-32022`) passando pelo `agent-loop-basic`.
- [ ] `release.yml`/`releasing.md`: publicar também no `v2/` do `atom-store` — publisher assina o delegado
      `publisher:atom` com `ATOM_PUBLISHER_KEY` e manda no PR (`atom-store/docs/store-v2.md §8`).

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
