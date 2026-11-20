# 06: Qualify generated-only insertion through the browser SQL Client

**What to build:** The browser SQL Client demonstrates the complete generated-only identity contract against a real backend. Browser callers receive UUIDv7 primary keys for omitted and DEFAULT inserts and cannot insert explicit UUIDv5 or UUIDv7 primary keys. Final integration checks establish the specification across browser storage, native and remote SQL Clients, affected services, and updated Replicas.

**Blocked by:** 05: Enforce generated-only UUIDv7 primary keys in the SQL Client

**Status:** ready-for-agent

Work: resolved

- [ ] Real-backend browser tests prove omitted and DEFAULT inserts return and persist RFC-variant UUIDv7 primary keys.
- [ ] Real-backend browser tests prove explicit UUIDv5 and UUIDv7 primary keys are rejected through supported SQL text and parameter forms.
- [ ] Browser tests prove non-primary-key UUID values and natural or composite primary keys retain their existing behavior.
- [ ] Filtered browser package build, test, lint, and type checks pass, followed by affected dependent package checks.
- [ ] Real synchronization checks cover generated identities and convergence between fresh updated Replicas.
- [ ] Affected Device, IdP, Management, and Storage integration suites pass against fresh stores.
- [ ] Appropriate Rust feature-powerset, Clippy, CRAP, documentation, and TypeScript qualification is run for the full change surface.
- [ ] Any unrun or failed broader qualification is recorded explicitly rather than claimed as passing.

## Comments

Browser tests are integrated. The Chromium remote Storage SQL test and local WASM/IndexedDB test passed on the browser implementation tip, not the final integration tip. The full SQL browser suite was blocked by missing JavaScript helpers. The password-manager real-backend suite failed in `Database.synchronize` (cause unconfirmed).

Final integration qualification passed SQL/KV package builds, type checks, and lint; both example lint and Svelte checks; the KV WASM build; 12 KV unit tests; 12 SQL Client/remote tests with one pre-existing ignore; and the final Chromium `row-identities.test.ts`. Formatting, documentation, and diff checks passed. The nested WASM workspace and KV imports are fixed without temporary patches. Biome 2.5.6 configuration and package/example scopes are aligned.

Focused SQL/KV Clippy passed with baseline allowances for redundant feature names and duplicate dependency versions; SQL also required the baseline collapsible-if allowance. Unmodified strict Clippy failed on those existing diagnostics. Example production builds remain unqualified: the SQL verifier expects two WASM files in a SQL-only build, and the KV example could not resolve `@ofsdk/kv`. Full SQL browser, password-manager/backend, full owner integration, feature-powerset, and CRAP qualification were not completed or remain blocked as recorded above.
