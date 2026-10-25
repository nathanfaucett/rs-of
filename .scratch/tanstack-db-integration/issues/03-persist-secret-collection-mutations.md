# 03: Save and delete secrets through the collection

**What to build:** A User can create, edit, and delete secrets through the collection-backed password manager, with responsive optimistic feedback, visible failures, and persistence after reopening the filesystem.

**Blocked by:** 02 — Open a filesystem through TanStack DB.

**Status:** ready-for-agent

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope.

## Acceptance criteria

- [ ] Route the app's insert, update, and delete operations through collection mutation handlers backed by existing Storage operations, without a separately maintained secrets array.
- [ ] Validate outgoing records and preserve unedited fields, additional URIs, custom fields, timestamps, and existing secret-change history according to current application behavior.
- [ ] Follow the verified TanStack DB persistence/sync contract: reconcile authoritative state before settling optimistic changes, surface rejected writes, and handle ambiguous outcomes without falsely reporting success or encouraging blind retries.
- [ ] Switching filesystem or authenticated context during a mutation cannot populate the new collection, edit its records, or show a stale completion as its success. Do not promise cancellation of a write already sent to Storage.
- [ ] Test create/update/delete, reload persistence, validation, preservation of unedited fields/history, rejection and rollback, ambiguous outcomes, and in-flight mutation disposal/context switching through the public collection boundary.
- [ ] Delete the obsolete application-specific persistence implementation once collection mutations replace its callers. Keep form-only state in the application.
- [ ] Run focused tests and relevant type/Svelte checks; update example documentation for CRUD and failure behavior. Do not claim atomic multi-record operations or persist secrets in a browser outbox.
