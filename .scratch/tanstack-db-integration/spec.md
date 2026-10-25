# Password Manager: TanStack DB and Storage Integration

Status: ready-for-agent

## Problem Statement

The password-manager application is intended to be a complete, current example of using the system, but it has drifted from current configuration, terminology, and integration contracts. Its secrets list is manually loaded and rewritten in component state after each Storage operation. There is no reusable bridge between Storage and TanStack DB.

The User wants the frontend to use TanStack DB for synchronized application data and the example to demonstrate the current system accurately, rather than introduce another application-specific persistence layer.

## Solution

Deliver two integration layers using one new custom package and one existing upstream package:

- `@aicacia/storage-db-collection`: a reusable, framework-independent bridge between Storage and TanStack DB collections.
- `@tanstack/svelte-db`: the official Svelte bindings consumed directly by the password manager for reactive live queries and component lifecycle integration.

The password manager depends on both packages. Do not create a separate Aicacia Svelte wrapper package. Storage remains authoritative for persistence and resource authorization; TanStack DB maintains the reactive browser-side view and optimistic mutation state.

Preserve filesystem selection and secret CRUD while correcting stale configuration and terminology along the example's sign-in-to-storage path. Clearly distinguish server-side Node replication from browser collection refresh and browser-offline persistence.

## User Stories

1. As an example application developer, I want a working sign-in-to-storage workflow, so that I can learn the current system from executable code.
2. As an application developer, I want a separate Storage collection integration, so that other applications can reuse it without importing the password manager.
3. As a Svelte developer, I want the official TanStack DB Svelte bindings, so that collection lifecycle and reactive queries follow supported patterns.
4. As an application developer, I want typed collection records and validation, so that invalid data does not silently enter application state.
5. As an application developer, I want authentication and service configuration supplied by the application, so that the integration is independent of framework environment modules and route state.
6. As a User, I want to sign in against my configured IdP, so that the example uses my selected Installation.
7. As a User, I want to discover my Application's filesystem Storage Resources, so that I can choose where to store secrets.
8. As a User, I want to create a filesystem, so that I can start using the example without manually provisioning storage.
9. As a User, I want to open a filesystem by its stable Filesystem ID, so that duplicate or changed display names do not select the wrong resource.
10. As a User, I want secrets loaded into a reactive collection, so that the list reflects authoritative storage data.
11. As a User, I want clear initial-loading, empty, ready, and error states, so that a failed load is not mistaken for an empty filesystem.
12. As a User, I want to create a secret, so that it persists after reloading the application.
13. As a User, I want to edit a secret, so that changes persist without losing unrelated fields.
14. As a User, I want to delete a secret, so that it no longer appears after refreshing or reopening the filesystem.
15. As a User, I want optimistic changes with visible persistence failures, so that the UI feels responsive without claiming failed writes succeeded.
16. As a User, I want favorite-first and name ordering through a live query, so that sorting stays current when records change.
17. As a User, I want existing secret-change history preserved, so that editing a password does not discard earlier values.
18. As a User, I want supported refresh behavior to observe changes from another client, so that the collection is not permanently stale after its initial load.
19. As a User, I want to recover after a connection or authorization failure, so that I can resume work without silently dropping mutations.
20. As a User, I want switching filesystems to discard the previous filesystem's visible records and editor selection, so that I do not edit a secret in the wrong resource.
21. As a User, I want late responses from a previously selected filesystem ignored, so that rapid switching cannot leak records into the new selection.
22. As a User, I want secret data and storage connections cleared when the authenticated context ends, so that one User's data does not remain visible to the next.
23. As an application developer, I want documented synchronization and failure semantics, so that I do not assume a WebSocket provides a change feed or durable offline writes.
24. As a maintainer, I want reproducible public-behavior tests and example setup instructions, so that the integration remains a trustworthy reference.

## Implementation Decisions

