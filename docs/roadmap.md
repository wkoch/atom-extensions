# Roadmap do atom-extensions

Ordem obrigatória. Item só fecha com CI verde (Linux/macOS/Windows). Legenda `[ ]`/`[x]`.

## E0 — Extração e base
- [ ] Repo criado no GitHub; CI (`ci.yml`) verde com o código extraído.
- [ ] `atom-sdk` publicado pelo repo `atom` (crates.io ou tag git) e referenciado em `[workspace.dependencies]`.
- [ ] Migrar cada extensão Rust para `atom-sdk` (remover protocolo feito à mão). Começar por `tools-fs`.
- [ ] Testes e2e que hoje vivem em `atom/crates/atom-core/tests/{guard,kind,...}_e2e.rs` e usam
      `extensions/*` movidos para cá (`tests/` na raiz, usando o binário `atom` de release).
- [ ] `release.yml` funcionando: tag `tools-fs@x.y.z` → artefatos 5 plataformas → PR no `atom-store`.

Aceite: perfil limpo instala `atom/code-starter` da loja publicada por este repo.

## E1 — Tokens, kit e shell limpo
- [ ] Contrato `ui.theme@0` (`contracts/ui-theme`) com schema de tokens (spec `ui-theme.md`).
- [ ] `kit/` = `atom/ui-kit`: Web Components da spec `ui-kit.md` + tema padrão claro/escuro.
- [ ] Remover de `extensions/ui-web/assets/style.css` tudo que não for reset; mods JS existentes passam a usar o kit.
- [ ] Ponto de montagem isolado (`specs/ui-layout.md` §2) no shell; error boundary + skeleton por dicas de tamanho.
- [ ] Testes visuais (Playwright) da matriz {layout-basic} × {menu-top, menu-side} × {settings-popup, settings-panel} × {claro, escuro}.

Aceite: trocar qualquer renderer ou tema não muda a posição/tamanho de nenhuma outra área (diff de screenshot).

## E2 — Mods WASM
- [ ] Contrato `ui.tree@0` (vocabulário = componentes do kit) — spec `ui-tree.md`.
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
