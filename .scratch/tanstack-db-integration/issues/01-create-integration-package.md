# Restore the password-manager example with a TanStack DB Storage integration

Status: ready-for-agent
Type: task

## Goal

Implement the [feature spec](../spec.md): the framework-independent `@aicacia/storage-db-collection` package and a password-manager migration consuming it alongside the official `@tanstack/svelte-db` bindings. These are two layers, not two new custom packages.

## Work

1. Verify compatible TanStack DB core/Svelte APIs and existing workspace build/test conventions.
2. Implement a framework-independent filesystem record collection package using the existing Storage client boundary, explicit configuration, validated records, and safe keys/paths.
3. Implement initial load, single-record mutations, authoritative refresh/reconciliation, visible failure handling, and deterministic disposal. Document request/response refresh limitations; do not invent an available server subscription.
4. Migrate the example to live queries and collection mutations; delete obsolete persistence code, preserve existing Secret fields/history, and fix stale IdP configuration and terminology in the workflow.
5. Test public collection behavior and an application smoke path. Document setup, synchronization limits, security boundaries, and real-system qualification steps.

## Acceptance Criteria

- The app discovers, creates, and opens filesystem Storage Resources using current service/auth contracts.
- Secrets are read through TanStack DB live queries and persisted through collection mutation handlers, with reload persistence and clear failures.
- The collection observes additions, updates, and deletions on supported refresh without corrupting pending optimistic state.
- Failed writes, malformed records, switching, late async responses, and teardown are covered by tests.
- `@aicacia/storage-db-collection` has no Svelte dependency or password-manager, SvelteKit environment, or app authentication-state imports.
- The password manager depends directly on `@aicacia/storage-db-collection` and `@tanstack/svelte-db`; app-specific composition remains in the app, with no new Aicacia Svelte wrapper package.
- No unverified real-time, offline-browser, atomic-batch, or encryption guarantees are claimed.
- Applicable tests, type/Svelte checks, lint/format checks, and production build run; any unavailable checks or existing failures are reported.

## Testing Boundary

Use the real TanStack DB public collection/live-query API with controlled Storage responses as the primary seam, plus a focused application smoke test. This boundary is proposed in the spec, not yet User-confirmed.

## Comments

The earlier sample implementation was illustrative and incorrect. Follow the corrected spec and verified library types, not its Database class, observer types, npm feature flags, or storage save/list method assumptions.
