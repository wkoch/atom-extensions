# Release e publicação

## Papéis e chaves
- **Publisher `atom`** (este repo): chave ed25519 em secret `ATOM_PUBLISHER_KEY` (seed `ed25519-seed:BASE64`).
  A pública vai em `atom-store/publishers/atom.toml`.
- **Loja** (`atom-store`): chave própria, nunca neste repo. A loja re-assina índices no merge.
- `ATOM_STORE_TOKEN`: token com permissão de abrir PR no `atom-store` (fine-grained, só esse repo).

## Fluxo
1. Bump de versão em `atom.toml` (+ `Cargo.toml` se Rust) e entrada no `CHANGELOG.md`.
2. Merge no `main`.
3. `git tag tools-fs@0.2.1 && git push --tags`.
4. `release.yml`:
   - descobre o diretório pela tag (`extensions/`, `mods/`, `contracts/`, `distributions/`, `kit/`);
   - Rust: build nos 5 alvos (matriz igual à do core: musl via zigbuild, macOS, Windows msvc);
   - `atom ext pack <dir> --key <publisher> --out-dir out/` por plataforma (ou uma vez, `any`);
   - anexa `.tar.gz` a uma GitHub Release com a tag;
   - abre PR no `atom-store` com `out/*.index.json` mesclado (`atom ext publish --store-dir v1 ...` sem chave da loja).
5. Mantenedor da loja revisa e faz merge; CI da loja assina índice e regenera `catalog.json`.

## Regras
- Versão publicada é imutável; correção = nova versão. Retirada = `yanked` na loja.
- Capability nova em update pausa auto-update do usuário (core v0.7): mencionar no CHANGELOG.
- Contrato `@0` com quebra: publicar contrato + todos os providers/consumidores oficiais juntos.