- Add the JavaScript/TypeScript workspace package `@aicacia/storage-db-collection` alongside the existing client packages. It owns record loading, persistence handlers, refresh/reconciliation, and Storage connection lifecycle through the existing Storage client. Use TanStack DB core APIs without a Svelte dependency. Package publication to an external registry is not required.
- Implement the supported TanStack DB custom collection/options integration for the selected compatible release. Verify the installed release's types before implementation. Do not implement the previously proposed, unverified `Database.query`/`Database.mutate` API or substitute TanStack Query observer types for DB types.
- Keep the integration independent of Svelte, password-manager schemas, application routes, environment imports, and a fixed OAuth Client ID, so the same adapter can be consumed by Svelte, React, or vanilla TypeScript applications. The password manager directly depends on `@aicacia/storage-db-collection` and the official `@tanstack/svelte-db` package rather than a parallel custom Svelte store abstraction.
- Keep app-specific composition, authenticated resource selection, and form state in the password manager. Do not create an Aicacia-specific Svelte package now. Consider extracting one only if multiple applications demonstrate substantial shared Svelte behavior not already provided by the official bindings.
- Use the existing Storage client boundary for token exchange, resource discovery, creation, and socket opening. Supply service bases and bearer-token access through application composition. Preserve explicit IdP and Storage service prefixes; do not infer Storage from the IdP origin.
- Implement the filesystem-backed record collection required by the example first. Do not introduce a SQL backend migration solely to adopt TanStack DB.
- Accept the record schema, stable record key, and directory/serialization configuration at the integration boundary. Keep Secret-specific fields, history rules, and form editing in the application. Validate keys and derived paths so record identities cannot escape the configured directory.
- Preserve the existing one-JSON-file-per-secret representation and Secret validation unless verified current Storage contracts require a change. Preserve fields not edited by the current form, including additional URIs and custom fields.
- Scope collection identity and lifecycle to the authenticated User, Application, and selected Filesystem ID. Never use a display name as resource identity. Dispose obsolete collections, subscriptions, timers, and owned sockets; ignore late async results after disposal.
- Replace the app's manually maintained secrets array with collection-backed live queries. Route inserts, updates, and deletes through collection mutation handlers. Keep temporary form fields and other presentation state local; TanStack DB need not own every UI flag.
- Treat Storage acknowledgements and authoritative collection reconciliation as distinct steps. Settle optimistic mutations only according to the selected TanStack DB release's persistence/sync contract; expose failures and reconcile ambiguous outcomes before encouraging a retry.
- Do not imply atomic multi-record Storage writes: the current client exposes individual filesystem operations. Support the example's single-record mutations; any broader batching must document partial failure behavior rather than claim server transactions.
- The inspected Storage socket has read/write/delete/list requests and responses, but no exposed change subscription, replay cursor, or compare-and-swap contract. Use initial loading, post-write reconciliation, and an explicit Refresh action. Refresh can reopen a failed connection before reloading authoritative data. Do not implement polling, automatic reconnect, or a new server change-feed protocol.
- Refresh must reconcile additions, updates, and deletions without overwriting pending optimistic edits with a stale snapshot. Failed or malformed reads must surface an error, not silently delete valid records or present a complete empty snapshot.
- Audit and correct stale configuration and terminology along this example's workflow, including the discrepancy between `PUBLIC_IDP_BASE_URL` and the legacy `PUBLIC_LIDP_BASE_URL`. Use IdP, Application, Storage Resource, and Filesystem ID consistently with the domain glossary. Remove obsolete example persistence code rather than retain compatibility wrappers.
- Preserve the existing OAuth flow and Storage-Audience Access Token authorization model. A token authenticates a Principal within a User/Application namespace; discovery does not grant resource access. Do not expose tokens or secret contents in logs or diagnostic messages.
- Do not persist decrypted secrets or pending secret mutations into browser localStorage, IndexedDB, or an offline outbox as part of this work.
- Before implementation, assess complexity, risk, and priority: the priority is restoring a reference example; main risks are data isolation, lifecycle races, stale refreshes, optimistic acknowledgement, and unsupported sync assumptions. Prefer eager loading for this small example over pagination or query pushdown.

## Testing Decisions

- Primary proposed seam: exercise the integration's public TanStack DB collection and live-query behavior against a controlled Storage endpoint using the existing client boundary. Prefer deterministic transport responses and bounded async waits over spying on internal methods. Use the real TanStack DB runtime so tests detect unsupported API assumptions.
- The User approved three vertical slices: open/read with lifecycle isolation, persistent CRUD with mutation isolation, then explicit refresh and complete-workflow qualification. Include focused tests and relevant documentation within each slice rather than creating separate hardening phases.
- Cover initial load, empty resources, validated round trips, insert/update/delete, ordering, preservation of unedited fields, mutation rejection and optimistic rollback, refresh additions/updates/deletions, malformed content, and disconnect/recovery.
- Cover late snapshot versus pending mutation races, rapid Filesystem ID switching, disposal during load/write, authentication-context changes, and teardown. Assert no stale records appear and no new work is initiated by disposed resources.
- Verify namespace/path isolation, rejection of unsafe keys, explicit service prefixes, and denied or expired authorization. Reuse existing Storage client token-exchange/service-base tests rather than duplicating their internals throughout the adapter tests.
- Prior art includes the Storage client's request/response tests and the password manager's Storage configuration tests. The inspected application has no established SecretStorage test suite to rely on.
- Add a focused application smoke test for sign-in boundary, filesystem discovery/creation/opening, secret CRUD, reload persistence, and switching. Use a controlled service fixture for repeatability; document a real-system manual qualification path without treating mocks as proof of cross-Node replication.
- Run package tests, TypeScript checks, app Svelte checks, applicable lint/format checks, and a production build using the repository's actual scripts. Run project complexity validation where available. If Rust changes become necessary, also run the required Rust feature-powerset validation; Cargo commands do not validate a TypeScript-only adapter.

## Out of Scope

- Polling, automatic reconnect, and background browser synchronization; add these only when a demonstrated need justifies them.
- Creating or maintaining a custom Aicacia Svelte wrapper package or framework binding library.
- Replacing Storage with ElectricSQL, PowerSync, TrailBase, RxDB, or another backend.
- New OAuth flows, changes to domain resource ownership, or cross-User sharing.
- New Storage database protocols, server subscriptions, resumable change streams, or distributed conflict-resolution guarantees.
- Durable browser-offline writes, automatic cross-tab synchronization, and browser-side encrypted vault design.
- New password-manager features such as pagination, search, password generation, history browsing, or a broad UI redesign. Preserve current behavior; do not turn earlier illustrative suggestions into requirements.
- Rewriting unrelated example applications or requiring npm registry publication.

## Further Notes

- This spec supersedes the earlier draft's invented Database interface, Rust-style npm feature flags, application imports inside a reusable package, and claims that WebSockets automatically provide offline or cross-client synchronization.
- TanStack DB collections are reactive browser views, not a replacement for Storage's resource ownership and persistence. Node replication and browser collection freshness are separate concerns.
- Reference documentation reviewed: https://tanstack.com/db/latest/docs/installation and https://tanstack.com/db/latest/docs/overview.md. Check the chosen release's official collection-options and Svelte APIs during implementation; no package version was established in this conversation.
- Browser access to secret contents does not by itself establish end-to-end encryption or production password-vault security. Document the example's actual security properties without implying guarantees not demonstrated by the system.
