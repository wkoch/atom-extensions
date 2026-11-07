# atom/agent-loop-basic

Loop agente simples: llm ↔ tools até stop (ai.agent@0)

## Capabilities

- `service:ai.llm@0` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.
- `service:ai.tools@0` — advisory [C-UI-1]: o core não bloqueia; a tela de aprovação diz isso.

A lista normativa está em `atom.toml` (`capabilities`).
