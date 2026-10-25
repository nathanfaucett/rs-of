# 04: Refresh and qualify the example

**What to build:** A User can explicitly refresh to observe another client's changes and retry loading after a failed connection. A developer can follow documented setup instructions and verify the complete current-system workflow.

**Blocked by:** 03 — Save and delete secrets through the collection.

**Triage:** ready-for-agent

Status: claimed

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope.

## Acceptance criteria

- [ ] Expose an explicit Refresh action that reconciles authoritative additions, updates, and deletions; reopen a failed connection when needed rather than implementing automatic reconnect.
- [ ] Preserve pending optimistic edits against stale snapshots. Prefer simple serialization or disabling conflicting actions when sufficient; do not build a generalized synchronization scheduler.
- [ ] Failed, partial, or malformed reads surface an error and do not silently replace valid records with an empty or incomplete snapshot.
- [ ] Test external changes, pending-mutation versus refresh ordering, connection failure followed by manual retry, and disposal during refresh using the public collection boundary.
- [ ] Exercise the full sign-in boundary, filesystem discovery/creation/opening, CRUD, reload persistence, explicit refresh, and context-switching workflow with a focused app smoke test and controlled service responses.
- [ ] Document setup, package responsibilities, refresh/failure semantics, and a real-system manual qualification path. Distinguish Node replication from browser freshness, and do not claim unverified encryption, push synchronization, cross-tab synchronization, or browser-offline writes.
- [ ] Run package tests, TypeScript/Svelte checks, applicable lint/format checks, production build, and available project complexity validation. Report unavailable checks and pre-existing failures; run required Rust validation only if Rust code changes.
- [ ] No polling, automatic reconnect, custom Svelte wrapper, server change feed, or unrelated password-manager feature is added.
