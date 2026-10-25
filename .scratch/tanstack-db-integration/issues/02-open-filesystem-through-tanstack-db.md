# 02: Open a filesystem through TanStack DB

**What to build:** A User can sign in, discover or create a filesystem Storage Resource, and view its secrets through a TanStack DB live query. Switching filesystems or ending the authenticated context clears obsolete records and editor selection safely.

**Blocked by:** None (can start immediately).

**Triage:** ready-for-agent

Status: claimed

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope; the umbrella ticket is not an additional implementation prerequisite.

## Acceptance criteria

- [ ] Verify compatible TanStack DB core and official Svelte APIs; add the framework-independent `@aicacia/storage-db-collection` workspace package and consume it alongside `@tanstack/svelte-db` in the password manager.
- [ ] The reusable package uses the existing Storage client boundary, injected configuration, validated records, and safe keys/paths; it has no Svelte, app-schema, environment-module, or fixed OAuth Client ID dependency.
- [ ] Sign-in, filesystem discovery/creation/opening, and initial secret loading work with explicit service bases. Correct stale IdP configuration and domain terminology along this workflow.
- [ ] Display loading, empty, ready, malformed-record, and authorization-error states accurately; query the secrets collection with favorite-first/name ordering instead of maintaining a duplicate list.
- [ ] Switching Filesystem ID or authenticated context clears old data and editor selection, disposes obsolete collections and owned connections, and ignores late results. Basic resource cleanup is part of this slice, not deferred hardening.
- [ ] Use the real TanStack DB public API with controlled Storage responses to test initial load, empty/error states, unsafe keys, explicit service prefixes, rapid switching, authentication changes, and disposal during load. Run relevant package and app checks.
- [ ] Keep this slice usable without bypassing the collection for writes: until ticket 03 provides collection mutations, do not expose active legacy write controls against the collection-backed view.
- [ ] Document the read workflow and its current limitations. No custom Svelte wrapper, polling, or automatic reconnect is introduced.
