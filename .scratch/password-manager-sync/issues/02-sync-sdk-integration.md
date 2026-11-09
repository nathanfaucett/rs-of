# Ticket 02: Sync SDK Integration

## Type: task
## Status: open

### Description
Integrate `@ofsdk/sync-sdk` into password-manager. Initialize `BrowserSyncSDK` with auth token provider and Storage Service URL.

### Acceptance Criteria
- [ ] Add `@ofsdk/sync-sdk` to package.json dependencies
- [ ] Create `src/lib/sync/sync-client.ts` with `SyncClient` singleton:
  - `initialize(config: SyncConfig): Promise<SyncSDK>` — creates and returns SDK instance
  - `getInstance(): SyncSDK` — returns initialized instance
  - `resetForTesting(): void` — clears singleton for tests
  - Config sourced from auth state: `apiUrl`, `appId`, `userId`, `getToken`
- [ ] `SyncConfig`:
  - `apiUrl`: `import.meta.env.VITE_STORAGE_API_URL` || `https://storage.localhost:1355`
  - `appId`: `"password-manager"`
  - `userId`: from `auth.getUserInfo().sub`
  - `getToken`: `() => auth.getAccessToken()`
- [ ] Initialize sync client after successful authentication
- [ ] Clean up sync client on logout (`sdk.close()`)
- [ ] Export `sync` instance for vault module

### Implementation Notes
- Sync SDK auto-detects browser platform
- `SyncSDK` interface provides `sql`, `kv`, `fs`, `syncToDevice`, `close`
- For password-manager, only `sql` is needed (Phase 1)
- Handle initialization errors gracefully (show user-friendly message)

### Dependencies
- Blocked by: 01 (needs auth token provider)

### Testing
- Unit: `SyncClient` singleton lifecycle, config passing
- Integration: Initialize with valid token, call `sync.sql.list()` → expect empty array
- Error: Invalid token → expect auth error from Storage Service
