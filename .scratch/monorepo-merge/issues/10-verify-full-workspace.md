# Ticket 10: Verify full workspace gates pass

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates, 02-rewrite-cargo-path-refs, 03-regenerate-cargo-lock, 04-rewrite-pnpm-path-refs, 05-regenerate-pnpm-lock, 06-merge-docs-and-glossaries, 07-port-ci-workflows, 08-fix-docker-and-container-contract, 09-extend-gitignore

## What it delivers

All repo hygiene gates and CI-equivalent checks pass on the merged workspace.

## Scope

Run the full verification sequence from `of/`:

1. `cargo metadata --format-version 1` — parses cleanly
2. `cargo check --workspace` — all crates compile
3. `just fmt-check` — formatting
4. `just clippy` — lints (zero warnings)
5. `just boundary-check` — server dependency boundary test
6. `python3 scripts/test_container.py` — container contract
7. `pnpm install --frozen-lockfile` — lockfile consistency
8. `pnpm sdk:build` — generated clients
9. `pnpm e2e:prepare` — frontend deps + wasm builds
10. `cargo hack test --feature-powerset --workspace --all-targets` — full test matrix (or `just test`)
11. `just crap-summary` — coverage/complexity

## Acceptance

All 11 commands exit 0. Any failures indicate a missing path rewrite, missing crate copy, or CI job not yet ported.

## Notes

This is the integration ticket. Each preceding ticket should be independently verifiable at a lower seam (cargo metadata for 01–03, pnpm build for 04–05, docker build for 08, etc.), but this ticket is the final "everything works together" checkpoint before the merge is considered done.
