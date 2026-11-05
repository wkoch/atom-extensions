# Release e publicação

## Papéis e chaves
- **Publisher `atom`** (este repo): chave ed25519 em secret `ATOM_PUBLISHER_KEY` (seed `ed25519-seed:BASE64`).
  A pública vai em `atom-store/publishers/atom.toml`. Em `store/v1` assina cada artefato (`atom-artifact-v1`);
  em `store/v2` assina o papel delegado `publisher:atom` (índice + hashes), `atom-store/docs/store-v2.md §2`.
- **Loja** (`atom-store`): chaves próprias, nunca neste repo. `v1`: re-assina índices no merge. `v2`: assina
  `targets`/`snapshot`/`timestamp`; nunca recebe a chave do publisher.
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
   - abre PR no `atom-store` com `out/*.index.json` mesclado (`atom ext publish --store-dir v1 ...` sem chave da loja);
   - (`store/v2`, quando o core tiver a ferramenta — `store-v2.md §8`) gera e assina o delegado `publisher:atom`
     com versão incrementada sobre a última publicada e inclui no mesmo PR.
5. Mantenedor da loja revisa e faz merge; CI da loja assina índice e regenera `catalog.json` (`v1`) e
   assina `targets`/`snapshot`/`timestamp` (`v2`).

## Regras
- Versão publicada é imutável; correção = nova versão. Retirada = `yanked` na loja.
- Capability nova em update pausa auto-update do usuário (core v0.7): mencionar no CHANGELOG.
- `engine` sempre com teto (`<2.0.0`); subir o piso para `>=1.1.0` ao usar recurso do core v1.1 (ADR-E007).
- CLI `atom` usado no `release.yml` (`vars.ATOM_VERSION`) fixado numa release v1.1+ do core.
- Contrato `@0` com quebra: publicar contrato + todos os providers/consumidores oficiais juntos.
