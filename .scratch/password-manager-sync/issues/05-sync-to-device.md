# Ticket 05: Sync to Device — Multi-Device Vault Access

## Type: task

## Status: done

### Description

Implement `syncToDevice` flow: allow user to select a target device and initiate sync session via Storage Service. Requires Management Service device selection (admin action).

### Acceptance Criteria

- [x] Add `@ofsdk/management-client` to package.json
- [x] Create `SyncVaultHandle.syncToDevice(targetDeviceId)` method:
  - Delegates to `sync.syncToDevice(vaultId, targetDeviceId)`
- [x] Add UI in vault sidebar (`src/lib/vault/components/SyncStatus.svelte`):
  - "Sync to Device" section
  - Input for target device ID
  - Button to start sync
  - Show sync session status
- [x] Handle Management Service pre-requisite:
  - Admin must select both source and target device for the vault resource
  - If not selected, show message: "Contact admin to enable sync for this device"
- [x] Store last synced device for quick re-sync

### Implementation Notes

- Management Service API (via `@ofsdk/management-client`):
  - `listDevices()` — returns devices with `id`, `name`, `type`, `status`
  - `getResourceSelections(resourceId)` — returns selected device IDs
  - Admin selects via Management UI (separate app)
- Storage Service `syncToDevice`:
  - Creates sync session between source replica (this device) and target replica
  - Returns `SyncSession` with `sessionId`, `status`
- Sync SDK `syncToDevice` wraps this REST call
- Phase 1: One-way sync initiation (this device → target). Bidirectional happens automatically once connected.

### Dependencies

- Blocked by: 02 (needs SyncSDK), 04 (needs vault UI)
- Requires: Management Service deployed and configured

### Testing

- Integration: Admin selects two devices for vault → user initiates sync → session created → data replicates
- Error: No device selection → clear error message
- E2E: Two browsers (different device IDs) → sync vault → verify data on both
