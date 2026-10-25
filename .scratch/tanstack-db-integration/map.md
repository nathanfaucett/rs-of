# TanStack DB Integration — Map

## Notes

- [Specification](spec.md) — Status: ready-for-agent.
- [Existing umbrella ticket 01](issues/01-create-integration-package.md) — unchanged; not an additional prerequisite for the slices below.
- [02: Open a filesystem through TanStack DB](issues/02-open-filesystem-through-tanstack-db.md) — ready-for-agent; no blockers; current frontier.
- [03: Save and delete secrets through the collection](issues/03-persist-secret-collection-mutations.md) — ready-for-agent; blocked by 02.
- [04: Refresh and qualify the example](issues/04-refresh-and-qualify-example.md) — ready-for-agent; blocked by 03.
- Numbering starts at 02 to preserve the existing umbrella ticket 01 unchanged.
- The goal is a current password-manager reference app and a reusable Storage integration, not implementation of the earlier unverified code sample.

## Decisions-so-far

- Two layers, one new custom package: `@aicacia/storage-db-collection` bridges Storage to framework-independent TanStack DB collections; the app consumes it alongside the existing official `@tanstack/svelte-db` package.
- Do not create an Aicacia Svelte wrapper package. Keep app-specific composition in the password manager; extract shared Svelte utilities only when multiple apps demonstrate a need beyond the official bindings.
- Reuse the existing Storage client for authentication and filesystem operations; inject configuration and keep Secret-specific behavior in the application.
- Use filesystem JSON records, post-write reconciliation, and explicit Refresh. Refresh can reopen a failed connection; skip polling and automatic reconnect. The inspected socket does not expose a change subscription.
- Preserve current example behavior, correct stale configuration/terminology, and remove obsolete persistence code.
- The User approved three vertical slices: read/open including lifecycle isolation; persistent CRUD including mutation isolation; then explicit refresh and workflow qualification. Each includes its relevant tests and documentation.
- Primary test seam: the public collection/live-query API backed by controlled Storage responses, plus a focused app smoke test.

## Fog

- Select compatible TanStack DB core/Svelte releases and verify exact collection/mutation lifecycle APIs before coding.
- Verify empty-directory behavior, reconnect behavior, and error contracts against the composed Storage service during implementation.
- Background synchronization is deferred until a demonstrated need; do not imply push synchronization or durable browser-offline writes.
- Real-system qualification is needed to establish behavior beyond the controlled transport tests.
