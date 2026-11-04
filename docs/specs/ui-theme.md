# Spec: `ui.theme@0` — tokens de tema

Status: rascunho (E1). Contrato em `contracts/ui-theme`.

## 1. Modelo
- Um mod de tema contribui no ponto `ui.theme.tokens` (cardinalidade `single`, escolhido no perfil:
  `[bindings] "ui.theme.tokens" = "atom/theme-default"`).
- O tema só fornece **valores**; as **chaves são fixas** neste contrato. Chave desconhecida ou valor
  fora do formato = mod recusado (cai no tema padrão do kit) com aviso no log.
- O shell aplica tokens como custom properties em `:root` (`--atom-<chave>`); o kit só lê essas variáveis.
- Esquemas claro/escuro: um tema pode trazer `light` e `dark`; o shell escolhe por `prefers-color-scheme`
  ou pela config `ui.theme.scheme = "light"|"dark"|"auto"`.

## 2. Chaves (v0)
| Grupo | Chaves | Formato |
|---|---|---|
| Cor | `color.bg`, `color.surface`, `color.surface-2`, `color.fg`, `color.fg-muted`, `color.border`, `color.accent`, `color.accent-fg`, `color.danger`, `color.warning`, `color.success`, `color.focus` | cor CSS (`#rgb[a]`, `#rrggbb[aa]`, `rgb()`, `oklch()`) |
| Tipografia | `font.sans`, `font.mono` | lista de famílias (sem `url()`) |
| | `font.size-base` | `px` entre 12 e 18 |
| | `font.scale` | número 1.1–1.333 (escala modular) |
| Forma | `radius.sm`, `radius.md`, `radius.lg` | `px` 0–16 |
| | `shadow.level` | 0–3 |
| Movimento | `motion.duration` | `ms` 0–300 |

**Fora do tema, de propósito:** espaçamento, alturas de barra, larguras de área, line-height estrutural.
São do kit (`ui-kit.md` §3); assim um tema nunca muda a geometria.

## 3. Exemplo
```toml
# mods/theme-default/atom.toml
manifest = 1
name = "atom/theme-default"
version = "0.1.0"
kind = "ui-mod"

[contributes."ui.theme.tokens".light]
"color.bg" = "#ffffff"
"color.fg" = "#1b1b1f"
"color.accent" = "#3b5bdb"
"radius.md" = "6px"

[contributes."ui.theme.tokens".dark]
"color.bg" = "#121214"
"color.fg" = "#e8e8ea"
"color.accent" = "#748ffc"
```
Chaves omitidas herdam o padrão do kit.

## 4. Regras de acessibilidade (validadas no load)
- Contraste `color.fg`/`color.bg` e `color.accent-fg`/`color.accent` ≥ 4.5:1 (WCAG AA); falhou → aviso e padrão.
- `color.focus` obrigatório distinto de `color.bg`.
- `motion.duration` é zerado se `prefers-reduced-motion`.
