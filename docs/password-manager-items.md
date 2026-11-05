# Password manager typed items

Implemented by ticket 05. The field authority is [personal-json-v1](password-manager-json.md); the evidence limits remain in [the reference baseline](password-manager-reference.md). Only Login has an editor in this ticket. The other four types have validated storage and backup support, not type-specific UI, autofill, import, or export.

## Model and interfaces

`apps/password-manager/src/lib/secrets/secret.ts` exports `SecretSchema`, a strict Valibot variant on `kind`, and the `Secret` union. It also exports each type and schema: `Login`, `Card`, `Identity`, `SecureNote`, and `SshKey`. Narrow on `kind` before reading type-specific properties. There is no parallel generic required-password schema or legacy writer.

Type fields are flat within each encrypted record. The names are local names, not Bitwarden wire objects:

| JSON kind / number | Local `kind` | Type-specific mapping                                                                                                                                                 |
| ------------------ | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Login / 1          | `login`      | `login.password` → `secret`; `username`, `totp`, and ordered `uris` retain their names. All credentials are optional.                                                 |
| Secure Note / 2    | `secureNote` | `secureNote.type` → `type`, default integer `0`; body → common `notes`.                                                                                               |
| Card / 3           | `card`       | `cardholderName`, `brand`, `number`, `expMonth`, `expYear`, `code`, unchanged. All optional strings; no numeric coercion, brand restriction, Luhn, or date rejection. |
| Identity / 4       | `identity`   | All eighteen fields in the JSON contract retain their names. All optional strings; no country-dependent normalization.                                                |
| SSH Key / 5        | `sshKey`     | `privateKey`, `publicKey`, `keyFingerprint`, unchanged. Only `privateKey` must be nonblank; accepted text is not reformatted or checked as a key pair.                |

Common data: UUID `id`, nonblank `name`, optional `notes`, `favorite`, ordered `fields`, ordered `history`, `createdAt`, `updatedAt`, optional `deletedAt`, optional UUID/null `folderId`, and optional `importSource`. Dates retain their supplied ISO timestamp strings during transition; edits retain creation time and set update time. `passwordHistory[].password/lastUsedDate` maps to `history[].secret/changedAt`. No history truncation or deduplication occurs.

Optional scalar fields accept omission, null, and strings, including empty strings. The Login draft uses empty controls but retains an unchanged stored absent/null/empty value on save. Removing a previously stored password records the prior value in history. A new or changed nonempty password requires confirmation; removing one does not require an impossible nonempty confirmation. Cancel changes no stored value, including nested URI/custom-field edits.

URI `match: null` means inherited default and differs from explicit `domain`. The other local rules are `host`, `startsWith`, `exact`, `regex`, and `never`. Legacy omission becomes explicit `domain`, as the old reader required. No URI rule is executed by this ticket. Text and hidden field values can be null. Existing Boolean wire text is preserved, including old unset values; the future personal JSON importer must separately enforce its exact `"true"`/`"false"` input rule. Unknown keys, kinds, URI strategies, and custom-field variants block the complete snapshot; they are not stripped from persisted data.

`importSource: { format: "personal-json-v1", id: <nonblank external ID> }` is encrypted within the item and included in backups. This is the durable source-ID slot for ticket 17. It does not authorize destination overwrite. Import must allocate a fresh local ID, validate the whole file, compare source IDs and logical duplicates, and require the documented batch choice. No importer, duplicate index, plaintext hash, or bulk commit API is implemented here. `folderId` stores membership only; folder records and behavior remain ticket 12.

### Callers for ticket 06 and type tickets

