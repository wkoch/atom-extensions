# Decisões (ADRs deste repo)

Prefixo `E` para não colidir com os ADRs do core (`atom/docs/decisions.md`). Formato:
contexto → decisão → consequência. Não editar; substituir com nova ADR.

## ADR-E001 — Três repositórios: core, extensões oficiais, loja
- **Contexto:** após a v1.0 o core congelou, mas contratos `@0`, extensões e mods mudam toda semana.
  No monorepo o CI do core compilava tudo, as extensões não usavam o SDK público e a chave da loja
  convivia com código.
- **Decisão:** `atom` (kernel + SDKs), `atom-extensions` (oficiais, versão por pacote), `atom-store`
  (dados assinados). Extensões dependem do `atom-sdk` por versão/tag publicada.
- **Consequência:** dogfooding real do SDK; releases independentes; chaves separadas por papel
  (ADR-011 do core). Custo: mudança no SDK exige release do `atom` antes de usar aqui.

## ADR-E002 — Mods em Rust → WASM, rodando em Web Worker, emitindo árvore UI
- **Contexto:** frameworks Rust de UI (Leptos, Dioxus, Yew) assumem um app único. N mods = N runtimes
  de framework, memórias separadas e sem linking dinâmico no browser; composição entre mods inviável.
- **Decisão:** mod WASM **não acessa o DOM**. Roda em Worker e expõe `init/view/update` (ABI JSON em
  `specs/mod-wasm-abi.md`); `view` devolve uma árvore no vocabulário `ui.tree@0`; o shell renderiza com
  o `ui-kit`. Efeitos (bus, storage, navegação) são pedidos ao shell, que aplica grants.
  Crate `atom-ui` dá builder Rust para a árvore.
- **Consequência:** sandbox real (Worker), consistência visual por construção, mods pequenos, mesma ABI
  executável em wasmtime no futuro. Limite: interações muito ricas (editor, canvas) vão para o nível
  "superfície" (ADR-E004).

## ADR-E003 — Design system único e tokens como contrato
- **Contexto:** trocar mods quebra layout quando cada um traz CSS próprio ou depende de classes do shell.
- **Decisão:** `atom/ui-kit` (Web Components com Shadow DOM) é o único dono de CSS. Temas são mods que
  só fornecem valores de tokens definidos por `ui.theme@0` (chaves fixas, validadas por schema).
  Espaçamento estrutural é do kit, não do tema. O shell não tem CSS além de reset.
- **Consequência:** trocar tema/mod não altera geometria. Vocabulário do kit versiona por major junto
  com `ui.tree@N`.

## ADR-E004 — Geometria pertence ao container; superfície livre é exceção
- **Decisão:** renderers definem áreas (CSS grid); contribuições só declaram dicas de tamanho no
  manifesto. Cada montagem é isolada (`contain`, Shadow DOM, error boundary). Mods que precisam de DOM
  livre declaram capability `ui.surface` e recebem uma caixa de tamanho imposto pelo container
  (Shadow DOM para `atom/*`, `<iframe sandbox>` para terceiros).
- **Consequência:** nenhum mod consegue deslocar outro; troca de renderer só remonta conteúdo.

## ADR-E005 — Versão e release por pacote
- **Decisão:** cada diretório é um pacote com versão própria; tag `<dir>@<versão>` publica só ele.
  `atom.toml` e `Cargo.toml` devem ter a mesma versão (CI verifica). Pacotes sem binário são `any`.
- **Consequência:** sem "release do repo inteiro"; CI de PR roda só no que mudou (+ dependentes).
