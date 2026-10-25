# 03: Save and delete secrets through the collection

**What to build:** A User can create, edit, and delete secrets through the collection-backed password manager, with responsive optimistic feedback, visible failures, and persistence after reopening the filesystem.

**Blocked by:** 02 — Open a filesystem through TanStack DB.

**Triage:** ready-for-agent

Status: resolved

## Parent

Umbrella ticket 01: Restore the password-manager example with a TanStack DB Storage integration. Follow the feature spec and its approved reduced scope.

## Acceptance criteria

- [x] Route the app's insert, update, and delete operations through collection mutation handlers backed by existing Storage operations, without a separately maintained secrets array.
- [x] Validate outgoing records and preserve unedited fields, additional URIs, custom fields, timestamps, and existing secret-change history according to current application behavior.
- [x] Follow the verified TanStack DB persistence/sync contract: reconcile authoritative state before settling optimistic changes, surface rejected writes, and handle ambiguous outcomes without falsely reporting success or encouraging blind retries.
- [x] Switching filesystem or authenticated context during a mutation cannot populate the new collection, edit its records, or show a stale completion as its success. Do not promise cancellation of a write already sent to Storage.
- [x] Test create/update/delete, reload persistence, validation, preservation of unedited fields/history, rejection and rollback, ambiguous outcomes, and in-flight mutation disposal/context switching through the public collection boundary.
- [x] Delete the obsolete application-specific persistence implementation once collection mutations replace its callers. Keep form-only state in the application.
- [x] Run focused tests and relevant type/Svelte checks; update example documentation for CRUD and failure behavior. Do not claim atomic multi-record operations or persist secrets in a browser outbox.

## Answer

Implemented in `ad1c4f32a` on `implement/tanstack-db-03`, based on integration tip `f9350db82`. Explicitly merged `integration/tanstack-db-integration` before resolution: already up to date. Main checkout and other worktrees were not changed.

### Delivered

- `packages/storage-db-collection/src/index.ts`: typed `onInsert`/`onUpdate`/`onDelete`, outgoing schema/key validation and serialization, single-record Storage operations, full authoritative post-write reconciliation, and serialized mutation/core settlement. Multi-record transactions reject before transport. Explicitly rolled-back queued transactions cannot send writes.
- Verified the installed core `0.15.2` types **and runtime**: `commit()` accepts authoritative sync before handler settlement but may defer visibility behind persisting optimistic transactions. The handler must not await that visibility receipt (deadlock); core publishes accepted state as optimism settles. Regression tests assert normalized authoritative results, rejection/rollback, and pending-optimistic isolation via public APIs.
- Expected acknowledgements settle only after reconciliation. Wrong-path/unexpected/lost acknowledgements reconcile but reject with `UnacknowledgedStorageWriteError`; disconnect or failed reconciliation closes/fails the collection and warns that persistence may have occurred. No automatic write retries or success on ambiguity.
- `Filesystem.svelte`: collection-backed CRUD with optimistic list feedback, pending controls, visible failures, and local form-only state. Preserves additional URIs, match modes, custom fields, unedited notes (including empty strings), creation/deletion timestamps, and prior history; changes to the secret append history and saves update `updatedAt`. Ambiguous saves/deletes retire affected drafts instead of encouraging a blind retry.
- Disposal during outgoing validation, sent writes, or queued writes cannot publish into a replacement collection. Mounted tests cover filesystem switching, User changes, and sign-out during writes without stale success/errors. Already-sent writes are not promised cancellation.
- Obsolete `SecretStorage` and its callers were already deleted by 02; no compatibility implementation or direct-write bypass was reintroduced. Updated both package and app READMEs with CRUD, settlement, failure, isolation, and manual qualification guidance.

### Validation

All run in this worktree:

- `pnpm --filter @aicacia/storage-db-collection test`: **41 passed** using real TanStack runtime and real Storage client with controlled HTTP/WebSocket boundary responses.
- `pnpm --filter @aicacia/storage-db-collection check`, `build`, `lint`, `format`: passed, no lint warnings.
- `pnpm --filter password-manager test`: **18 passed**, including **11 mounted collection/browser tests**. Expected live-query error diagnostics occur in disconnect/malformed-record cases.
- `pnpm --filter password-manager exec node --test svelte.config.test.mjs`: **2 passed**.
- `pnpm --filter password-manager check`: **0 errors, 0 warnings**.
- `pnpm --filter password-manager build`: passed with static adapter output.
- Focused Biome lint/format on `browser.test.ts`: passed. Biome's Svelte analysis reports template-only variables as unused; Svelte check/autofixer are the authoritative component checks here.
- Svelte autofixer: `Filesystem.svelte` has no issues/suggestions. `Browser.svelte` has no issues; suggestions concern the existing authenticated-context async lifecycle effect, deliberately retained rather than rewriting ticket 02's lifecycle.
- `git diff --check`: passed. TDD regressions were observed red before fixes for wrong-path acknowledgement, unedited empty notes, and ambiguous delete editor cleanup. Added an explicit queued-rollback regression after inspecting core's pending-versus-persisting rollback contract.

### Limits and handoff

Priority was restoring the reference app; critical risks were optimistic settlement, data preservation, uncertain outcomes, and identity isolation. Kept the eager snapshot/serialized single-record design, with no new dependency, Svelte wrapper, batch protocol, polling, or secret outbox.

Ticket 04 is unblocked for explicit Refresh/recovery and full-workflow qualification. No deployed Installation/OAuth/Node replication qualification was performed; controlled CRUD/reopen persistence is not proof of deployed-system behavior. There is no compare-and-swap/conflict prevention or atomic multi-record snapshot. A silent connected endpoint has no request deadline and can keep a mutation pending until response/disconnect/disposal; a sent write may still persist after disposal. These limits are documented, not hidden behind automatic retries.

No Rust changes; Cargo feature-powerset tests are not applicable. As requested, `just crap` remains for the coordinator's main checkout with sibling Rust path resolution. Six pre-existing tracked `packages/storage-client/node_modules/.bin/*` install-shim modifications remain uncommitted and untouched by implementation commits; coordinator-owned nested worktree directories were preserved.
