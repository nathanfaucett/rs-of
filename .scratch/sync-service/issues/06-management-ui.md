# 06: Management UI — Sync Capability Indicator

**Status:** `done`
**Type:** `task`
**Blocked by:** 03

## Description

Existing management UI already has device selection and residency controls. This ticket adds a small indicator showing which devices have the `sync` feature enabled (can serve `wss://.../storage/sync/ws`).

## What it delivers

- Device list shows "Sync: Yes/No" badge per device
- Based on device-server health or storage-server `/health` with `sync` feature flag
- No new backend APIs -- reuse existing device list + selection UI

## Scope

- `of/apps/app/src/lib/common/state/deviceClient.svelte.ts` -- add `getSyncCapability(deviceId)` or read from device health
- `of/apps/app/src/lib/common/components/DevicePolicy.svelte` -- add sync badge column
- Reuses existing selection (`PUT /devices/{id}/selection`) and residency (`PUT /filesystems/{id}/residency`) controls

## Acceptance Criteria

- [ ] Device list shows which devices can serve sync WebSocket
- [ ] Select resource on sync-capable device -> app can connect and sync
- [ ] Deselect resource -> device stops syncing (existing behavior)
- [ ] Passthrough toggle works (existing behavior)

## Notes

- Selection UI already in `DevicePolicy.svelte` using `@ofsdk/device-client`
- Residency UI already in `ResourceResidency.svelte` using `@ofsdk/storage-client`
- Token exchange for sync uses existing `StorageClient.exchangeToken(["read","write"], signal, "storage:sync")` pattern
- No backend changes
