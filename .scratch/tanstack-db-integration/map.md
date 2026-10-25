# TanStack DB Integration — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- [Existing umbrella ticket 01](issues/01-create-integration-package.md) — unchanged; not an additional prerequisite for the slices below.
- [02: Open a filesystem through TanStack DB](issues/02-open-filesystem-through-tanstack-db.md) — resolved; read/open slice delivered in `ca92f03cc`.
- [03: Save and delete secrets through the collection](issues/03-persist-secret-collection-mutations.md) — resolved; persistent CRUD delivered in `ad1c4f32a`.
- [04: Refresh and qualify the example](issues/04-refresh-and-qualify-example.md) — ready-for-agent; 03 resolved; unblocked current frontier.
- Numbering starts at 02 to preserve the existing umbrella ticket 01 unchanged.
- The goal is a current password-manager reference app and a reusable Storage integration, not implementation of the earlier unverified code sample.

## Decisions-so-far

- [03 resolved](issues/03-persist-secret-collection-mutations.md#answer): `ad1c4f32a` adds validated single-record collection mutations, authoritative reconciliation before optimistic settlement, explicit rejection/ambiguity handling, scoped write disposal, form-only CRUD with field/history preservation, and updated documentation. Qualified installed core `0.15.2` settlement behavior with real-runtime controlled Storage tests: 41 adapter tests, 18 app tests, 2 Svelte config tests, type/Svelte checks, build, focused lint/format passed. Based on and explicitly merged integration `f9350db82` (already up to date). 04 is unblocked; main-checkout `just crap` remains coordinator-owned.
- Mutation sync commits are accepted before handler settlement; visibility receipts may wait for persisting optimism and must not be awaited inside handlers. Missing/unexpected/wrong-path acknowledgements reconcile but still reject, and unreconciled outcomes fail closed. Switching cannot cancel already-sent writes or surface their results in replacement contexts.

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

- Initial-load/cleanup/official Svelte APIs and mutation/optimistic settlement verified against installed core `0.15.2` and Svelte `0.5.12`; explicit Refresh/recovery races and complete-workflow qualification remain 04.
- Current transport has no request deadline: a silent still-connected endpoint can leave a mutation pending until response/disconnect/disposal. No automatic write retry, compare-and-swap, or multi-record server atomicity is promised.
- Empty-directory/full-path protocol verified in current service/offs source and controlled transport tests. Real composed-service authorization/content availability and manual reconnect/reopen qualification remain; no automatic reconnect is implemented.
- Background synchronization is deferred until a demonstrated need; do not imply push synchronization or durable browser-offline writes.
- Real-system qualification is needed to establish behavior beyond the controlled transport tests.
