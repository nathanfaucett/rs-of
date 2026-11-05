# Password manager plaintext JSON export

Ticket 18 implements export of the [personal-json-v1 subset](password-manager-json.md). This is plaintext portability, not the [encrypted recovery backup](password-manager-backup.md). No selected Bitwarden/Vaultwarden client acceptance is claimed; the [reference limits](password-manager-reference.md) still apply.

## User flow and scope

In an unlocked vault, select **Export Bitwarden JSON**. The page reads the complete committed snapshot through `vault.readItems()`, not the search, type, favorite, folder, or Trash view. The preview shows active counts for all five types, the folder count, and the count of excluded Trash items. It contains no credential values. Unsaved drafts are not exported. The snapshot is fixed at preview; later edits need a new preview.

The warning states that anyone with the file can read credentials, including private keys, notes, and historical passwords. **Cancel export** is focused first. Cancel releases the preview, creates no Blob or download, and returns focus to the export control. Controls are labelled native buttons and work with the keyboard. This is an inline section, not a modal or a focus trap. **Download plaintext JSON** is a separate explicit confirmation. A download-request message is not proof that the browser saved the file. Save it securely.

All folders are included, including empty folders and separate identities with duplicate names. Slash-delimited names remain literal. **All Trash items are excluded**, with an explicit count. Restore an item first to include it. Use an encrypted backup to retain Trash and complete vault recovery data. Export neither deletes nor modifies stored records.

Exclusions are explicit: Trash, attachments, passkeys, Sends, source account defaults/equivalent domains, encrypted import provenance, and local lock settings. Provenance is local migration bookkeeping, not content or output identity. Organization permissions are not recreated. Active data outside the supported model blocks the entire export; there is no partial-item fallback.

## Implemented mapping

`src/lib/json-export/export.ts` contains explicit allowlisted mappings. It validates the strict complete item/folder snapshot before preview and again before serialization. Unknown keys, item kinds, URI strategies, field variants, malformed records, duplicate IDs, or dangling folders block export. Controlled validation errors name JSON-pointer paths, never field values. Read/download/serialization failures use fixed messages, not exception text.

| Local data                         | Output                                                                                                                                                                                                                                      |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Envelope                           | `encrypted: false`, `folders`, `items`; no encrypted/account envelope                                                                                                                                                                       |
| Current local item/folder UUID     | `id`; membership uses the same current folder UUID in `folderId`, never an external source ID                                                                                                                                               |
| `kind`                             | Login 1, Secure Note 2, Card 3, Identity 4, SSH Key 5; exactly one matching payload                                                                                                                                                         |
| `name`, `notes`, `favorite`        | Same logical text/flag; absent/null optional text becomes null; empty string remains empty                                                                                                                                                  |
| Personal ownership                 | `organizationId: null`, `collectionIds: []`, `reprompt: 0`, `archivedDate: null`, `deletedDate: null`                                                                                                                                       |
| `fields`                           | Same order and duplicate names; text 0, hidden 1, Boolean 2; values preserved; optional null `linkedId` sentinel omitted                                                                                                                    |
| Boolean fields                     | Only exact strings `"true"` and `"false"`; old null, unset, or other text blocks export with the field path, never a truthiness conversion                                                                                                  |
| `history[].secret/changedAt`       | `passwordHistory[].password/lastUsedDate`; all types, ordered, no truncation/deduplication                                                                                                                                                  |
| `createdAt/updatedAt`              | `creationDate/revisionDate` in UTC ISO form; preserve the supplied instant; invalid calendar dates and nonzero sub-millisecond precision block instead of silently changing the instant                                                     |
| Login `username`, `secret`, `totp` | `login.username`, `login.password`, `login.totp`; no password requirement                                                                                                                                                                   |
| Login `uris`                       | Ordered `{ uri, match }`; inherited null stays null; domain 0, host 1, startsWith 2, exact 3, regex 4, never 5; no regex execution/autofill claim                                                                                           |
| No local passkeys                  | `login.fido2Credentials: []`                                                                                                                                                                                                                |
| Secure Note `type`, common `notes` | `secureNote.type: 0`; body remains common `notes`                                                                                                                                                                                           |
| Card                               | `card.cardholderName`, `brand`, `number`, `expMonth`, `expYear`, `code`, unchanged strings; no numeric/date/brand normalization                                                                                                             |
| Identity                           | `identity.title`, `firstName`, `middleName`, `lastName`, `address1`, `address2`, `address3`, `city`, `state`, `postalCode`, `country`, `company`, `email`, `phone`, `ssn`, `username`, `passportNumber`, `licenseNumber`, unchanged strings |
| SSH Key                            | `sshKey.privateKey`, `publicKey`, `keyFingerprint`, unchanged opaque text; no parsing, fingerprint derivation, or key-pair validity claim                                                                                                   |
| Missing arrays                     | Empty arrays, not null; optional type-specific strings are null; Unicode, multiline text, leading zeroes, and array order remain intact                                                                                                     |

