# 01: Make the secret editor consume one workflow interface

**What to build:** Preserve the password-manager's existing behavior while giving the secret editor one simple, app-local workflow interface. The view renders state, owns input bindings, and invokes operations; it no longer coordinates collection lifecycle, transaction settlement, recovery, or late responses. Concentrate those rules in one concrete module rather than adding pass-through wrappers.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

## Acceptance criteria

- [ ] Introduce one app-local Svelte workflow module exposing reactive state and concrete edit, cancel, save, delete, refresh, and dispose operations. No generic controller framework, new dependency, or parallel records array is introduced.
- [ ] Keep input bindings local to the view and pass draft values to Save. Expose only the state the view needs to render records, editor selection, notices, errors, and permitted actions; the caller does not need transaction promises, acknowledgement exception classes, socket-health rules, or late-response guards.
- [ ] Keep secret construction and preservation rules in a pure function within the workflow module. Validate required fields; preserve additional URIs, the first URI's match mode, custom fields, unchanged notes, creation/deletion timestamps, and existing history. Changing the secret appends its previous value with a change timestamp; other edits do not. Clearing Website removes only the first URI, and Save updates the modification timestamp.
- [ ] Reuse the existing SDK-provided authenticated Storage capability, collection adapter, official live-query binding, and keyed teardown. Keep the SDK independent of the Secret schema and collection adapter, and the adapter independent of Svelte and the SDK.
- [ ] Preserve initial loading, empty, ready, malformed-record, and authorization-failure behavior. Failed or partially malformed loads and refreshes never publish an empty or incomplete list. Ordering remains favorite-first, then name, through the live query.
- [ ] Preserve optimistic creation, updates, and deletion, disabling conflicting controls until persistence and authoritative reconciliation settle. Report success only after settlement. Rejected writes reconcile records and preserve editable drafts where appropriate; uncertain writes never report success or permit blind duplicate creation. Failed reconciliation hides untrusted records and directs the User to explicitly Refresh and inspect before retrying. Do not add automatic write retries.
- [ ] Preserve manual Refresh: reload external additions, updates, and deletions; reuse a healthy connection; recover a failed connection only on explicit request. Successful Refresh closes stale unsaved edits. No polling, automatic reconnect, or background synchronization is added.
- [ ] Preserve immediate removal of old records and drafts on Filesystem ID switching, reopening, route teardown, sign-out, or authenticated-context retirement. Late results and notices cannot populate a replacement view; same-User token renewal preserves the current collection and editor. Already-sent writes may still persist remotely.
- [ ] Move the replaced orchestration out of the view and delete obsolete state and cleanup paths. Do not retain compatibility wrappers or duplicate ownership. Reading the view should not require knowledge of persistence or transport implementation details.
- [ ] Test observable workflow behavior through the new public interface using the real collection adapter and controlled Storage transport, reusing existing test infrastructure. Cover CRUD, preservation/validation, ordering, Refresh, rejection, uncertain outcomes, failed reconciliation, and disposal during pending work. Do not assert internal flags, queues, or private helper structure.
- [ ] Retain focused mounted-app coverage for UI wiring, authenticated-context isolation, immediate record/draft removal, and same-User renewal preservation. Move applicable workflow assertions rather than duplicating the full suite at both seams.
- [ ] Run relevant existing SDK, collection-adapter, and app regression checks, app type checking, and production build. Report any unavailable checks and distinguish controlled-transport validation from live Installation qualification.

## Scope limits

No authentication redesign, new routes, search, copy/reveal/generate controls, schema changes, encryption, offline persistence, provider-logout changes, or unrelated cleanup. Do not redesign SDK scoping or move generic behavior between packages without a demonstrated need. Existing authentication, authorization, validation, and data-loss protections must remain intact throughout the refactor.

## Design check

The success criterion is a smaller caller-facing interface, not fewer internal states or merely relocating code. The workflow module owns the interaction between loading, editing, persistence, recovery, and disposal; the view must not reconstruct that policy. Keep distinct state dimensions distinct rather than forcing them into one oversized status enum. A concrete module is sufficient; no additional interchangeable adapter seam is required.
