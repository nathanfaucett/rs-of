# Ticket 02: Sync SDK Integration

## Type: task

## Status: done

### Description

Integrate `@ofsdk/sync-sdk` into password-manager. Initialize `BrowserSyncSDK` with auth token provider and Storage Service URL.

### Acceptance Criteria

- [x] Add `@ofsdk/sync-sdk` to package.json dependencies
- [x] Create `src/lib/shared/state/sync.svelte.ts` with `SyncStateModule`:
  - `initializeSync(): Promise<SyncSDK>` — creates and returns SDK instance
  - `getSyncSDK(): SyncSDK` — returns initialized instance
  - `closeSync(): void` — closes SDK and cleans up
  - Config sourced from auth state: `apiUrl`, `appId`, `userId`, `getToken`
- [x] `SyncConfig`:
  - `apiUrl`: `import.meta.env.PUBLIC_STORAGE_URL` || `https://storage.localhost:1355`
  - `appId`: `"password-manager"`
  - `userId`: from `auth.getUserInfo().sub`
  - `getToken`: `() => auth.getAccessToken()`
- [x] Initialize sync client after successful authentication
- [x] Clean up sync client on logout (`sdk.close()`)
- [x] Export `createDatabase`, `openDatabase`, `listDatabases`, `syncToDevice` for vault module

### Implementation Notes

- Sync SDK auto-detects browser platform
- `SyncSDK` interface provides `sql`, `kv`, `fs`, `syncToDevice`, `close`
- For password-manager, only `sql` is needed (Phase 1)
- Handle initialization errors gracefully (show user-friendly message)

### Dependencies

- Blocked by: 01 (needs auth token provider)

### Testing

- Unit: `SyncStateModule` lifecycle, config passing
- Integration: Initialize with valid token, call `sync.sql.list()` → expect empty array
- Error: Invalid token → expect auth error from Storage Service
