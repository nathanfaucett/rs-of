# Contributing

## Start here

Read in this order:

1. [README](README.md) for current support and repository layout.
2. [Product goal](docs/goal.md) for the north-star workflow and priorities.
3. [Target architecture](docs/architecture.md) for ownership boundaries.
4. [Glossary map](GLOSSARY-MAP.md) and the documents for the context you change.

Target architecture and current implementation are different document types. Do not present a target as delivered behavior.

## Canonical sources

All changes land in `nathanfaucett/rs-of`. Do not edit or build sibling `ofdb`, `ofnet`, or `offs` checkouts as part of normal development. Do not manually copy changes between mirrored source trees.

Generated files must come from their checked-in generators and existing package scripts. Do not hand-edit generated OpenAPI clients or WASM output.

## Toolchain

- Rust: `rust-toolchain.toml`; minimum language compatibility is in `Cargo.toml`.
- Node: `.nvmrc`.
- pnpm: 12.4.1.
- WASM work: `wasm-pack` and the `wasm32-unknown-unknown` target.
- Browser E2E: Chromium and the dependencies in [E2E setup](e2e/README.md).

Install JavaScript dependencies from the repository root:

```bash
pnpm install --frozen-lockfile --ignore-scripts
```

## Change workflow

1. Identify the user outcome and milestone.
2. Read the relevant context contract and existing tests.
3. Make the smallest complete change at the owning boundary.
4. Update current documentation, generated contracts, and tests in the same change.
5. Delete code and documentation made obsolete by the change. Do not leave parallel legacy guidance.
6. Run focused checks, then the required broader checks for the change's risk.

Root-cause fixes are preferred. Rewriting is not a default requirement. Public APIs, persisted data, and wire protocols need an explicit compatibility or migration decision; do not silently discard User data or retain unbounded compatibility glue.

Use the tracked local Markdown issue tracker in `.scratch/` for durable work and milestones. See `docs/agents/issue-tracker.md`. Use `.tmp/` for temporary agent analysis, generated reports, and evidence. Stable documentation must not depend on `.scratch` or `.tmp`.

## Documentation contract

- `README.md`: current support and one canonical entry path.
- `docs/goal.md`: target outcomes, priorities, and non-goals.
- `docs/architecture.md`: accepted target ownership and system design.
- Context design documents: current contracts and constraints for that context.
- OpenAPI: HTTP routes and payloads.
- Rustdoc and TypeScript declarations: API signatures.
- E2E evidence: GitHub Actions logs and artifacts tied to a specific commit or run.

Do not duplicate route tables, method inventories, or implementation-status tables across documents.

## Validation by change

| Change               | Required focused checks                                           | Broader checks when behavior or risk requires                               |
| -------------------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Documentation only   | `git diff --check`                                                | Render or execute changed examples where practical                          |
| Rust crate           | Focused crate tests, `cargo fmt --all -- --check`, focused Clippy | `cargo hack test --feature-powerset --workspace --all-targets`, `just crap` |
| TypeScript package   | Filtered build, test, lint, and type check                        | `pnpm test`, dependent package checks                                       |
| Public HTTP contract | OpenAPI generation/checks and generated-client tests              | Real-backend owner and browser tests                                        |
| Browser/WASM         | Filtered WASM build and browser tests                             | `pnpm test:backend`, `pnpm test:e2e`, supported browser matrix              |
| Deployment/lifecycle | Focused process tests                                             | Independent Create, restart, and Device admission and platform qualification                  |

Do not claim a check passed unless you ran it or can link to a specific CI result. Record unrun or failed qualification explicitly.

Inspect current CI runs and failed-step logs with:

```bash
gh run list
gh run view --log-failed
```

Useful commands:

```bash
just build
just test
just clippy
just crap
pnpm test
pnpm test:backend
pnpm test:e2e
```
