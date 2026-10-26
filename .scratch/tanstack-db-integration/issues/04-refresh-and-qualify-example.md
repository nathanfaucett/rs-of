# 04: Refresh and qualify the example

**What to build:** A User can explicitly refresh to observe another client's changes and retry loading after a failed connection. A developer can follow documented setup instructions and verify the complete current-system workflow.

**Blocked by:** 03 — Save and delete secrets through the collection.

**Triage:** ready-for-agent

Status: resolved

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope.

## Acceptance criteria

- [x] Expose an explicit Refresh action that reconciles authoritative additions, updates, and deletions; reopen a failed connection when needed rather than implementing automatic reconnect.
- [x] Preserve pending optimistic edits against stale snapshots. Prefer simple serialization or disabling conflicting actions when sufficient; do not build a generalized synchronization scheduler.
- [x] Failed, partial, or malformed reads surface an error and do not silently replace valid records with an empty or incomplete snapshot.
- [x] Test external changes, pending-mutation versus refresh ordering, connection failure followed by manual retry, and disposal during refresh using the public collection boundary.
- [x] Exercise the full sign-in boundary, filesystem discovery/creation/opening, CRUD, reload persistence, explicit refresh, and context-switching workflow with a focused app smoke test and controlled service responses.
- [x] Document setup, package responsibilities, refresh/failure semantics, and a real-system manual qualification path. Distinguish Node replication from browser freshness, and do not claim unverified encryption, push synchronization, cross-tab synchronization, or browser-offline writes.
- [x] Run package tests, TypeScript/Svelte checks, applicable lint/format checks, production build, and available project complexity validation. Report unavailable checks and pre-existing failures; run required Rust validation only if Rust code changes.
- [x] No polling, automatic reconnect, custom Svelte wrapper, server change feed, or unrelated password-manager feature is added.

## Answer

Implemented in `3e4c35f65` on `implement/tanstack-db-04`, only in the assigned worktree. Before changes, clean HEAD exactly matched integration `1aca2c4619965bd8a574a2e71d16c3f9108ee48f`; confirmed ancestry. Explicitly merged `integration/tanstack-db-integration` before finishing: already up to date at that same tip. Main checkout and other branches/worktrees were not changed. Invoked TDD, ponytail, and both Svelte skills; used the spec-approved public collection/live-query and mounted-app seams.

### Risk / complexity / priority

Assessed before implementation: high priority to complete the reference app; moderate complexity, with critical risks around stale snapshots, optimistic settlement, disposal/auth isolation, and indefinitely pending transport. Reused the existing operation queue and eager all-or-error snapshot path rather than adding a scheduler. The no-deadline limitation was relevant: a silent write could indefinitely disable Refresh. Adapter-local deadlines address it without changing Rust or unrelated StorageClient behavior.

### Delivered

- `collection.utils.refresh()` accepts complete authoritative additions/updates/deletions, reuses a healthy socket, and reopens a failed socket only on explicit request, obtaining fresh authorization through the supplied client. Idle Refresh loads once; disposed collections cannot revive.
- Refresh and writes share the operation queue. Earlier writes include authoritative reconciliation and core settlement. A later optimistic transaction remains visible while the refresh snapshot is accepted. Release the queue before awaiting its visibility receipt, otherwise a later persisting mutation deadlocks waiting for its own handler. Real core `0.15.2` tests cover both operation orderings. Receipt rejection fails closed.
- Failed/partial/malformed refresh leaves the last complete snapshot internally intact, enters error, and closes transport; app consumers hide error-state data. Explicit retry recovers the same collection and official live query. Disposal during refresh and late responses cannot populate replacement contexts.
- Positive, timer-safe `requestTimeoutMs` (default 30 seconds) bounds socket opening/token retrieval/exchange and each Storage request. Timeout aborts opening or immediately fails/closes the current connection, without waiting for a delayed browser close event. A sent write is uncertain, never automatically retried. Deadlines are cleared on settlement/disposal; no polling or automatic reconnect. Application-provided async schema validation and total multi-record snapshot duration are not globally bounded.
- `Filesystem.svelte` exposes Refresh in ready/error states, disables Refresh during persistence and write/editor controls during refresh, and closes stale unsaved form edits on successful refresh. Recovery clears the old error without replacing the collection; failed refresh does not claim success or empty data. Context teardown still clears all old records/forms/notices.
- Focused controlled full-workflow smoke: configured sign-in submission boundary, filesystem discovery/create/open, CRUD/history, fresh app mount reading persisted data, explicit Refresh, switch during refresh, authenticated User change, and sign-out. Also mounted malformed-refresh/manual reconnect and pending-write control checks. Real TanStack runtime, official Svelte binding, and existing StorageClient are not mocked; HTTP/WebSocket and the OIDC/authenticated Page boundary are controlled fixtures.
- Both READMEs now document setup/callback/service prefixes, package responsibilities, security/credential handling, refresh/failure/deadline semantics, and an explicit real-system checklist for OAuth, authorization, CRUD/reload, external changes, failure/recovery, switching, and separate Node replication qualification. No encryption, change feed, automatic cross-tab freshness, browser secret persistence, or offline outbox claims.

### Validation

Passed in the assigned worktree:

- Dependency setup: frozen pnpm install; required IdP/native-fetch/OIDC/Storage ESM builds and svelte-headless prepack.
- `pnpm --filter @aicacia/storage-db-collection test`: **59 passed**, including controlled real-runtime refresh/order/error/recovery/disposal/deadline tests.
- Adapter `check`, `build`, `lint`, and `format`: passed.
- `pnpm --filter @aicacia/storage-client exec vitest run`: **18 passed**.
- `pnpm --filter password-manager test`: **21 passed**, including **14 mounted browser tests** and the full-workflow smoke.
- App `build`: production static output passed. App `check`: **0 errors, 0 warnings**. Svelte config tests: **2 passed**.
- Focused Biome lint of changed app TypeScript and format of changed TypeScript/Svelte sources: passed. Svelte autofixer: no issues or suggestions. `git diff --check`: passed.
- Observed red before green for missing Refresh, absent UI action, silent open/read/write deadlines, and failure before a delayed close event. Updated prior failure assertions to require the new explicit Refresh recovery/uncertain-write message.

Initial clean-checkout Svelte check failed because Paraglide runtime had not yet been generated; production build generated it and repeated check passed. Expected upstream live-query diagnostics occur in deliberately malformed/disconnected cases. No remaining focused check failure.

As assigned, `just crap` remains coordinator-owned in the main checkout due nested-worktree sibling Rust paths; it was not run here and does not measure TypeScript. No Rust changed, so Cargo feature-powerset tests were not applicable. No live Installation/credentials were available: real OAuth redirects/callback, actual browser reload/navigation, server authorization, content availability and cross-Node replication remain manual qualification, not inferred from fixtures. Reload smoke uses a fresh app mount and new collections, not browser automation. Sequential reads are complete-or-error client snapshots, not server-atomic snapshots or conflict prevention.

### Worktree cleanup handoff

Only explicit source/docs files were committed. `pnpm install` rewrote six tracked `packages/storage-client/node_modules/.bin/` shims (`biome`, `rolldown`, `shx`, `tsc`, `tsserver`, `vitest`); Storage-client tests also rewrote tracked `packages/storage-client/node_modules/.vite/vitest/da39a3ee5e6b4b0d3255bfef95601890afd80709/results.json`. These seven generated changes remain uncommitted for coordinator-owned safe worktree cleanup; do not include them in integration. No other user changes were reverted or removed.
