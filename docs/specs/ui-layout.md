# Spec: layout estável (pontos de montagem, dicas, troca de mods)

Status: rascunho (E1). Implementação: shell + renderers de layout.

## 1. Quem manda
- **Renderer de container** (layout, menu, settings) define a geometria das áreas com CSS grid/flex do kit.
- **Contribuição** só declara dicas no manifesto; nunca larguras absolutas de página.

```toml
[contributes.layout]
"ui.views" = { min = "320px", preferred = "1fr", grow = 1 }
"ui.status.items" = { min = "24px", preferred = "24px", grow = 0 }
```
Unidades aceitas: `px`, `fr`, `%` do container. Ausente = padrão do ponto (definido no contrato do ponto).

## 2. Ponto de montagem (obrigatório no shell)
Todo mod é montado num `<atom-mount>` do shell:
```css
atom-mount { display: block; contain: layout paint style; min-width: 0; min-height: 0; overflow: auto; }
```
- Shadow DOM: CSS não entra nem sai.
- Antes do mod carregar: skeleton com o `preferred` da dica (zero layout shift).
- Error boundary: falha de load/render/worker → placeholder do mesmo tamanho com mensagem e "recarregar".
- Timeout de primeiro render: 3s → placeholder (mod continua tentando).

## 3. Troca de mod/renderer/tema
1. Shell obtém o novo binding (evento `atom.binding.changed` `{contract}` do core, emitido por `atom bind`; editar `[bindings]` à mão só vale após `atom profile reload`, `[C-AMB-3]`).
2. Salva `state` das instâncias afetadas (já persistido a cada `update`).
3. `document.startViewTransition()` (se disponível) → desmonta renderer antigo → monta o novo com as **mesmas
   contribuições** e estado restaurado.
4. Contribuições aninhadas (`atom-slot`) remontam pelo mesmo caminho; não sabem que o container mudou.
Tema: só troca custom properties; nada é remontado.

## 4. Responsividade
- Container queries (`@container`) dentro do kit; nunca media queries em mods.
- Cada renderer de layout declara breakpoints próprios do container (ex.: menu lateral colapsa < 600px).

## 5. Verificação (CI)
- Playwright: para cada combinação {layout × renderers × tema × largura (360, 1024, 1440)} grava screenshot e
  bounding boxes das áreas. Mudar um mod de conteúdo não pode alterar bounding boxes de áreas que não o contêm.
- Lint de pacote: mod JS com `<style>`/`document.head`/`document.body` direto → erro (exceto nível superfície).
