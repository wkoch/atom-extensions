# atom/ui-web

Shell web mínimo (ADR-016): serve assets, ponte p/ o bus, loader de ui-mod. Sem telas próprias.

## Capabilities

- `ui.serve (HTTP 127.0.0.1:0)` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.
- `service:atom.packages.mods` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.
- `service:atom.packages.asset` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.
- `atom.ui.session (token WS)` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.

A lista normativa está em `atom.toml` (`capabilities`).
