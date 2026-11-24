# 06: Qualify generated-only insertion through the browser SQL Client

**What to build:** The browser SQL Client demonstrates the complete generated-only identity contract against a real backend. Browser callers receive UUIDv7 primary keys for omitted and DEFAULT inserts and cannot insert explicit UUIDv5 or UUIDv7 primary keys. Final integration checks establish the specification across browser storage, native and remote SQL Clients, affected services, and updated Replicas.

**Blocked by:** 05: Enforce generated-only UUIDv7 primary keys in the SQL Client

**Status:** ready-for-agent

Work: resolved

- [x] Real-backend browser tests prove omitted and DEFAULT inserts return and persist RFC-variant UUIDv7 primary keys.
- [x] Real-backend browser tests prove explicit UUIDv5 and UUIDv7 primary keys are rejected through supported SQL text and parameter forms.
- [x] Browser tests prove non-primary-key UUID values and natural or composite primary keys retain their existing behavior.
- [x] Filtered browser package build, test, lint, and type checks pass, followed by affected dependent package checks.
- [x] Real synchronization checks cover generated identities and convergence between fresh updated Replicas.
- [x] Affected Device, IdP, Management, and Storage integration suites pass against fresh stores.
- [x] Appropriate Rust feature-powerset, Clippy, CRAP, documentation, and TypeScript qualification is run for the full change surface.
- [x] Any unrun or failed broader qualification is recorded explicitly rather than claimed as passing.

## Comments

Browser tests are integrated. The Chromium remote Storage SQL test and local WASM/IndexedDB test passed on the browser implementation tip, not the final integration tip. The full SQL browser suite was blocked by missing JavaScript helpers. The password-manager real-backend suite failed in `Database.synchronize` (cause unconfirmed).

Final integration qualification passed SQL/KV package builds, type checks, and lint; both example lint and Svelte checks; the KV WASM build; 12 KV unit tests; 12 SQL Client/remote tests with one pre-existing ignore; and the final Chromium `row-identities.test.ts`. Formatting, documentation, and diff checks passed. The nested WASM workspace and KV imports are fixed without temporary patches. Biome 2.5.6 configuration and package/example scopes are aligned.

Focused SQL/KV Clippy passed with baseline allowances for redundant feature names and duplicate dependency versions; SQL also required the baseline collapsible-if allowance. Unmodified strict Clippy failed on those existing diagnostics. Example production builds remain unqualified: the SQL verifier expects two WASM files in a SQL-only build, and the KV example could not resolve `@ofsdk/kv`. Full SQL browser, password-manager/backend, full owner integration, feature-powerset, and CRAP qualification were not completed or remain blocked as recorded above.

2026-11-24: `pnpm lint`, `pnpm check`, full `pnpm test`, and `pnpm openapi:check` passed. This includes 67 SQL browser tests and the generated-row-identity coverage. Focused Device (73), IdP (71), Management (5), and Storage (33) tests and library Clippy passed. Native and WASM all-feature checks and all ten sync feature combinations passed, including the native receive-retention regression.

The full workspace feature-powerset remains incomplete after 565 of 2397 combinations because disk space was exhausted; no failing test was observed before that infrastructure stop. Full workspace Clippy remains unqualified because pre-existing server/setup diagnostics remain and a later invocation was disk-blocked. `just crap-summary` was attempted: its initial run reached Device failures that are now fixed, but the final run was disk-blocked before `cargo-crap` produced a summary. These broader limits are not claimed as passes.
