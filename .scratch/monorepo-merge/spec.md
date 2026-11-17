# Spec: Absorb ofdb, ofnet, and offs into the of monorepo

Status: ready-for-agent

## Problem Statement

The `of` repository still depends on three sibling repositories (`ofdb`, `ofnet`, `offs`) through relative path dependencies (`../ofdb/crates/...`, `../ofnet/crates/...`, `../offs/crates/...`), external GitHub checkouts in the e2e workflow, `link:` pnpm dependencies, Docker build context entries, and documentation links. The source folders were already copied into `of/` (`db/`, `fs/`, `net/`, `packages/sql|kv|net|file-system|example`), and `of/crates/` was renamed to `of/services/` on disk, but no manifest, workflow, doc, or test was updated. As a result the Cargo workspace does not resolve, CI checks out dead repos, e2e links point outside the repo, and the glossary references a repo that no longer exists at that path. Only 7 of 32 ofdb crates were copied; the copied crates still reference not-yet-copied siblings.

## Solution

Finish the migration inside `of` only:

1. Copy the remaining 25 ofdb crates into `of/db/` so every sibling path dependency resolves locally.
2. Make the moved crates first-class workspace members and rewrite every path reference (Cargo, pnpm, Docker, scripts).
3. Merge documentation: one root `GLOSSARY.md`, updated `GLOSSARY-MAP.md`, README, and package READMEs.
4. Port CI/CD: remove external repo checkouts, port the ofdb/ofnet/offs workflow jobs and just recipes into `of`.
5. Fix the deploy contract: single-repo Docker context and updated container contract test.
6. Regenerate `Cargo.lock` and `pnpm-lock.yaml`; extend `.gitignore` for nested build output.

## User Stories

1. As a developer, I want `cargo metadata` and `cargo check --workspace` to succeed in `of` without any sibling checkout, so that I can build the whole workspace locally.
2. As a developer, I want `cargo hack test --feature-powerset --workspace --all-targets` to run the moved db/fs/net crates as workspace members, so that their tests run in CI like every other crate.
3. As a developer, I want the `db/wasm` crate to stay a standalone wasm-pack workspace (own lockfile), mirroring how ofdb excluded it, so that browser builds keep working unchanged.
4. As a developer, I want all `ofdb-*`, `iroh-chain`, and `file-system` package names and feature flags (for example `ofdb`, `native-stage`, `sync`, `indexeddb`) to keep working unchanged, so that no Rust code in `services/` or `tests/` needs to change imports.
5. As a maintainer, I want CI to stop checking out `rs-ofdb`, `rs-ofnet`, and `rs-offs` and instead build the in-repo wasm packages, so that e2e tests run against the same commit as the rest of the repo.
6. As a maintainer, I want the ofdb remote-feature isolation checks (ofdb-sql/ofdb-kv `--features remote` must not pull embedded engines or sync crates) ported into the `of` lint job, so that the dependency isolation guarantee survives the move.
7. As a maintainer, I want the ofdb browser test matrix and native regression jobs ported, so that wasm/browser behavior of the moved crates is still verified.
8. As a maintainer, I want the ofdb docker publish flow (kv/sql server images) ported to the single-repo layout, so that server images can still be built and published.
9. As an operator, I want the Docker image to build from `of/` alone, so that deployment no longer requires four repository checkouts in the build context.
10. As a developer, I want `pnpm install --frozen-lockfile` to succeed and `packages/sql`, `packages/kv`, `packages/net`, `packages/file-system` to build their wasm artifacts from `db/`, `net/`, `fs/`, so that browser apps work without the old repos.
11. As a developer, I want the password-manager app to consume `@ofsdk/sql` from the in-repo package instead of `link:../../../ofdb/packages/sql`, so that e2e and local dev use the workspace package.
12. As a reader, I want the root `GLOSSARY.md` to define Endpoint, Peer, Mesh, Broadcast, Direct Message, Byte Stream, and the ofdb/offs terms inline, so that docs no longer link to `../ofnet/GLOSSARY.md`.
13. As a reader, I want `GLOSSARY-MAP.md`, `README.md`, and package READMEs to reference `services/…`, `db/…`, `fs/…`, `net/…` instead of `crates/…` and the old repo names, so that all doc links resolve inside `of`.
14. As a developer, I want `just clippy`, `just fmt-check`, `just boundary-check`, and the container contract test (`scripts/test_container.py`) to pass, so that repo hygiene checks match the new layout.
15. As a developer, I want `.gitignore` to cover nested `target/`, wasm `pkg/`, `test-results/`, and `artifacts/` output of the moved crates, so that copied build output does not pollute git status.
16. As a maintainer, I want the old `ofdb`/`ofnet`/`offs` repositories left untouched on disk and on GitHub for history, so that the move is purely additive inside `of`.

## Implementation Decisions

