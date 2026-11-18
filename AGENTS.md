# AGENTS.md

## Required reading

Read in this order before planning:

1. `README.md` for current support.
2. `docs/goal.md` for the north-star workflow and priorities.
3. `docs/architecture.md` for accepted target boundaries.
4. `GLOSSARY-MAP.md` and the relevant context documents.

Do not present target architecture as current behavior. Generated OpenAPI and current source establish implemented contracts; ADRs establish accepted decisions; `.scratch` contains temporary work only.

## Source ownership

`nathanfaucett/rs-of` is canonical. Never require sibling `ofdb`, `ofnet`, or `offs` checkouts or manually mirror changes into another source tree.

## Changes

- Fix root causes at the owning context. Reuse existing public interfaces and libraries.
- Make the smallest complete change for the user outcome.
- Delete code and documentation made obsolete by the change; do not leave parallel legacy guidance.
- Do not rewrite by default. Preserve working behavior unless the task requires a contract change.
- Make an explicit compatibility/migration decision for persisted data, public APIs, and wire protocols.
- Update current docs, generated contracts, and tests with behavior changes.
- Use GitHub Issues for durable work. Stable docs must not link to `.scratch`.

## Rust conventions

- Default to `no_std`; enable `std` only when required.
- Use explicit imports and minimal dependencies. Order std/core/alloc, external, then internal imports.
- Keep `lib.rs` and `mod.rs` thin. Put implementation in sibling modules.
- Use only `pub` or private visibility; re-export public API from the crate/module root.
- Dependencies use `major.minor`, full table form, `default-features = false`, and only required features.
- Use `expect("clear invariant message")` instead of `unwrap()`.
- Follow `rustfmt` and Clippy. Do not add comments that restate code.

## Documentation

- Keep glossaries limited to domain definitions.
- Keep HTTP routes/payloads in OpenAPI and API signatures in Rustdoc/TypeScript declarations.
- Keep current behavior, target design, and commit-specific evidence separate.
- Avoid duplicated route tables, source inventories, and manually maintained status tables.
- Run `just docs-check` for documentation changes.

## Validation

Use focused tests while working. Select final validation by change and risk; see `CONTRIBUTING.md`.

- Rust behavior: focused tests plus the appropriate feature-powerset, Clippy, and CRAP checks.
- TypeScript: filtered build, test, lint, and type check, then affected dependents.
- Public contracts: OpenAPI/generated-client checks and real-owner tests.
- Browser or synchronization behavior: real-backend browser tests; unit tests alone are insufficient.

Never claim an unrun check passed. Record failures and qualification limits.

## ADRs

Use `docs/adr/` only for hard-to-reverse decisions with real alternatives and non-obvious rationale. Keep ADRs short. Do not use them for current behavior or task history.
