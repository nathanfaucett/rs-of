# 02: Open a filesystem through TanStack DB

**What to build:** A User can sign in, discover or create a filesystem Storage Resource, and view its secrets through a TanStack DB live query. Switching filesystems or ending the authenticated context clears obsolete records and editor selection safely.

**Blocked by:** None (can start immediately).

**Triage:** ready-for-agent

Status: resolved

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope; the umbrella ticket is not an additional implementation prerequisite.

## Acceptance criteria

- [x] Verify compatible TanStack DB core and official Svelte APIs; add the framework-independent `@aicacia/storage-db-collection` workspace package and consume it alongside `@tanstack/svelte-db` in the password manager.
- [x] The reusable package uses the existing Storage client boundary, injected configuration, validated records, and safe keys/paths; it has no Svelte, app-schema, environment-module, or fixed OAuth Client ID dependency.
- [x] Sign-in, filesystem discovery/creation/opening, and initial secret loading work with explicit service bases. Correct stale IdP configuration and domain terminology along this workflow.
- [x] Display loading, empty, ready, malformed-record, and authorization-error states accurately; query the secrets collection with favorite-first/name ordering instead of maintaining a duplicate list.
- [x] Switching Filesystem ID or authenticated context clears old data and editor selection, disposes obsolete collections and owned connections, and ignores late results. Basic resource cleanup is part of this slice, not deferred hardening.
- [x] Use the real TanStack DB public API with controlled Storage responses to test initial load, empty/error states, unsafe keys, explicit service prefixes, rapid switching, authentication changes, and disposal during load. Run relevant package and app checks.
- [x] Keep this slice usable without bypassing the collection for writes: until ticket 03 provides collection mutations, do not expose active legacy write controls against the collection-backed view.
- [x] Document the read workflow and its current limitations. No custom Svelte wrapper, polling, or automatic reconnect is introduced.


## Answer

Implemented in `ca92f03cc` (`Open Storage filesystem records through TanStack DB`) on `implement/tanstack-db-02`, in the assigned worktree. Before starting, HEAD and integration tip were both `92b20ce53`; the worktree was clean. TDD and both Svelte skills were invoked. Tests use the spec-approved public collection/live-query and app boundaries.

### Risk / complexity / priority

Assessed before implementation: moderate implementation complexity, high isolation/lifecycle risk, and high priority to restore the reference example. Eager loading avoids pagination/query-pushdown complexity. The primary risks are late authentication/socket/read/schema results leaking into replacement views, failed reads appearing empty, and unsupported synchronization assumptions. Terminal cleanup, cancellable socket opening/token exchange, all-or-error validation, keyed application lifecycle boundaries, and real-runtime transport tests address the read slice. Optimistic acknowledgement and stale-refresh races remain deliberately assigned to 03/04; no speculative write/reconnect machinery was introduced.

### Delivered

- Framework-independent `@aicacia/storage-db-collection`, using existing StorageClient with injected identity/configuration, Standard Schema validation, safe directory/key/filename matching, configurable extension/deserialization, and owned connection cleanup. No Svelte, Secret schema, environment, or OAuth Client ID is imported into the package.
- Installed and verified core `@tanstack/db 0.15.2` and official `@tanstack/svelte-db 0.5.12`. Real custom sync (`begin/write/commit/markReady/markError`), preload errors, cleanup, live-query ordering, and upstream `useLiveQuery` are exercised. No custom Svelte wrapper.
- Application sign-in/discovery/create/open, official favorite/name live query, distinct loading/empty/error states, read-only details, Filesystem ID selection, and User/IdP/filesystem/sign-out lifecycle isolation. Corrected `PUBLIC_IDP_BASE_URL`, IdP terminology, form submission/error handling, and missing Paraglide project settings following workspace conventions.
- Removed SecretStorage, its save/delete handlers, and obsolete composition helpers. New/Save/Delete controls are disabled; no legacy write bypass exists.
- Storage protocol verified from current service/offs source: listing entries contain full relative paths; listing a missing directory fails. Successful parent listings establish absence/empty without treating `operationFailed` as empty. StorageClient socket cancellation prevents work after disposal, closes connecting sockets, and releases opening listeners. Its already-tracked runtime/type outputs were rebuilt with the API change; environment-specific node_modules changes were not committed.
- Usage/API/failure/lifecycle documentation in `packages/storage-db-collection/README.md` and `apps/password-manager/README.md`, including reproducible checks and manual real-system qualification.

### Validation

Passed final focused validation:

- `pnpm --filter @aicacia/storage-client build:esm` and `exec vitest run`: 18 tests.
- `pnpm --filter @aicacia/storage-db-collection test`: 24 tests, real TanStack runtime plus real Storage client over controlled HTTP/WebSocket responses. Covers initial/empty load, sorting, malformed/partial snapshots, denied authorization, path/key isolation, disposal before/during load and token exchange/opening, disconnect during async validation, and rejection of writes.
- Adapter `check`, `build`, `lint`, and `format` scripts.
- `pnpm --filter password-manager test`: 12 tests, including mounted upstream-binding discovery/create/open/switch, late response, malformed/auth errors, User changes/sign-out, configured sign-in boundary, and existing explicit-prefix/token-exchange checks.
- `pnpm --filter password-manager exec node --test svelte.config.test.mjs`: 2 tests.
- App `build` (production static output), then `check`: zero errors/warnings.
- Focused Biome lint/format of all changed TypeScript/config sources, plus `git diff --check`.
- Svelte autofixer: no issues. Browser's advisory effect suggestions were reviewed; its effect owns asynchronous external discovery and invalidates obsolete contexts, rather than calculating a duplicate derived secrets array.

Limits: the existing broad app `lint` command scans generated `.svelte-kit`, build, and Paraglide output and fails on generated diagnostics; focused changed-source checks pass. `just crap` was attempted and fails in Cargo metadata because this nested worktree's relative sibling `ofdb/crates/sql/Cargo.toml` does not exist. It is Rust-only and does not measure TypeScript. No Rust code changed, so Rust feature-powerset tests were not used as TypeScript validation. A live Installation/OAuth server, server authorization, and cross-Node replication were not qualified; controlled tests are not evidence of those guarantees.

### Handoff

03 is unblocked. Add persistence through collection mutation handlers, preserving URI arrays/custom fields/history; do not restore SecretStorage. 04 owns explicit refresh/reconciliation and complete-workflow/live-system qualification. Current Open filesystem replaces the collection, not in-place refresh. There is no polling, automatic reconnect, browser secret persistence, offline outbox, or claim of end-to-end encryption. The current client still requests read/write socket authorization for the read-only view.
