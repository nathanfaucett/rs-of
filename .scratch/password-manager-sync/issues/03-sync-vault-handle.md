# Ticket 03: SyncVaultHandle — Replace SharedDb + VaultHandle

## Type: task
## Status: open

### Description
Create `SyncVaultHandle` that wraps `DatabaseHandle` from sync-sdk, adding per-vault encryption. Replace `SharedDb` singleton and `VaultHandle` class.

### Acceptance Criteria
- [ ] Create `src/lib/vault/sync-vault.ts` with `SyncVaultHandle` class:
  - Constructor takes `DatabaseHandle`, `encryptionKey`, `vaultId`, `vaultMeta`
  - Implements same public API as current `VaultHandle`:
    - `id`, `loadSecrets()`, `loadFolders()`, `createFolder()`, `renameFolder()`, `moveItems()`, `deleteFolder()`
    - `decryptSecret()`, `encryptAndStore()`, `deleteSecret()`
    - `exportBackup()`, `loadLockSettings()`, `saveLockSettings()`
    - `readItems()`, `prepareItems()`, `mutateItems()`
    - `close()`
  - **New**: `changes$` — Observable<ChangeEvent> (maps remote events to decrypted items)
  - **New**: `state$` — Observable<SyncState> (sync status: connecting/synced/disconnected/error)
  - **New**: `sync(options?)` — starts bidirectional sync
  - **New**: `syncToDevice(targetDeviceId)` — delegates to `sync.syncToDevice(vaultId, targetDeviceId)`
- [ ] Encryption/decryption logic unchanged from current `VaultHandle` (reuse `crypto.ts`)
- [ ] `execute(sql, params)` → calls `handle.execute()`, decrypts result rows
- [ ] `upsert(doc)` / `delete(id)` → encrypts payload, calls `handle.upsert/delete`
- [ ] `changes$` subscription:
  - `remote-upsert` → decrypt, emit parsed Secret/Folder
  - `remote-delete` → emit deleted ID
  - `conflict` → emit conflict event for UI handling
- [ ] `state$` subscription → update UI sync indicator

### Implementation Notes
- `DatabaseHandle` from `@ofsdk/sync-sdk` has:
  - `execute(sql, params)` → returns `{ columns, rows }`
  - `upsert(doc)`, `delete(id)` — high-level document API
  - `changes$` — Observable<ChangeEvent> (local-upsert, remote-upsert, local-delete, remote-delete, ack, conflict)
  - `state$` — Observable<SyncState> (status, version, vectorClock, error)
  - `sync(options)` — starts sync session
  - `close()` — closes handle
  - `id` — resource ID (vaultId)
- Current `VaultHandle` uses `Client.execute()` with row mappers. `DatabaseHandle.execute()` returns parsed JSON.
- Migration: Keep `vault.ts` exports (`createVault`, `unlockVault`, `listVaults`) but implement using `SyncVaultHandle`
- `SharedDb` can be removed once all vault operations use sync-sdk

### Dependencies
- Blocked by: 02 (needs initialized SyncSDK)

### Testing
- Unit: `SyncVaultHandle` encrypts/decrypts correctly, maps change events
- Integration: Create vault → add secret → verify `changes$` emits local-upsert → sync → verify remote-upsert on other client
- Migration: Existing vaults open correctly (same schema, same encryption)
