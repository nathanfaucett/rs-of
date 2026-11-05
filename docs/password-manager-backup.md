# Password manager encrypted backup and restore

This is the local backup contract implemented by ticket 04. It is not Bitwarden JSON, a server export, a password-recovery mechanism, or an independent security audit. See [reference scope](password-manager-reference.md) and [plaintext JSON portability](password-manager-json.md).

## User operation

1. Unlock the source vault. Select **Download encrypted backup**.
2. Check that the browser saved the file. Keep a copy outside browser storage. The app requests a download; it cannot prove that the operating system saved it.
3. In a clean browser profile, load the app and select **Restore encrypted backup**. Select the file and enter the master password used when the file was made.
4. Select **Validate backup**. Check the authenticated display name, full vault ID, and record count. No destination is written at this step. Cancel leaves local vaults unchanged.
5. Select **Restore vault**. After **Restore verified**, unlock the restored vault and check its contents. Keep the backup until recovery is confirmed.

Export contains the last committed record snapshot. Wait for pending edits to save before export. Browser-local data, service-worker caches, and the download directory are not substitutes for an external backup. A downloaded file remains sensitive: protect it against deletion and offline password guessing.

Restore preserves the vault ID and every record ID. An existing ID, including an interrupted creation reservation, blocks restore. There is no overwrite, merge, replace, or import-as-new mode. Use a separate clean profile; do not delete the source to bypass this check. Independent vaults can share a display name; the selector identifies them by full ID.

## Version 1 format

The UTF-8 JSON envelope has these exact supported fields. Unknown fields and other parameter values are rejected. Object member order and whitespace do not change authenticated semantic values.

| Field | Value |
| --- | --- |
| `format` | `password-manager/encrypted-backup` |
| `version` | Integer `1` |
| `kdf.name` | `PBKDF2` |
| `kdf.hash` | `SHA-512` |
| `kdf.iterations` | Integer `600000` |
| `kdf.salt` | Canonical Base64 of the source's 16-byte derivation salt |
| `cipher` | `AES-256-GCM` |
| `nonce` | Canonical Base64 of a fresh random 12-byte nonce |
| `ciphertext` | Canonical Base64 of the encrypted payload, including the 128-bit GCM tag |

The non-exportable 256-bit source key is derived locally from the master password and salt. No password, raw key, recovery key, plaintext item, or plaintext vault name is in the envelope. Export never calls `exportKey`. No network request is needed.

GCM additional authenticated data is the UTF-8 encoding of this compact JSON array:

```json
["password-manager/encrypted-backup",1,"PBKDF2","SHA-512",600000,"<kdf.salt>","AES-256-GCM"]
```

`<kdf.salt>` means the envelope's exact canonical Base64 salt string. This purpose label separates backup encryption from record/proof encryption. GCM also authenticates its nonce and the complete ciphertext. The parser accepts only the fixed supported derivation parameters, not attacker-selected work factors or algorithms.

The decrypted payload has exactly:

- `meta`: original `id`, `salt`, `iv`, and `createdAt`.
- `authentication`: the original versioned vault authentication proof string.
- `name`: the source registry's display name.
- `secrets`: the full encrypted record array, including deleted/Trash records. Each entry retains `id`, `ciphertext`, `nonce`, `createdAt`, and `updatedAt`.

The outer GCM tag binds the full set, order, IDs, timestamps, name, proof, and source metadata. The proof is separately checked against the original metadata and derived key. All records are decrypted and validated against the current application schema, with matching inner/outer IDs and no duplicate IDs, before export or restore preparation succeeds. No partial supported subset is accepted. Decrypted record text is not reserialized into storage: its original ciphertext and nonce are retained.

Version 1 backs up the current Login-like Secret model: credentials, ordered URI rules, TOTP text, notes, favorite state, ordered custom fields, history, dates, and deletion state. It does not claim that later typed items, folder tables, import provenance, attachments, or settings already exist. A future schema that adds persisted logical data must extend/version this contract and its complete-snapshot tests before claiming complete backup support. App assets, cache readiness, registry storage paths, and transient editor drafts are not vault data and are not restored.

