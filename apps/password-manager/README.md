# Password-manager reference application

This Svelte application demonstrates the platform's north-star workflow. It is intentionally small and is **not a qualified production password manager**.

## Current local behavior

- Vaults use the browser `@ofsdk/sql` package and IndexedDB.
- Core local vault operations do not require an IdP account or Storage service.
- Search and filters work together. Select an item name to view its details, move it to an existing folder, or choose **Edit**. Details appear beside the list on desktop and below it on narrow screens. Login details show available authenticator codes and password history. **Launch** opens a valid HTTP or HTTPS website in a new tab.
- The vault selector and the navigation sidebar for an unlocked local vault link to the OIDC sign-in route when you are signed out. On narrow screens, use **Open navigation** in the page header. **Close navigation** and Escape return focus to that button. If the window switches to the desktop layout while navigation is open, focus moves to the current desktop destination.
- Vault records are encrypted with AES-256-GCM. The current key derivation uses PBKDF2-SHA-512 with 600,000 iterations.
- Press Enter to create or unlock a vault. The create form shows the master-password rule before submission.
- Ordinary Identity fields and Notes are visible while you edit. Saved Secure Note and Identity notes still start masked in item details.
- Protected values start masked. Use **Show** to reveal one field. Changing a hidden custom field to another type clears its draft value; cancel the edit to keep the saved field. Copy feedback does not include the value. The application does not clear the clipboard automatically.
- A confirmed inactivity timeout save starts a new deadline for the unlocked vault. Unlock loads the saved setting from the encrypted vault. Returning to the vault selector with the browser Back button ends the unlocked session.
- Leaving a dirty item draft asks for confirmation. Locking does not ask: it clears the decrypted view and discards unsaved edits. A save already written to local storage can still be present after unlock.
- Browser local storage is not an encrypted backup.
- The application includes local backup/recovery and offline-shell support; platform, eviction, update, and recovery limits still apply.
- Items stay in Trash until you restore them or confirm permanent deletion. There is no automatic deletion. Permanent deletion does not change existing backups.
- In Settings, validate a personal Bitwarden JSON file before you import it. **Cancel import** clears the current batch and returns to the file picker. A verified import closes its preview and focuses the result. If mobile navigation is open, focus stays there until it closes. Select **Import another file** to start a new batch.

Use test secrets only. Do not infer production cryptographic, browser, or remote synchronization qualification from local CRUD tests.

## Target platform workflow

1. Create and use an encrypted vault offline.
2. Sign in to a Sync Server with OIDC.
3. Approve the Device and select the vault Resource for synchronization.
4. Synchronize ciphertext, not the master password or plaintext vault records.
5. Select the vault on a second Device, edit offline on both, and converge after reconnection.

