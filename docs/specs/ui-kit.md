# Spec: `atom/ui-kit` — design system

Status: rascunho (E1). Pacote em `kit/`.

## 1. Papel
- Único lugar com CSS (além do reset do shell). Implementado como **Web Components** com Shadow DOM.
- É o vocabulário de `ui.tree@0`: cada tipo de nó da árvore = um componente do kit.
- Mods JS legados usam os elementos diretamente; mods WASM só via árvore.

## 2. Componentes (v0)
| Tag | Uso | Props principais | Eventos |
|---|---|---|---|
| `atom-stack` | layout 1D | `direction` (row/column), `gap` (0–4), `align`, `justify`, `wrap` | — |
| `atom-grid` | layout 2D simples | `columns` (1–6 ou `auto`), `gap` | — |
| `atom-scroll` | área rolável | `axis` | — |
| `atom-text` | texto | `variant` (body/muted/heading-1..3/code), `text` | — |
| `atom-icon` | ícone do set do kit | `name` | — |
| `atom-button` | ação | `label`, `icon`, `variant` (primary/secondary/ghost/danger), `disabled` | `press` |
| `atom-field` | entrada | `type` (text/number/check/select/textarea/secret), `label`, `value`, `options`, `placeholder` | `change`, `submit` |
| `atom-list` | lista selecionável | `items[{id,label,icon?,badge?}]`, `selected`, `orientation` | `select` |
| `atom-menu` | menu de ações | `items` (aninháveis) | `select` |
| `atom-tabs` | abas | `items`, `selected` | `select` |
| `atom-dialog` | modal | `title`, `open` | `close` |
| `atom-badge`, `atom-spinner`, `atom-progress`, `atom-divider` | estado | — | — |
| `atom-markdown` | conteúdo rico seguro | `source` (sanitizado no kit) | `link` |
| `atom-log` | texto em streaming (chat, saída) | `lines`/`append` | — |
| `atom-slot` | monta um **ponto de contribuição** aninhado | `point` | — |

`atom-slot` é como um mod expõe pontos próprios (ADR-016 §6) sem conhecer quem renderiza.

## 3. Escala fixa (não tematizável)
- Espaçamento: `--atom-space-0..4` = 0, 4, 8, 12, 16px (props `gap`/`pad` aceitam só 0–4).
- Alturas de controle: 28px (compacto) / 32px (padrão), via `density` do container.
- Line-height: 1.45 corpo, 1.2 títulos.
- Ícones: 16/20px.

## 4. Regras
- Todo componente é acessível por teclado, com `role`/`aria-*` corretos e foco visível (`color.focus`).
- Texto vindo de mod é sempre texto (`textContent`); HTML só via `atom-markdown` (sanitizado).
- Nenhum componente aceita `style`, `class` ou CSS arbitrário vindo de mod.
- Mudança incompatível de tag/prop = novo major do kit **e** de `ui.tree@N`.
