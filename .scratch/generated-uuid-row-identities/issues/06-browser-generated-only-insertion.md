# 06: Qualify generated-only insertion through the browser SQL Client

**What to build:** The browser SQL Client demonstrates the complete generated-only identity contract against a real backend. Browser callers receive UUIDv7 primary keys for omitted and DEFAULT inserts and cannot insert explicit UUIDv5 or UUIDv7 primary keys. Final integration checks establish the specification across browser storage, native and remote SQL Clients, affected services, and updated Replicas.

**Blocked by:** 05: Enforce generated-only UUIDv7 primary keys in the SQL Client

**Status:** ready-for-agent

Work: open

- [ ] Real-backend browser tests prove omitted and DEFAULT inserts return and persist RFC-variant UUIDv7 primary keys.
- [ ] Real-backend browser tests prove explicit UUIDv5 and UUIDv7 primary keys are rejected through supported SQL text and parameter forms.
- [ ] Browser tests prove non-primary-key UUID values and natural or composite primary keys retain their existing behavior.
- [ ] Filtered browser package build, test, lint, and type checks pass, followed by affected dependent package checks.
- [ ] Real synchronization checks cover generated identities and convergence between fresh updated Replicas.
- [ ] Affected Device, IdP, Management, and Storage integration suites pass against fresh stores.
- [ ] Appropriate Rust feature-powerset, Clippy, CRAP, documentation, and TypeScript qualification is run for the full change surface.
- [ ] Any unrun or failed broader qualification is recorded explicitly rather than claimed as passing.

## Comments

Browser tests are integrated. The Chromium remote Storage SQL test and local WASM/IndexedDB test passed on the browser implementation tip, not the final integration tip. The WASM build required temporary workspace and KV import workarounds plus direct binding generation; those changes were not committed. The full SQL browser suite was blocked by missing JavaScript helpers. SQL package and dependent type/unit checks passed, but lint was blocked by Biome configuration. The password-manager real-backend suite failed in `Database.synchronize` (cause unconfirmed). Full owner integration, strict Clippy, feature-powerset, and CRAP qualification did not pass or was not completed.
