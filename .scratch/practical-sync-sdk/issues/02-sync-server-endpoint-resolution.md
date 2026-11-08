# 02 - Sync Server: resolve endpoint via Management selection

**Status: needs-triage**

## Problem

`of/crates/storage-server/src/sync.rs` `authorize_sync()` uses hardcoded `local` endpoint lookup. WebSocket sync cannot connect to the correct target Device because it doesn't query Management for Device selection.

## Solution

Modify `authorize_sync()` in `sync.rs`:

1. On `AttachRequest`, extract `resource_id` (database ID) from `store_identity.store_id`
2. Call Management Service to get selected target Device for this resource:
   - Query: "Which approved Device has selection for this resource_id?"
   - Returns: target Device endpoint ID (iroh NodeAddr) or none
3. If no target selected → reject with `AbortReason::BackendUnavailable`
4. If target found → resolve endpoint via Device Service / iroh-chain
5. Pass resolved endpoint to `run_sync_session()`

### Management Query Interface

Add trait in `of/crates/storage-service`:
```rust
#[async_trait]
pub trait ManagementSyncSelection {
    async fn get_sync_target(&self, resource_id: ResourceId, source_device: DeviceId) 
        -> Result<Option<DeviceEndpoint>>;
}
```

Storage Service implements this via internal call to Management Service.

### Server Changes

- `authorize_sync()` → calls `ManagementSyncSelection::get_sync_target()`
- `run_sync_session()` → receives resolved endpoint, connects via Iroh
- Remove hardcoded `local` lookup entirely

## Acceptance Criteria

1. WebSocket sync connects to target Device endpoint from Management selection
2. No selection → sync rejected (not silently using `local`)
3. Both source and target Devices must have selection for resource
4. Works for SQL databases (Phase 1)

## Dependencies

- Ticket 01 (Storage Client calls sync_to_device)
- Ticket 03 (Management exposes selection API)

## Comments
