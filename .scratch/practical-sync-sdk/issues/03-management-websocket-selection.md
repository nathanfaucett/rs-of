# 03 - Management Service: WebSocket sync selection API

**Status: needs-triage**

## Problem

Management Service only handles resource selection for Iroh mesh sync. WebSocket sync (browser apps) needs the same selection API so admins control which devices sync via WebSocket.

## Solution

Extend Management Service with WebSocket sync selection:

### API (of/crates/management-service)

```rust
// New endpoints (parallel to existing Iroh selection)
POST   /management/devices/{device_id}/sync-selections
    Body: { resource_id: ResourceId, select: bool }
    → selects/deselects resource for WebSocket sync on this device

GET    /management/devices/{device_id}/sync-selections
    → returns list of selected resource_ids for WebSocket sync

GET    /management/resources/{resource_id}/sync-targets
    → returns list of DeviceIds that have this resource selected for WebSocket sync
```

### Storage

- Add `sync_selections` table (device_id, resource_id, selected_at)
- Separate from `iroh_selections` but same admin restrictions apply
- Single source of truth: a Device either has selection or not (shared)

### Authorization

- Same as Iroh selection: requires Management permission + Device ownership
- Admin restrictions apply to both transports
- Revoked Device → selections auto-cleared (existing cleanup)

### Sync Server Integration

- `authorize_sync()` calls Management to check:
  - Source Device has selection for resource
  - Target Device has selection for resource
  - Both Devices approved + not revoked

## Acceptance Criteria

1. Admin can select resource for WebSocket sync on a Device via API
2. Selection persists and survives restart
3. Sync Server can query selection state for WebSocket sync
4. Admin restrictions apply to WebSocket sync selections
5. Revoked Device loses WebSocket sync selections

## Dependencies

- Ticket 02 (Sync Server needs this API)

## Comments
