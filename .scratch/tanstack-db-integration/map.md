# TanStack DB Integration — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- [Existing umbrella ticket 01](issues/01-create-integration-package.md) — unchanged; not an additional prerequisite for the slices below.
- [02: Open a filesystem through TanStack DB](issues/02-open-filesystem-through-tanstack-db.md) — resolved; read/open slice delivered in `ca92f03cc`.
- [03: Save and delete secrets through the collection](issues/03-persist-secret-collection-mutations.md) — ready-for-agent; 02 resolved; unblocked current frontier.
- [04: Refresh and qualify the example](issues/04-refresh-and-qualify-example.md) — ready-for-agent; blocked by 03.
- Numbering starts at 02 to preserve the existing umbrella ticket 01 unchanged.
- The goal is a current password-manager reference app and a reusable Storage integration, not implementation of the earlier unverified code sample.

## Decisions-so-far

- [02 resolved](issues/02-open-filesystem-through-tanstack-db.md#answer): `ca92f03cc` adds the reusable eager read collection, official Svelte live-query app wiring, safe filesystem/auth lifecycle, disabled legacy writes, cancellation, real-runtime transport/mounted-app tests, and read-workflow docs. Qualified core `0.15.2` / Svelte `0.5.12`. 03 can proceed with mutations; 04 retains refresh/full-workflow qualification.
- Storage listings use full relative paths; parent listings distinguish an absent record directory from operation failure. Disposal is terminal, including pending bearer-token exchange/socket opening/validation; construct a new collection to reopen.

- Two layers, one new custom package: `@aicacia/storage-db-collection` bridges Storage to framework-independent TanStack DB collections; the app consumes it alongside the existing official `@tanstack/svelte-db` package.
- Do not create an Aicacia Svelte wrapper package. Keep app-specific composition in the password manager; extract shared Svelte utilities only when multiple apps demonstrate a need beyond the official bindings.
- Reuse the existing Storage client for authentication and filesystem operations; inject configuration and keep Secret-specific behavior in the application.
- Use filesystem JSON records, post-write reconciliation, and explicit Refresh. Refresh can reopen a failed connection; skip polling and automatic reconnect. The inspected socket does not expose a change subscription.
- Preserve current example behavior, correct stale configuration/terminology, and remove obsolete persistence code.
- The User approved three vertical slices: read/open including lifecycle isolation; persistent CRUD including mutation isolation; then explicit refresh and workflow qualification. Each includes its relevant tests and documentation.
- Primary test seam: the public collection/live-query API backed by controlled Storage responses, plus a focused app smoke test.

## Fog

- Initial-load/cleanup/official Svelte APIs verified against installed core `0.15.2` and Svelte `0.5.12`; 03 must verify persistence/optimistic settlement semantics against that release.
- Empty-directory/full-path protocol verified in current service/offs source and controlled transport tests. Real composed-service authorization/content availability and manual reconnect/reopen qualification remain; no automatic reconnect is implemented.
- Background synchronization is deferred until a demonstrated need; do not imply push synchronization or durable browser-offline writes.
- Real-system qualification is needed to establish behavior beyond the controlled transport tests.