- `createLocalSecretCollection({ vault })` accepts the full union. It calls `vault.prepareItems()` before loading a complete validated snapshot. Existing single-mutation acknowledgement and authoritative reconciliation remain. This is **not** an atomic batch importer.
- `createLocalSecretWorkflow({ vault })` exposes union `records`. Its current `edit`/draft/save path is Login-only. It refuses other kinds instead of converting them into Login. Detail actions in ticket 06 must narrow the kind. Later editors can use the corresponding schema without changing storage or backup routing.
- `createSecretDraft` / `constructSecret` accept `Login`, not the union. They preserve metadata and isolate nested editable arrays. Transient TanStack virtual properties are stripped only when making a draft; durable records use strict validation.
- Public vault preparation: `await vault.prepareItems()`. Call it before typed writes and reads outside the collection. The low-level encrypted storage API still accepts serialized text for storage tests/recovery inspection; after preparation it validates the full typed item and matching ID. There is no old-format write path. `close()` retires the handle and releases its key reference. Late decrypted results cannot return through a closed handle.
- `exportBackup()` exports the current committed format. `exportTransitionBackup()` independently authenticates and exports the retained pre-transition version 1 file, if present. It does not depend on current records being valid; damaged recovery metadata is rejected. The page provides both download actions, even when collection loading fails.

## Versioned transition and recovery

SQL migration 3 appends `item_format` (default `1`) and `transition_backup`; migrations 1 and 2 are unchanged. New vaults start at item format `2`. Authentication/identity transitions remain separate. Unlock alone does not rewrite records. The collection prepares the format before exposing any records or enabling editing.

For format 1, preparation validates **every** decrypted record, nested field, date, and matching inner/outer ID. It builds Login records without changing IDs, logical dates, passwords, TOTP, notes, favorites, ordered URI rules, custom fields, history, or deletion dates. It also retains the original outer record timestamps. Malformed/unknown data stops the operation before staging; no partial subset is shown.

Preparation then makes an authenticated version 1 source backup and checks that the complete version 2 backup, including the source backup, fits the supported limit. It uses the same transactional staging and close/reopen/exact-readback verification as restore. The new database contains the complete converted set, original metadata/proof, item format 2, and the encrypted source backup. Only after durable verification does an IndexedDB transaction compare the original registry entry and publish the new database pointer. A registry readback is required before success.

The **original database is retained unchanged**. Registry `previousDatabaseName` prevents a retained named legacy source from being registered again as another vault. Staging names are not auto-discovered. No error path deletes the original, guesses that a rejected write rolled back, or reports unconfirmed conversion as saved. A failed or unconfirmed publication can leave either the original pointer or a complete converted pointer; reopen and verify before retrying. Retry validates the authoritative published format and does not convert it twice.

Web Locks serialize preparation and local writes by vault ID across handles/tabs. A stale handle resolves the published database before writing, checks its bound metadata/proof, and cannot write into the retained source. Other old application builds do not participate in this protocol: close them before upgrade. This ticket does not promise compatibility with old writers.

For recovery, keep browser storage. Download the current encrypted backup, or **Download pre-transition backup** after a committed transition. The latter contains the state before conversion, not later edits. Restore in a clean profile; an existing ID blocks restore. Version 1 recovery restores exact original encrypted records; the next collection load validates and transitions them again. Do not delete a source merely to bypass the ID check. See [backup operations and limits](password-manager-backup.md).

Retention costs space: source database, staged database, and embedded recovery backup can coexist. Failed staging can leave unlisted encrypted databases. There is no automatic cleanup or purge. Both current and retained pre-transition data can include deleted records and old passwords. Keep that in mind when retaining or distributing backups. The total backup limit remains 16 MiB / 10,000 current records, including embedded recovery bytes in the byte limit. Oversized transitions stop before publication.

## Checks and scope

`src/lib/vault/item-transition.test.ts` uses real Chromium SQL/WASM, IndexedDB, and Web Crypto through public vault methods and a mounted real page. It covers all five field sets, optional values, encrypted provenance, source retention, exact reopened snapshots, both backup versions in clean storage, malformed later records, unknown data, mismatched IDs, lost/quota writes, failed reopen, publication failure/readback uncertainty, concurrent preparation, stale handles, damaged-current recovery, damaged recovery rejection, mounted recovery downloads, cancel, save, lock, and reopen. Existing collection acknowledgement tests remain in place.

No new runtime dependency, network request, plaintext index, worker, or separately fetched asset was added. Production offline checks cover the built typed Login workflow and version 2 backup/clean-profile restore. Real offline **legacy conversion** is covered by mounted/public persistence tests, not a separate built-app upgrade fixture. Automatic timeout lock, secure JavaScript erasure, independent security review, real disk exhaustion, and other browser engines remain separate qualification work.
