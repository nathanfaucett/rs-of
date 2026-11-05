# Password manager local lock

## User behavior

Each vault supports 1, 5, 15, or 30 minutes of inactivity. The default is 5 minutes. Use **Automatic lock** → **Lock after inactivity** → **Save lock settings**. The controls support keyboard use and have labels and save/error feedback. Only a verified save changes the current timeout. Settings apply to this vault, not other vaults or already open sessions in other tabs.

Each unlock starts a new deadline with the saved duration. Pointer, keyboard, input, click, and wheel activity extend an unexpired deadline. Activity after expiry locks first; it cannot extend an expired session or submit the active draft. Focus, page show, and visibility changes check the deadline without extending it. A one-second timer checks while the page can run. Backup completion also checks before starting a download. Browser timers can be suspended; the app cannot execute while suspended. Resume checks enforce the deadline when execution resumes. Ordinary browser code cannot reliably detect operating-system lock.

**Lock vault** works during pending work and discards unsaved drafts without confirmation. Both manual and automatic lock retire the editor/workflow and collection, clear selection and application references to decrypted records, drafts, history, and revealed values, and close the handle. Reopening needs the master password. Reload, tab close, or page destruction does not retain an unlocked session. Generator output and displayed TOTP codes are not present in this baseline; later features must keep transient values inside the retired subtree.

## Storage and public API

`VaultHandle.loadLockSettings()` returns `{ timeoutMinutes: 1 | 5 | 15 | 30 }`. `saveLockSettings(settings)` validates, encrypts, writes, reads back the exact ciphertext, decrypts, and validates before acknowledging success. Both use the existing per-vault Web Lock. A failed acknowledgement may mean the write committed. The UI keeps the previous session deadline and tells the user to lock, reopen, and verify before retrying.

SQL migration 4 adds nullable `vault_meta.lock_settings`. Null means the documented default, not an unlocked or unlimited session. A saved value is JSON containing AES-GCM ciphertext and a fresh nonce, not plaintext settings. Additional authenticated data is UTF-8 `password-manager/lock-settings/v1/<vault ID>`. This binds settings to their purpose and vault. Invalid or damaged saved settings block entry; they are not reset silently. This does not prevent deletion or rollback by someone who controls browser storage.

Current encrypted backups include optional nullable `lockSettings` inside the authenticated payload, preserving the exact encrypted string. Existing version 1/2 files without it use the default. Envelope versions remain 1/2 and typed item format remains 2; older strict readers reject the extended payload. Use this app or a newer supported reader for recovery. Staging, format transition, and close/reopen restore verification retain and authenticate the settings. A pre-transition backup contains settings as they were before conversion, not later updates.

`close()` is idempotent and retires that handle permanently. Public reads, writes, preparation, settings, and backup APIs reject through a retired handle. Async results check handle state after persistence or crypto work. The page rejects late session results; destroyed controls cannot publish saved/copied feedback. A request already sent to storage can commit despite lock. Lock does not promise transaction cancellation or rollback. Unlock reloads authoritative persisted data; verify before retrying. Existing complete-snapshot validation and write reconciliation remain in place.

## Security limits

Lock is local and uses no server, biometric, PIN, or TOTP unlock. Closing releases application key references, but JavaScript cannot guarantee memory zeroization or cancel in-flight cryptographic work. A compromised browser/OS is outside this protection. Clipboard writes already sent may finish after lock; clipboard contents are not cleared automatically. The app does not attempt a read/clear sequence that could overwrite newer clipboard data. Stored items and encrypted backups are not erased by lock.

## Evidence

- `src/lib/vault/lock.test.ts`: real Chromium SQL/WASM, IndexedDB, and Web Crypto; encrypted settings, defaults, clean-storage backup restore with an item, damaged settings, lost/rejected-after-commit writes, retired handles, and late item/settings/delete/backup results.
- `src/routes/lock.test.ts`: mounted real page/workflow/collection; visible timeout save/reopen, active draft expiry, timer suspension and resume, expired activity, unexpired activity, page restart, wrong password, vault isolation, and late settings/backup callbacks.
- `src/routes/detail.test.ts`: manual and automatic lock during reads, clipboard writes, item writes, and persisted-write acknowledgement; no late secrets or success feedback; authoritative reopen after committed writes.

These checks use synthetic data, not live vaults. Other browser engines, actual operating-system suspension/lock, secure erasure, and independent security review are not established.

## Merger contract for ticket 12

This branch and folder ticket 12 both append SQL migration 4. Before release, keep one as 4 and append the other as 5 in the combined ordered migration list. Do not change a migration already used by released vaults. Test each branch's development vaults only with its matching migration history; do not clear real storage to bypass a mismatch.

Folder work adds authenticated backup envelope version 3 and separate folder records. Keep both `folders` and `lockSettings` in snapshot construction, validation, transactional staging, exact reopen verification, and every format-transition or folder-mutation staging payload. Folder mutations must not reset settings. Retain folder version selection/AAD rules and old version 1/2 reading, with absent lock settings selecting the default. Apply the closed-handle guards to the new folder APIs and test folder mutations across lock. No shared map/spec or folder source is changed by this ticket.