Maximum input/output: **16 MiB** of UTF-8 JSON and **10,000 records**. The UI checks file size before reading it; the API checks encoded size before JSON parse/key derivation. These are tested implementation limits, not encryption limits. Export fails rather than omitting records above the limits.

The obsolete `exportEncrypted()` record-only file is not recoverable by this contract: it lacks required derivation metadata and complete authentication. It is rejected, not wrapped or guessed. Keep its original source browser data for separate recovery.

## API and atomic publication

- `VaultHandle.exportBackup()` returns the complete encrypted file. It validates the proof and all records and does not modify source storage.
- `prepareBackupRestore(password, file)` returns an authenticated preview with `id`, `name`, `recordCount`, and `restore()`. Preparation does not open or write a destination database. Drop the preview when cancelling; it holds a non-exportable key and encrypted records in memory.
- `preview.restore()` checks existing identities, writes to a fresh unlisted `password-manager-restore-<random UUID>` database, and commits metadata/proof plus all records in one SQL transaction.
- It closes that database and reopens it, then compares the exact persisted metadata, proof, record identities, ciphertext, nonces, and timestamps. Acknowledgement alone is not success.
- Only after verification does one IndexedDB registry `add` transaction publish the complete destination as ready. Unique ID/database constraints prevent concurrent restore or creation from replacing it. The saved registry entry is read back before success is reported.

SQL storage and the registry are separate databases; there is no claimed cross-database transaction. Atomic **visibility** comes from publishing only a complete, verified destination. A crash/failure before publication can leave encrypted, unlisted staging storage. It is never auto-registered as a legacy vault or shown as a partial destination. Retry uses a fresh staging database. This version does not automatically delete abandoned staging databases; keep the backup and check available browser storage. Never clear all site data to repair a failure.

A rejected publication or failed readback can have an uncertain outcome. The error tells the user to reload the selector, check the full vault ID, and unlock/verify it before retrying. If publication committed, the destination is complete. No error path assumes that a rejected write rolled back, deletes a possibly published vault, or claims success. A repeat restore is then blocked by the existing ID. Source vaults are never overwritten or deleted.

## Security and recovery limits

Validation rejects wrong passwords, missing metadata, changed authenticated values, damaged/truncated files, unsupported versions/parameters, duplicate/mismatched IDs, invalid logical data, and unsupported payload fields before destination writes. Errors do not contain passwords, keys, decrypted content, parser values, or imported field values. The UI displays only authenticated preview metadata, not decrypted records.

Authenticated backup detects alteration of a file's interpreted contents. It does not prove that the source record set was historically complete, detect replacement with an older valid backup, prevent source deletion, recover a forgotten password, or protect a compromised browser/OS. Someone who knows the master password can create another valid backup. JavaScript does not guarantee memory zeroization. Key lifecycle/automatic lock qualification and independent cryptographic review remain separate work.

## Evidence and checks

`apps/password-manager/src/lib/vault/backup.test.ts` uses real Chromium Web Crypto, SQL/WASM, and IndexedDB. It discards source storage before empty/populated recovery; compares all supported logical fields and exact encrypted metadata/proof/records after reopen; checks sensitive markers, invalid files, malformed logical records, bounds, collisions, concurrent publication, quota/write faults, a successful-but-lost write, and publication readback failure.

`src/routes/backup.test.ts` mounts the real page, workflow, collection, and vault. It exercises the download, file input, masked password, validation preview, confirmation, cancel, collision feedback, failure/retry, and reopened data. Only the browser download action is intercepted to capture its encrypted Blob; persistence and crypto are real.

Run from `apps/password-manager`:

```sh
pnpm test
pnpm run check
pnpm run build
node scripts/check-offline.mjs
```

The built-app check reuses the existing test server and creates isolated test browser contexts. It exports empty/populated synthetic vaults offline, closes the source context, restores into a new clean context offline, reloads/unlocks, and compares a second backup's complete metadata/proof/encrypted record set. It uses no live browser tabs. Other engines, actual disk exhaustion, private-browsing eviction, and independent security review are not established by these checks.
