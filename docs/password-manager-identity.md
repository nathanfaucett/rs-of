# Password manager Identity items

Ticket 09 adds Identity create, detail, edit, cancel, save, and reopen. The field authority is [personal JSON](password-manager-json.md#five-type-field-mapping-and-validation), not an observed reference form.

## Fields and editing

Use **New identity**. Only Name is required. The eighteen optional Identity fields are title, first/middle/last name, three address lines, city, state/province, postal code, country, company, email, phone, social security number, username, passport number, and license number.

Values remain strings. Entry does not impose country, title, email, telephone, postal-code, or identifier rules. It does not trim, coerce numbers, remove leading zeroes, or change Unicode. Non-identifier controls accept multiline text. Identifier controls are password inputs. Existing multiline identifiers remain unchanged on unrelated edits; these controls do not offer multiline identifier entry.

An unchanged absent, null, or empty value retains that state on save. Clearing a stored value saves an empty string. Notes, favorite state, ordered custom fields, existing history, creation time, folder membership, and encrypted import provenance remain intact. Only the edited item changes. Identity edits do not add Login password history. Cancel discards the isolated draft and returns to masked details. The existing unsaved-navigation guard applies.

`identity-fields.ts` contains field keys and labels only. `IdentityFields.svelte` uses the existing Field control. The existing Identity schema remains the sole durable model. The draft, workflow, and detail use direct Identity branches; no storage layer, plugin, migration, network service, dependency, or extra asset is added.

## Protected actions and persistence

Every Identity-specific detail value and its notes are masked until explicitly revealed. The existing ProtectedValue control copies without revealing, reports clipboard failure without exception text or field values, and discards pending feedback on destruction. Identity usernames can appear in normal list results, as in the existing search contract. Common text/Boolean custom fields retain their existing display rules; hidden fields remain masked.

Focus moves to Name on entry and the detail heading on return. Controls have labels and use the existing validation/error feedback. No Identity values are logged. Manual or automatic lock destroys the draft, revealed detail, and pending-copy feedback. A write already sent can persist; reopen and verify before retrying. JavaScript memory zeroization and clipboard removal are not guaranteed.

Writes use the existing complete-snapshot validation, encrypted atomic staging, acknowledged collection writes, and authoritative reload after failure. Current version 3 encrypted backup includes Identity data, folders, Trash, and lock settings without a format change. See [backup](password-manager-backup.md), [folders](password-manager-folders.md), and [lock](password-manager-lock.md).

## Evidence

- `apps/password-manager/src/routes/identity-items.test.ts`: mounted real page/workflow/collection, Chromium SQL/WASM, IndexedDB, and Web Crypto. All eighteen synthetic international fields, multiline text, Unicode, leading zeroes, sparse absent/null/empty values, isolated cancellation and edits, common metadata/history, unchanged neighboring Login, required-name failure, clipboard success/failure, encryption/readback failures, manual lock during draft/copy/read/write/acknowledgement, expired Save, and authoritative reopen.
- The same suite exports a mixed vault with full and sparse Identity records, removes source IndexedDB storage, rejects a wrong password, restores, and compares the complete item/folder snapshot and saved lock settings.
- `scripts/check-identity.mjs`, called by `check-offline.mjs` and `check-backup.mjs`: built-app keyboard entry of all eighteen fields, protected detail, cancellation, cold offline reopen and edit with the server stopped; source-profile disposal and clean-profile offline encrypted restore with exact authenticated encrypted-payload comparison. Identity data is also excluded from the application-cache content check.

These checks use synthetic test vaults only. They do not establish other-browser support, actual disk-exhaustion behavior, reference-client parity, or an independent security audit.