## Session and plaintext lifetime

The export control exists only in the unlocked subtree. Page lock and destruction synchronously call its `retire()` method. Retirement clears preview/error/feedback references, invalidates requests, and revokes any pending object URL. Cancellation also invalidates pending preview reads. Every async read result checks its request and the page's inactivity deadline before publication. Confirmation checks the deadline/session before serialization, Blob creation, and anchor activation. Lock is not blocked by export.

Plaintext JSON and the Blob are created only on confirmation. No plaintext cache, storage record, log, telemetry, URL query, or remote API is used. The object URL is revoked after one second, on lock/destruction, or immediately on download failure. Download anchors are removed even on errors. A download already initiated cannot be recalled from the browser or filesystem. JavaScript cannot guarantee memory zeroization; these checks do not protect a compromised browser/OS.

## Independent evidence and commands

New synthetic fixtures only:

- `docs/fixtures/password-manager/export-local.json`: independently specified durable local inputs, including all five kinds and source provenance; offset dates exercise UTC normalization.
- `docs/fixtures/password-manager/export-expected.json`: static expected wire data based on the existing research `supported.json`, not output from the exporter or importer. Missing common research values use explicitly supplied fixture dates/defaults; the optional null linked sentinel is omitted. No real credentials or usable SSH keys.

`src/routes/json-export.test.ts` mounts the real page, workflow, SQL/WASM, IndexedDB, and Web Crypto. It captures only the browser download boundary. Tests compare all five types with the independent expected file, check unchanged public snapshots, warnings/scope, focus/cancel, Trash and duplicate/empty folders, unsupported Boolean paths, serialization/Blob-URL/anchor errors and retry, pending-read cancel/manual lock/deadline/unmount/error, and expired confirmation. Errors contain no synthetic secret values.

`src/lib/json-export/export.test.ts` gives supporting known-answer serializer/validation checks: every reference field, optional/minimal records, all custom variants, history on non-Login records, unknown root/nested data, variants, duplicate/dangling identities, and date precision/calendar rejection. It does not use importer self-round-trip as an oracle.

`scripts/check-json-export.mjs` exercises the built app in a fresh synthetic browser context. After asset readiness it disables networking and stops the server, reloads, creates all five types and an empty folder, locks/reloads/unlocks, checks keyboard cancel/focus, captures a real browser download, verifies known logical data, and checks lock and zero server requests. It does not connect to live browser tabs. The existing `check-offline.mjs` retains five-type cold startup and clean-profile encrypted backup/restore coverage.

Run from `apps/password-manager`; serialize each command:

```sh
flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock pnpm test --maxWorkers=1 --no-file-parallelism
flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock pnpm run check
flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock pnpm run build
flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock node scripts/check-json-export.mjs
flock /home/nathan/code/rust/aicacia/of/.scratch/password-manager-validation.lock node scripts/check-offline.mjs
```

## Combined importer integration

Ticket 17 is now integrated. The additional mounted `src/routes/json-export.test.ts` flow compares the actual confirmed download with the independent static expected wire fixture, then previews and atomically imports those bytes into a fresh vault. It checks all five types, empty folders and remapped membership, history/date normalization, all seven URI states, ordered custom fields and duplicate names, fresh IDs/provenance, deliberate counted Trash exclusion, unchanged source data, no preview writes, one public mutation, and rejection of an unconfirmed repeat. Imported logical values are checked against the separately specified local fixture, not a second serialization of importer output. Lock retires both previews; reopen verifies the destination.

This mounted test is supplemented by ticket 20's built-app offline UI export/preview/import using the same independent complete wire/local fixtures, rejected files, cancellation, injected quota failure and explicit duplicate choices. The existing export entry point now also checks complete encrypted recovery after source-context disposal. See [qualification, exact commands and operations](password-manager-qualification.md). Other engines, actual disk exhaustion, selected reference-client acceptance and an independent security review remain unverified.
