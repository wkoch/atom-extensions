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

## ADR-E006 — Contribuições de UI são dados primeiro (implementa ADR-025 do core)
- **Contexto:** ADR-025 (core) emenda ADR-016 §3–4: contribuição deixa de ser "dados + componente JS" e vira
  descritor serializável; componentes são opcionais e por superfície. O trabalho é todo deste repo. Hoje os
  schemas `ui.*@0` descrevem o renderer como "função JS no módulo" e `ui.views` aceita `module`/`html`.
- **Decisão:** spec `specs/ui-contributions.md`: descritor base (`id`, `title|label`, `icon`, `order`, `when`,
  `action`, `points`, `surfaces`), ações `call|command|view`, componentes em `surfaces.web|tui`, renderers
  por superfície (`[contributes.renderers."<ponto>".web]`). `ui-cli` passa a renderizar menus/views/settings
  genericamente na superfície `tui`. Emenda ADR-E002: o `InitIn.items` da ABI WASM são descritores.
- **Consequência:** qualquer frontend reaproveita as contribuições; mods ficam menores (muitos viram só
  declarativos). Quebra dos `ui.*@0` registrada no CHANGELOG (ADR-014); shell aceita a forma antiga com aviso
  até E3.

## ADR-E007 — Extensões oficiais alinhadas ao core v1.1
- **Contexto:** o core v1.1 adicionou ao `atom/1`, de forma opt-in: `$/cancelRequest` propagado ao provider,
  `$ctx.deadline`, backpressure padrão + `flow` (window/credit), `-32050`/`-32022`/`-32041`, eventos com
  `epoch/offset/from/gap`, KV versionado + `watch`, e identidade por conexão (extensão conectando no socket
  do daemon recebe `-32005`; editar `grants.toml`/`profile.toml` por fora é ignorado). As extensões deste repo
  foram extraídas na v1.0, reimplementam o protocolo à mão e fixam o SDK num commit pré-v1.1.
- **Decisão:**
  1. Toda extensão Rust usa `atom-sdk` v1.1 (tag). O SDK é o único lugar que implementa cancelamento,
     deadline e crédito; nenhuma extensão reimplementa.
  2. Providers tratam cancelamento (`$/cancelRequest` e `atom.stream.cancel`) e respeitam `deadline` nas
     sub-chamadas (via SDK). Providers de stream longo (`llm-*`, `agent-loop-basic`) declaram
     `flow = "credit"`.
  3. Nenhuma extensão se conecta ao socket do daemon nem edita arquivos do perfil; bindings/grants mudam só
     por comandos de cliente confiável (`atom bind`, `atom grants`).
  4. Pacote que usa recurso v1.1 declara `engine = ">=1.1.0, <2.0.0"`; os demais `">=1.0.0, <2.0.0"`.
  5. Capabilities `fs.*`, `net`, `exec`, `ui.serve` são documentadas como *advisory* no README de cada pacote.
- **Consequência:** comportamento consistente sob cancelamento/prazo/carga; dependência dos itens pendentes do
  core (SDK com `$/cancelRequest`, tag publicada). O `ui-web` depende de `atom.ui.session`, que o core vai
  restringir (`atom/docs/specs/protocol.md §9.0`): o shell deve pedir a capability que o core definir.