This is the accepted target, not a claim that every step works now. See [product goal](../../docs/goal.md), [shared Device architecture](../../docs/architecture.md), and [current status](../../README.md#current-status).

OIDC authenticates the User and Application. It does not decrypt vaults, approve Devices, or select Resources.

## Qualified single-Node synchronization

The real-backend Chromium scenario creates an encrypted IndexedDB vault, signs in with OIDC, and explicitly synchronizes through the public sync SDK to one local Sync Server. Create, update, delete, and repeated synchronization complete through SQL history v7. The test reads changed ciphertext through the public Storage API, checks repeat idempotency, and verifies that plaintext records and the master password are absent. It also rejects a wrong password and reopens with the correct password.

Attached sessions close after success and every failure. Handle or SDK close aborts and awaits active synchronization. Attach has a 10-second deadline and receive inactivity has a 30-second deadline. This is explicit single-Node behavior, not continuous delivery, two-Device convergence, durable replay, or production password-manager qualification.

### Sync the active local database

Use the vault selector to connect a populated local database or restore into a browser with no local history. First upload requires confirmation and a publication receipt from Storage. If the local save fails after Storage confirms publication, use **Retry local save**. This works only in the login session that received the receipt. It does not contact Storage or replace a newer local binding. It saves the binding with sync detached; choose **Reattach sync** to activate sync. If that receipt is lost, the app does not link the database or try an unguarded upload. A restore only reads from Storage when it creates the local copy.

After a verified upload or restore, **Reattach sync** activates exchange with the saved Resource for this tab. The SDK exchanges on activation, after local commits, and when the browser comes back online. Use **Sync now** to retry or check for remote-only changes. This is not continuous remote delivery; writes in other tabs or workers do not wake this tab. Closing or reloading the tab requires activation again, even when the account and destination are unchanged.

The app retains the verified Installation, User/Application namespace, and exact Resource ID in local database metadata, not in SQL history or vault backups. A different account or Installation cannot use or replace the binding. Old permission records do not activate sync. A previously uploaded database without a verified local record remains unlinked; existing remote history does not prove that the earlier upload came from this database.

Each exchange requires current Storage authorization and Selection. The app rechecks the saved login and, for an established binding, attachment permission before attach, history sends, and local history writes. These checks do not wait for a notification from another tab. Edits stay local if sync fails. **Detach sync** cancels the exchange and keeps both copies. A local commit already issued to IndexedDB can finish. Signing out cancels sync and locks opened vaults without waiting for database access. If the browser cannot clear saved credentials, the app reports incomplete sign-out. Fix browser storage and retry before reloading. If no persistent change succeeds, reload can restore the old login and receipt. No background upload starts without explicit activation. **Retry local save** never activates sync. This is local sign-out, not IdP session termination or token revocation.

## Local development

Use the toolchain versions in `rust-toolchain.toml`, `.nvmrc`, and the root contributor guide. From the repository root:

```bash
pnpm install --frozen-lockfile --ignore-scripts
pnpm e2e:prepare
pnpm apps:dev
```

`pnpm e2e:prepare` builds local SQL/filesystem WASM and generated workspace dependencies. No sibling `ofdb` checkout is used. See [E2E setup](../../e2e/README.md) for prerequisites and validation limits.

Local vault use needs no service configuration. Use the [default Node setup](../../README.md#local-backend) to enable sign-in. Sign-in uses the ready Installation from the Node at `PUBLIC_UNIFIED_ORIGIN` (default: `https://unified.localhost:1355`; Portless also matches the worktree name and proxy port). If setup is incomplete, the sign-in page links to `PUBLIC_NODE_SETUP_URL` (default: `https://of.localhost:1355/setup`; Portless supplies the matching app URL). The Node must publish the selected IdP and Storage bases and permit the Password Manager origin to read its status. The selected Installation must permit that origin to use IdP and Storage APIs and provide the existing sign-in and consent UI. Unified Create configures access for the Password Manager origin. After Join, the existing Installation must permit that origin.

For a new OAuth Client, sign in as a User allowed to register it. Select **Accept** to register Password Manager, then **Allow** to grant access. The Application URI defaults to the Password Manager origin. An explicit fixed-client configuration uses `PUBLIC_OIDC_CLIENT_ID`, `PUBLIC_OIDC_APPLICATION_URI`, `PUBLIC_IDP_BASE_URL`, and `PUBLIC_STORAGE_URL` (or `PUBLIC_STORAGE_BASE_URL`), as in the [seeded test fixture](../../e2e/README.md#browser-configuration-and-desktop-isolation). Do not use a seeded Client for an Installation created by Node setup.

**Create new vault** always creates a local vault. Sign-in does not unlock a vault or upload its data.

To upload the local database, lock the vault and select **Connect local database**. Check the Installation issuer, User subject, and Application URI. Select **Confirm and upload** to register or reopen the same logical Storage Resource and try a guarded first upload. Storage must authorize the upload.

If Storage denies the upload, check whether the Resource is selected on the serving Node’s approved Device. Use the displayed Resource ID. The current [Device route](../../openapi/device.json) requires the serving Device owner to set Selection. That same User needs Storage read access to the Resource and the Application’s `devices.select` permission. An Installation operator cannot select the Resource just by being an operator. Password Manager does not set Selection or grant access. After authorization is in place, select **Retry first upload**.

The first upload uses the existing browser database. If the publish receipt is lost, Storage may already have saved the upload. Keep using the local vaults. A retry uses the same Resource and still requires an empty destination. If Storage reports existing history, the app stops: it cannot verify an earlier upload and does not merge, overwrite, or fall back to an unguarded exchange. First upload does not activate background sync. Vault names and other metadata are not encrypted by the vault record encryption.

The browser must allow IndexedDB, Web Crypto, Web Locks, and the relevant service-worker/cache APIs for offline delivery. Use HTTPS or a secure loopback development context.

## Reference-app scope

The platform milestones need basic encrypted CRUD, lock/unlock, backup/recovery, OIDC sign-in, Resource synchronization, and offline restart. Additional item types, import formats, generators, and production password-manager features are not platform completion criteria.

Do not add a second synchronization or identity model for this application. Use the public `ofdb` browser facade, OIDC client, Storage client, and shared Device/Selection contracts.

## Checks

```bash
pnpm --filter @ofsdk/password-manager test
pnpm --filter @ofsdk/password-manager check
pnpm --filter @ofsdk/password-manager test:backend
pnpm --filter @ofsdk/password-manager test:e2e
```

Backend tests start an isolated real local backend. They must not replace remote synchronization with local-only assertions. Record failures and qualification limits in CI results rather than claiming success from unit tests.

### Restore a Storage database

Sign in, select **Find remote database**, then confirm the displayed Storage Resource ID. Restore downloads the first-upload Resource into the regular local vault store. The app looks up the exact first-upload creation key in the signed-in namespace. It does not search by display name or create a Resource. If standalone Storage reports the copy unavailable after a restart, open it through an ordinary authorized Storage operation before retrying. Unified Storage does not require that step. Storage must authorize a read-only exchange and the serving Device must have a Selection when required. Unlock restored vaults with their existing passwords, with or without sign-in.

Finding a Resource on a fresh browser does not start an exchange. If this browser has retained SQL history, the selector offers merge instead of restore. If it has older local vaults in the per-vault registry, unlock them from the selector before trying again. A provisioned schema or a deleted vault is not a fresh database. Restore checks both local sources again before exchange. Local publication also checks retained SQL history in the write transaction. It does not clear either store. It records the verified source and Resource ID locally with the restored data. Reopening does not repeat the restore or activate sync.

### Merge independent databases

On a browser with local vaults, select **Find remote database**. Check the Installation issuer, User, Application, Storage URL, and Resource ID. **Decline merge** keeps both databases unchanged. Before approval, the app downloads only a private read-only snapshot; it does not send local history.

**Approve and merge** reconciles the whole shared database, including all vaults and retained deletions. An item whose folder was deleted appears as Unfiled, including in backups. The app applies this rule after decryption and does not rewrite encrypted history to make the item visible.

For concurrent edits to the same item, folder, or lock settings record, `ofdb` selects its canonical ciphertext. It copies the nonce and update metadata from the unique live source that holds that ciphertext. The ciphertext and nonce must have been written together, and the vault identity must agree. Timestamps do not select the winner. The merge keeps both edits in history. Ambiguous or incomplete encrypted records, incompatible history, and unmigrated legacy vaults stop the operation.

Storage must confirm the exact candidate before the app can publish it locally. Local publication checks the original local revision and saves one binding in the same transaction. A completed merge leaves sync detached.

For an ordinary merge, **Retry pending merge** checks the exact Storage receipt and can reuse the same operation and candidate only in the original sign-in session. **Cancel or archive pending merge** asks Storage about the original Resource and operation. Storage either confirms the earlier commit or permanently blocks that operation from publishing. This action sends no history. A fresh sign-in to the same Installation, Storage, Application, and User can use it if current Storage access and Selection permit. A different account cannot use it. The app keeps both live databases, the old operation, and its private candidate. To continue, find the remote database again and approve a new merge. Keep the browser data while recovery is pending. No vault or binding migration is needed.

### Upgrade an older synced database

Sign in to the original Installation and account, then select **Find remote database**. If no current Resource takes precedence, the app checks the old exact creation key. It does not use the display name, create a Resource, or open a second active synced database. If the old Resource has no creation key, or a current Resource is also present, use **Recover older data by Resource ID** and enter the exact ID from Storage. First upload blocks when the signed-in catalog lists any SQL Resource other than the exact first-upload Resource. The app does not use names to decide which Resource to enroll.

Check the destination and select **Approve and upgrade**. A catalog match alone does not identify a vault. The app checks the Storage snapshot, any retained browser replica, and the final private candidate against the supported vault schema and row readers. Exact-ID recovery requires a recognizable vault on Storage. The app accepts the supported migration ledger or the original ledger-free synced schema; it does not fix incomplete or unknown schemas. This check cannot authenticate ciphertext without the vault password.

After validation, the app retires an existing browser replica only if its IndexedDB revision still matches, in the same transaction that sets the terminal revision. It does not reserve an absent replica or change an unrelated Resource. The old database remains readable. This fence does not revoke remote clients. The app can then publish the combined encrypted history, including browser-only edits and deletions, to the original Resource. If interrupted, use **Retry pending merge** to check the receipt and resume. Unlike an ordinary merge, a validated older database upgrade can also resume after a fresh sign-in to the same Installation, Storage, Application, and User. Current Storage authorization and Selection still apply. An upgrade staged before source validation was added must be archived locally and reviewed again. Archiving that upgrade does not contact Storage or report whether the earlier attempt committed. A changed source or unsupported history stops publication without discarding either copy.

Unlock each upgraded vault from the normal selector with its existing password. Successful decryption of its records and lock settings makes **Reattach sync** available after all upgraded vaults have been checked. Sync still needs current authorization. The app does not automatically delete the older browser replica. If history cannot be upgraded or the active database is already linked elsewhere, use the exact Resource ID to open a retained browser copy read-only and export an encrypted backup, even when signed out. That recovery path does not establish a binding, import, or sync data. Without a verified Resource ID, the app does not guess from a name.

For an older local per-vault database, choose its **upgrade older vault** entry in the selector. This is also how to retry verification if a shared copy already exists. Opening the shared vault does not retry that upgrade. Close tabs that still run the older app before upgrading. The app reads the database named in the old registry, including copies created by older item edits. Format-2 vaults require their original password but do not require a pre-transition backup. If one is present, the app validates it and retains it in the original database. A read-only recovery export also includes that backup.

The app validates the source schema, authentication, records, folders, and lock settings, then compares the copy before it retires writes to the validated source revision. A wrong password, invalid source, or copy mismatch does not retire the source. If the source validates but cannot be copied, use its **recovery copy** entry to open it read-only and export an encrypted backup. You can restore the backup in the same profile if the shared database has no vault with that ID. Restore does not change or verify the retained source. The old databases and their history are retained; only active records are copied to shared storage.
