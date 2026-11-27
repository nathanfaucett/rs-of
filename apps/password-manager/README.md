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

Each exchange requires current Storage authorization and Selection. Edits stay local if sync fails. **Detach sync** cancels the exchange and keeps both copies. Signing out cancels sync and locks opened vaults without waiting for database access. If the browser cannot clear saved credentials, the app reports incomplete sign-out. Fix browser storage and retry before reloading. If no persistent change succeeds, reload can restore the old login and receipt. No background upload starts without explicit activation. **Retry local save** never activates sync. This is local sign-out, not IdP session termination or token revocation.

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

Local and legacy synced vaults use the same encryption, unlock, and editing code. They still use separate browser databases. This shared code does not migrate vaults or change their IDs, master passwords, lock settings, or Storage Resource bindings. Synced vaults still accept legacy SQL timestamp text for item, folder, and lock-setting rows without rewriting it. Local vaults still require ISO timestamps in those rows.

**Create new vault** always creates a local vault. Sign-in does not unlock a vault or upload its data.

To upload the local database, lock the vault and select **Connect local database**. Check the Installation issuer, User subject, and Application URI. Select **Confirm and upload** to register or reopen the same logical Storage Resource and try a guarded first upload. Storage must authorize the upload.

If Storage denies the upload, check whether the Resource is selected on the serving Node’s approved Device. Use the displayed Resource ID. The current [Device route](../../openapi/device.json) requires the serving Device owner to set Selection. That same User needs Storage read access to the Resource and the Application’s `devices.select` permission. An Installation operator cannot select the Resource just by being an operator. Password Manager does not set Selection or grant access. After authorization is in place, select **Retry first upload**.

The first upload uses the existing browser database. If the publish receipt is lost, Storage may already have saved the upload. Keep using the local vaults. A retry uses the same Resource and still requires an empty destination. If Storage reports existing history, the app stops: it cannot verify an earlier upload and does not merge, overwrite, or fall back to an unguarded exchange. First upload does not activate background sync. Vault names and other metadata are not encrypted by the vault record encryption.

Use **Show synced vaults** to reopen existing legacy synced vaults. This path still requires paired `/idp` and `/storage` bases on the same server. It does not open the new first-upload Resource as a legacy vault.

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

The local publish transaction accepts only a database with no retained SQL history. A provisioned schema or a deleted vault is not a fresh database. Restore does not clear either store or import the separate legacy synced-vault store. It records the verified source and Resource ID locally with the restored data. Reopening does not repeat the restore or activate sync. The legacy selector remains available.