- Destination layout (already chosen by the user, keep it): `ofdb/crates/*` → `of/db/*`, `ofnet/crates/*` → `of/net/*`, `offs/crates/*` → `of/fs/*`, TS wrappers → `of/packages/*`, and `of/crates/*` → `of/services/*`.
- Workspace membership: members become `services/*`, `db/*` except `db/wasm`, `fs/*`, `net/*`, and the existing tauri app. `db/wasm` stays excluded (standalone lockfile, wasm-pack build), mirroring ofdb's `exclude = ["crates/wasm"]`.
- Path rewrite rules for Cargo manifests: from the root manifest `../ofdb/crates/X` → `db/X` and `../ofnet/crates/iroh-chain` → `net/iroh-chain`; from `services/*` `../../../ofdb/crates/X` → `../../db/X`, `../../../ofnet/...` → `../../net/...`, `../../../offs/...` → `../../fs/...`; everywhere `crates/X` → `services/X` (root members, root dependencies and dev-dependencies, tauri app manifest).
- Moved crates keep their Cargo package names (`ofdb-sql`, `ofdb-kv`, `iroh-chain`, `file-system`, …) and `version = "0.1"` so path deps from `services/*` and root dev-deps keep resolving; their `repository` fields point at the `of` repository (fixes the ofnet manifest that wrongly says `rs-offs`).
- The moved crates inherit `of`'s `[workspace.package]` and `[workspace.lints]`; add any missing shared dependency entries to `[workspace.dependencies]` only if a moved crate references them with `.workspace = true`.
- pnpm wiring: `packages/kv`, `packages/net`, `packages/file-system` `build:wasm` scripts point at `../../db/wasm`, `../../net/iroh-chain-wasm`, `../../fs/file-system-wasm`; port `scripts/build-sql-wasm.mjs` from ofdb into `of/scripts/` and point it at `db/wasm`; `apps/password-manager` uses a workspace/link dependency on the in-repo `@ofsdk/sql`; `pnpm-workspace.yaml` globs drop the dead `crates/iroh-client-wasm` and point `crates/tauri-plugin-fetch-api` at `services/tauri-plugin-fetch-api`; root `packages:build` filter uses `./services/*`.
- Lockfiles: regenerate `Cargo.lock` (CI builds `--locked`) and `pnpm-lock.yaml` (`--frozen-lockfile`).
- CI: the e2e workflow drops the three external `actions/checkout` steps and the ofdb package build step; it builds the in-repo `@ofsdk/sql` wasm+esm with the pinned rust 1.99.1 / wasm-pack 0.13.1 already present. Port ofdb `test.yml` jobs into `of/.github/workflows/` (lint with the remote-feature isolation greps, cargo tests with crap coverage, browser matrix, native regression) and ofnet's wasm job; merge the needed just recipes (`browser-*`, `build-wasm`, `test-wasm`, …) into `of/justfile` and port the supporting `tests/browser` harness, `test-fixtures/`, shell scripts, `Dockerfile.server`, and `compose.yaml` from the old repos into their `db/` counterparts.
- Docker: `Dockerfile` builds only from `of/` (remove the `COPY ofdb/ offs/ ofnet/` lines); `Dockerfile.dockerignore` drops those allowlist entries; `scripts/test_container.py` expects the single-repo context and that every Cargo path dependency resolves inside `of/`.
- Docs: merge the three old root glossaries into `of/GLOSSARY.md` (single-context rule); update `GLOSSARY-MAP.md` (context links `crates/…` → `services/…`, drop the `../ofnet/GLOSSARY.md` link), `README.md`, and package READMEs/comments (`crates/…` → `services/…`, `ofdb/crates/…` → `db/…`).
- `.gitignore`: add `**/target/`, `**/pkg/` where generated (except intentionally checked-in wasm packages), `**/test-results/`, `**/artifacts/`.

## Testing Decisions

- Highest seam only: the workspace root. A change is good when `cargo metadata` resolves, `cargo check --workspace` passes, and the repo's own gates pass: `just fmt-check`, `just clippy`, `just test` (`cargo hack test --feature-powerset --workspace --all-targets`), `just boundary-check`, and `python3 scripts/test_container.py`.
- pnpm seam: `pnpm install --frozen-lockfile`, `pnpm sdk:build`, `pnpm e2e:prepare`, and each moved package's `build:wasm` produce artifacts under `packages/*/pkg`.
- Ported CI checks act as regression tests: the ofdb remote-feature isolation greps (`cargo tree -p ofdb-sql/ofdb-kv --no-default-features --features remote` must not list embedded engines, redb, automerge, or sync crates) and the browser matrix.
- Tests assert external behavior only: workspace resolves, binaries and wasm packages build, existing tests pass, doc links and Docker contract hold. No tests of internal file layout beyond the path-dependency contract test.
- Prior art: `of/tests/` (owner boundaries, system tests), ofdb's `tests/browser` wasm-bindgen harness, and the existing `ci.yml`/`e2e.yml` structure are the models to extend.

## Out of Scope

- Renaming `ofdb-*` / `file-system` / `iroh-chain` package names or changing any public Rust/TS API of the moved crates.
- Changing feature flags, domain behavior, or storage formats of the moved crates.
- Deleting or modifying the old `ofdb`/`ofnet`/`offs` repositories (they remain as history).
- Creating ADRs (repo rule: design notes live in stable topic files, not numbered decision records).
- Resolving the pre-existing duplicate `@ofsdk/sql` package name (`packages/sql` wrapper vs checked-in `packages/sql-wasm`); flagged, not changed.

## Further Notes

- `of/GLOSSARY-MAP.md` references root `docs/*.md` topic files that do not exist yet in `of/docs/`; pre-existing, left as-is.
- The e2e workflow's pinned `rs-ofdb` ref (`544b627…`) disappears with the external checkout.
- `of`'s `[workspace.lints]` is a superset-in-spirit of the old repos' lint tables; if a moved crate trips a lint the old workspace did not have enabled (for example `empty_structs_with_brackets`), decide per crate rather than weakening the shared table.
- Old repos' `docs/agents/` files are near-identical to `of/docs/agents/`; keep `of`'s copies, merge only if content diverges.
