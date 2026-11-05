# Personal JSON import

Ticket 17 implements the [personal-json-v1 subset](password-manager-json.md), not a general Bitwarden migration tool. All five item types use the existing typed model. The [plaintext exporter](password-manager-export.md) is now integrated; the importer remains independent of its serializer.

## Operation

1. Unlock the destination vault. Select **Import Bitwarden JSON**. Confirm discarding a dirty editor only if intended. The import replaces the editor subtree, not its stored records.
2. Select a trusted UTF-8 plaintext JSON file. **Validate import** shows counts by type, folder count, validation errors, unsupported JSON Pointer paths, date-default warnings, and duplicate counts. It does not show item names or field values. Unknown member names are shown as paths, never their values.
3. Any blocking issue prevents the complete batch. Correct a copy of the source or cancel. There is no lossy supported-only import. Cancellation keeps stored items and folders unchanged.
4. If duplicates exist, cancel by default or check **Import all duplicates as new records and folders**. Then select **Import all as new**. There is no skip, merge, name-based replacement, or external-ID overwrite.
5. After **Import verified**, the editor reloads the authoritative complete collection and folder catalog. Filters and discarded drafts are not retained. Check items and keep an encrypted backup. A plaintext source file is not an encrypted backup.

The page shows the destination's full vault ID. Source organization or collection information does not create local rights; nonempty ownership data blocks import. No network, telemetry, URL state, plaintext index, or persistent import cache is added. Source files are not modified.

## Validation and mappings

The exact supported fields, numeric variants, defaults and exclusions are in [the JSON contract](password-manager-json.md). Import maps wire Login password to `secret`, history password/date to `history[].secret/changedAt`, custom-field numeric types to Text/Hidden/Boolean, and URI numeric rules to the existing stored rules. Missing URI match remains inherited `null`, not explicit Domain. Boolean values require the exact strings `"true"` or `"false"`. Card numbers, identifiers, text bytes, array order, duplicate field names, empty strings and nulls are not coerced. Regex text is not executed. SSH key text is not parsed or checked for usability.

Accepted dates normalize to UTC instants. Missing/null creation and revision dates use the disclosed preview time. History requires its supplied valid date. Dates with impossible calendar days, missing timezone, or invalid time fields block import. Duplicate JSON members, including escaped spellings, block before mapping. Other payloads, keys, variants, passkeys, attachments, encrypted data, archive, deleted items, reprompt policies and shared ownership block rather than disappear.

Implementation bounds: **16 MiB UTF-8**, **10,000 items**, **10,000 folders**, and **64 nested JSON levels**. The UI checks file size before reading and decodes UTF-8 with fatal errors. The API checks encoded size before parsing. Split oversized files; do not remove data to bypass unsupported-field checks. Source item/folder IDs must be nonblank text because encrypted local provenance uses a nonblank source ID; whitespace-only IDs are rejected, not normalized. This is narrower than a merely nonempty external ID.

## Duplicate and commit boundary

`src/lib/import/personal-json.ts` validates the whole external document and prepares fresh local UUIDs and remapped memberships. `src/lib/import/import.ts` exposes `preparePersonalJsonImport(vault, text)`: counts, issues, warnings, duplicate counts, `commit(choice?)`, and `retire()`. Prepared records stay private to that transient preview.

Source-ID matches use existing encrypted `importSource`, including items in Trash. Logical duplicates compare complete logical contents, folder names instead of remapped IDs, and date instants; missing source dates do not compare preview-time defaults. Omitted/null optional scalars both mean no value; an empty string remains distinct. Array order matters. Duplicate names alone are not matches. Identical no-ID records within one source file remain separate; repeats against the destination require the same batch confirmation. Matching uses transient in-memory sets, not stored plaintext hashes. Counts identify incoming matching records and can overlap.

Commit checks that the destination snapshot has not changed since preview. Import confirmations are serialized by a vault-specific Web Lock. It then calls **`vault.mutateItems({ upsert, folders })` once**, never a single-item write loop. The vault's existing authoritative merge, complete validation, encrypted staging, close/reopen verification and atomic registry publication preserve existing items, folders, Trash, history, encrypted lock settings and retained transition recovery. See [atomic publication and storage costs](password-manager-folders.md#persistence-and-failure-safety). Ordinary independent writes still merge through that boundary; the import does not lock other editors for the lifetime of a preview.

A failed pre-publication write leaves the destination unchanged. Failed publication acknowledgement can leave either the old complete snapshot or the complete new batch. The UI must not call that success or promise rollback. It instructs the user to lock, reopen, and verify before retrying. A submitted preview cannot be reused after a storage attempt. Retained encrypted staging databases can consume space; automatic cleanup is not added here.

## Lock and cancellation

Manual lock, expired-deadline checks, and destruction retire the controls and preview. File references, parsed records, choices and feedback are released from application state. Async continuations check the session deadline before preview, commit and success feedback. A sent storage write can still finish after lock. Reopen loads authoritative storage. JavaScript memory zeroization and transaction cancellation are not promised.

## Evidence

- `src/lib/import/import.test.ts`: all shared accept/block fixture cases, all five field mappings, counts, fresh IDs, folder remapping, history and URI ordering, repeat/no-ID duplicate choices, complete rejection through the public API, corruption, duplicate members, unknown nested paths, and bounds.
- `src/lib/import/persistence.test.ts`: real Chromium SQL/WASM, IndexedDB and Web Crypto; one batch call, unchanged neighbors/Trash/history/settings, encrypted markers/provenance, exact clean-storage backup recovery, quota/lost/publication/uncertain failures, concurrent confirmations, stale previews and retired handles.
- `src/routes/import.test.ts`: mounted real page/workflow/collection/storage; preview/cancel/commit, protected details, folder catalog, Trash/restore, close/reopen, duplicate choice, dirty-navigation refusal, focus, unsupported paths, failure feedback, and manual/automatic lock across pending reads, encryption, submission and completed-write acknowledgement.

- Combined integration: `src/routes/json-export.test.ts` captures a confirmed UI export and compares it with the independent static `export-expected.json` oracle before feeding its bytes into the import file control in a fresh destination vault. It checks all five kinds, folders plus an empty folder, ordered history/custom fields, all seven URI states, normalized dates, fresh/remapped IDs and provenance, no preview writes, and exactly one public atomic mutation. Trash is counted and deliberately excluded. A repeated file is rejected without explicit duplicate confirmation. Both open previews retire on lock; close/reopen verifies the committed destination.

Synthetic fixtures only. No live vault, credential export, or reference-client import is used. The combined test uses the mounted real application, SQL/WASM, IndexedDB, and Web Crypto, not a built-app offline import scenario. Built-app combined import/round-trip qualification, selected Bitwarden/Vaultwarden acceptance, other engines, independent security review, actual disk exhaustion and secure erasure remain for ticket 20 or separate review.
