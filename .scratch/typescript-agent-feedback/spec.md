# Strict TypeScript, Biome, Svelte, and agent feedback

Status: ready-for-agent
Work: resolved
Milestone: M1 — Canonical foundation
Dependencies: Existing OpenAPI generators, package build/test workflows, and real-backend browser qualification.

## User outcome

Contributors and coding agents receive fast, deterministic feedback that catches nullability, async, dependency, Svelte, accessibility, and formatting defects before commit. The same commands run locally and in CI. Strict checks cover handwritten source and tests without weakening generated-code compatibility or product behavior.

## Scope

- Apply the shared strict TypeScript base to applications, packages, examples, E2E, and the Tauri TypeScript service.
- Add unreachable-code and unused-label errors. Remove broad per-package strict relaxations after separating generated compatibility.
- Make generated OpenAPI clients deterministic and safe for strict consumers. Do not hand-edit generated output without changing its generator.
- Run Biome formatter, linter, and assist checks with warnings fatal. Exclude temporary/build/generated artifacts and retain narrow verified Svelte overrides only.
- Enable measured high-value Biome rules for undeclared dependencies, non-null assertions, floating/misused Promises, and unnecessary conditions. Review each resulting diagnostic.
- Run `svelte-check` with strict config and warnings fatal, including application test code.
- Standardize root and package check/lint/fix/test commands. Use one-shot tests and fail generation commands visibly.
- Add repository-wide CI static gates using the same commands. Update contributor guidance.
- Run focused tests, package builds, deterministic OpenAPI checks, E2E type checks, and change-appropriate browser/backend qualification.
- Commit the completed change to `main` and push `origin/main` as requested.

## Non-goals

- Product feature changes, wire-protocol changes, persisted-data migrations, or generated-contract rewrites unrelated to type safety.
- Enabling every lint rule, adding a second general lint framework, or applying unsafe automatic fixes.
- Suppressing broad classes of handwritten-source errors to obtain a green result.
- Claiming unrun browser or synchronization qualification.

## Acceptance checks

- `pnpm lint` runs Biome formatter/linter/assist checks across all included workspace source and exits successfully with warnings fatal.
- `pnpm check` runs every TypeScript, Svelte, service, example, and E2E check and exits successfully with warnings fatal.
- Effective Svelte configs include the shared strict base. Password-manager tests are type checked.
- Generated OpenAPI clients pass their build and deterministic generation checks, and strict consumers do not inherit avoidable generated-source diagnostics.
- Package manifests declare imported dependencies. One-shot test scripts cannot enter watch mode unexpectedly.
- Promise, dependency, non-null, and unnecessary-condition rules pass without broad handwritten-source suppressions.
- Focused and affected package tests/builds pass. `pnpm openapi:check --committed` and `pnpm test:e2e:check` pass.
- CI runs root static gates, and contributor documentation names the same commands.
- Final diff is reviewed, only intended files are committed, and `origin/main` receives the commit after validation.

## Evidence and qualification limits

The initial audit found missing Svelte strict inheritance, nine broadly relaxed packages, generated-output noise, incomplete CI, and inconsistent commands. Raw temporary strict checks produced 353 errors in `apps/app`, 379 in `apps/password-manager`, and 3 in `packages/svelte-headless`; these counts include repeated generated-client diagnostics. Selected rule trials produced 19 Promise candidates, 53 condition warnings, and two undeclared dependencies.

## Comments

- 2026-11-21: User requested complete implementation, commit, and push to `main`. Initial nested Biome root inheritance repairs are preserved in this change.
- 2026-11-21: `pnpm lint`, `pnpm format`, and `pnpm check` pass across the full tree. `pnpm packages:build`, both application builds, and the complete `pnpm test` command pass, including Chromium and Firefox examples. Real-backend validation passes: 9 lifecycle tests, 9 Vitest backend tests, and 13 Playwright flows. `pnpm openapi:check` passes deterministic production exports, both SDK families, generated tests, and Rust server tests. Focused `cargo clippy` for `db/wasm` passes with warnings denied. Full Rust workspace feature-powerset, Clippy, and CRAP checks were not run.
- 2026-11-23: Final validation passes all static gates, package/application builds, the complete workspace tests, 67 SQL browser tests, and KV cross-peer tests in Chromium and Firefox. The ordered-import and deferred-ACK Rust regressions pass. Independent review reports no remaining KV/sync blockers. Real-backend validation passes 9 lifecycle tests, 5 password-manager Vitest tests plus the empty app suite, and 13 Playwright flows. `pnpm openapi:check` and focused WASM Clippy with warnings denied pass. The full `ofdb-kv-store` test command was attempted and still fails in two unchanged `value_document` tests; the new store regression passes. Full Rust workspace feature-powerset, Clippy, and CRAP checks were not run.
