# Ticket 04: Vault State Integration — Connect Sync to UI

## Type: task
## Status: open

### Description
Update `vault.svelte.ts` state machine to use `SyncVaultHandle` and expose sync status to UI. Add sync indicator in vault views.

### Acceptance Criteria
- [ ] Update `src/lib/shared/state/vault.svelte.ts`:
  - Replace `SharedDb` usage with `SyncClient.getInstance().sql.open/create`
  - `createVault(password, name)` → `sync.sql.create("vault-" + name)` → `SyncVaultHandle` → start sync
  - `unlockVault(password, id)` → `sync.sql.open(id)` → `SyncVaultHandle` → start sync
  - Add `syncStatus` derived from `vault.state$` (connecting/synced/disconnected/error)
  - Add `syncError` derived from `vault.state$.error`
  - Subscribe to `vault.changes$` for real-time updates (optional: auto-refresh UI)
- [ ] Update `VaultState` type to include sync info:
  - `unlocked` state includes `syncStatus`, `syncError`
- [ ] Update `src/routes/(app)/vault/` components:
  - Show sync status indicator (icon: spinning/synced/offline/error)
  - Tooltip with details: last sync, peer count, errors
  - Manual "Sync Now" button (calls `vault.sync()`)
- [ ] Handle sync errors: show notification, allow retry
- [ ] Handle conflicts: show conflict resolution UI (future ticket, for now log)

### Implementation Notes
- Sync starts automatically after vault unlock (call `vault.sync({ direction: "bidirectional" })`)
- `changes$` can drive reactive UI updates (e.g., TanStack DB integration)
- For now, manual refresh on `remote-upsert` is acceptable
- Sync status mapping:
  - `connecting` → spinner
  - `synced` → green check
  - `disconnected` → gray cloud with slash
  - `error` → red alert

### Dependencies
- Blocked by: 03 (needs SyncVaultHandle)

### Testing
- Integration: Unlock vault → verify sync starts → status goes to "synced"
- E2E: Two browser tabs → unlock same vault → add secret in one → verify appears in other
- Visual: Sync indicator shows correct state transitions
