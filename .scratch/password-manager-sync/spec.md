# Spec: Password Manager Sync Integration

## Problem Statement

The password-manager app currently uses `@ofsdk/sql` directly for local-only IndexedDB storage via a `SharedDb` singleton. It has no authentication, no sync capability, and no connection to the Storage Service or Management Service.

Users cannot:
- Access their vault on multiple devices
- Have changes sync automatically between devices
- Use Management Service to control which devices store their vault data

## Solution

Integrate `@ofsdk/sync-sdk` into the password-manager to provide:
1. **OIDC Authentication** — Sign in via IdP to get access tokens
2. **Synced SQL Database** — Replace `SharedDb` + raw `SqlDatabase` with `sync-sdk` `DatabaseHandle`
3. **Automatic Sync** — WebSocket sync to Storage Service with reactive `changes$` stream
4. **Device Sync Control** — `syncToDevice()` to connect vault to another approved device
5. **Management Integration** — Use `@ofsdk/management-client` for device/resource selection (future)

## User Stories

1. As a password-manager user, I want to sign in with my IdP account so the app can authenticate with Storage Service
2. As a password-manager user, I want to create a vault that automatically syncs to my other devices so I see the same data everywhere
3. As a password-manager user, I want to see sync status (connecting/synced/disconnected/error) so I know if my data is up to date
4. As a password-manager user, I want remote changes to appear in real-time via reactive streams so I don't need to refresh
5. As an admin, I want to use Management Service to select which devices can store this vault so I control data residency
6. As a developer, I want minimal code changes — replace `SharedDb.getClient()` with `sync.sql.open()` and get sync for free

## Current Architecture (Before)

```
Password Manager
    │
    ├─► @ofsdk/sql (WASM) ──► IndexedDB (local only)
    │
    └─► SharedDb singleton (manages single SqlDatabase)
            └─► VaultHandle (wraps Client, encrypts/decrypts per-vault)
```

## Target Architecture (After)

```
Password Manager
    │
    ├─► @ofsdk/oidc-client ──► IdP (Keycloak) ──► Access Token
    │
    ├─► @ofsdk/sync-sdk (BrowserSyncSDK)
    │       │
    │       ├─► @ofsdk/storage-client ──► Storage Service REST
    │       │       └─► create_sql_db / open_sql_db / sync_to_device
    │       │
    │       ├─► @ofsdk/storage-sync-client ──► Storage Service WS /sync/ws
    │       │       └─► Bidirectional sync protocol (Attach, frames)
    │       │
    │       └─► @ofsdk/sql (WASM) ──► IndexedDB (local replica)
    │               └─► SqlDatabase.synchronize(JsSqlSyncTransport)
    │
    └─► VaultHandle (wraps DatabaseHandle, adds per-vault encryption)
            └─► execute / upsert / delete / changes$ / state$ / sync()
```

## Implementation Decisions

### 1. Authentication Layer
- Add `@ofsdk/oidc-client` + `@ofsdk/idp-client` dependencies
- Implement OIDC login flow (redirect to IdP, handle callback, store tokens)
- Provide `getToken(): Promise<string>` to sync-sdk config
- Store tokens in memory (or secure storage), refresh on expiry

### 2. Sync SDK Integration
- Add `@ofsdk/sync-sdk` dependency
- Initialize `BrowserSyncSDK` with:
  - `apiUrl`: Storage Service URL (e.g., `https://storage.localhost:1355`)
  - `appId`: `"password-manager"`
  - `userId`: from OIDC userinfo
  - `getToken`: function returning current access token

### 3. Database Handle Replacement
Replace `SharedDb` + `VaultHandle` pattern:
- **Before**: `SharedDb.getInstance().getClient()` → raw `Client` → manual SQL
- **After**: `await sync.sql.open(databaseId)` → `DatabaseHandle` → `execute()`, `upsert()`, `delete()`, `changes$`, `state$`, `sync()`

Each vault gets its own `DatabaseHandle` (resourceId = vaultId). The `DatabaseHandle` wraps:
- Local `SqlDatabase` (WASM/IndexedDB)
- `StoreIdentity` for sync protocol
- Reactive streams for changes and sync state

### 4. VaultHandle Refactor
`VaultHandle` becomes a thin wrapper around `DatabaseHandle`:
- Keeps per-vault encryption key (derived from password)
- `execute(sql, params)` → decrypts results, encrypts inputs
- `upsert(doc)`, `delete(id)` → encrypts payload, calls `handle.upsert/delete`
- `changes$` → maps remote events to decrypted secrets/folders
- `sync()` → delegates to `handle.sync({ direction: "bidirectional" })`
- `syncToDevice(targetDeviceId)` → delegates to `sync.syncToDevice(vaultId, targetDeviceId)`

### 5. Schema Compatibility
- Existing schema (vaults, secrets, folders, lock_settings tables) works unchanged
- `DatabaseHandle.execute()` accepts same SQL/params as `Client.execute()`
- Migrations run on first open via `SqlDatabase` (same as before)

### 6. Sync Flow (Phase 1: SQL over WebSocket)
1. User signs in → gets access token
2. App calls `sync.sql.create("vault-{name}")` or `sync.sql.open(vaultId)`
3. Storage Service creates/returns resource with `storeId`, `replicaId`
4. App opens local `SqlDatabase` with resourceId as name
5. App calls `handle.sync()` → attaches to Storage Service WebSocket
6. Storage Service runs sync session with server-side replica (authoritative)
7. Changes flow bidirectionally via framed protocol
8. `changes$` emits `local-upsert` / `remote-upsert` / `local-delete` / `remote-delete`
9. Admin uses Management Service to select target device for resource
10. App calls `sync.syncToDevice(vaultId, targetDeviceId)` → connects to peer

## API Changes

### New Dependencies (package.json)
```json
{
  "dependencies": {
    "@ofsdk/sync-sdk": "workspace:*",
    "@ofsdk/oidc-client": "workspace:*",
    "@ofsdk/idp-client": "workspace:*",
    "@ofsdk/management-client": "workspace:*"
  }
}
```

### New Files
- `src/lib/auth/oidc.ts` — OIDC client setup, login/logout, token management
- `src/lib/auth/auth.svelte.ts` — Svelte 5 state for auth (user, token, login status)
- `src/lib/vault/sync-vault.ts` — `SyncVaultHandle` wrapping `DatabaseHandle`
- `src/lib/shared/state/sync.svelte.ts` — Global sync state (optional)

### Modified Files
- `package.json` — Add dependencies
- `src/lib/vault/vault.ts` — Replace `SharedDb` usage with `SyncVaultHandle`
- `src/lib/shared/state/vault.svelte.ts` — Use `SyncVaultHandle`, add sync status
- `src/routes/(app)/vault/*` — Show sync status indicator

### Removed/Deprecated
- `src/lib/vault/storage/SharedDb.ts` — Replace with sync-sdk managed databases
- Direct `@ofsdk/sql` imports in vault code (keep for types/migrations only)

## Testing Decisions

- **Unit**: `SyncVaultHandle` encrypts/decrypts correctly, maps `changes$` events
- **Integration**: 
  - Sign in → create vault → sync → verify data in Storage Service
  - Two browser contexts → create vault on one → sync to other → verify data appears
  - Offline → make changes → online → verify catch-up sync
- **E2E**: Playwright test: login, create vault, add secret, sync to second device, verify

## Out of Scope

- Device pairing UI (handled by `@ofsdk/app`)
- Management Service device selection UI (handled by `@ofsdk/app`)
- KV/Filesystem sync (Phase 2)
- Conflict resolution UI (app shows conflict, user resolves)
- Schema migration (handled by ofdb-sql locally)

## Further Notes

- This spec connects existing working pieces: `@ofsdk/sync-sdk`, `@ofsdk/storage-client`, `@ofsdk/sql` (WASM), Storage Service
- Phase 1: SQL sync only via WebSocket (browser). Native/Tauri uses Iroh transport (separate)
- Single app-facing seam: `sync-sdk` `SyncSDK` interface
- The `SyncVaultHandle` pattern can be reused by other apps needing per-resource encryption
