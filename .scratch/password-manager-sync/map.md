# Map: Password Manager Sync Integration

## Notes

- **Goal**: Add automatic multi-device sync to password-manager using `@ofsdk/sync-sdk`
- **Current state**: Local-only, no auth, uses `@ofsdk/sql` directly via `SharedDb` singleton
- **Target**: OIDC auth + sync-sdk → synced vaults with reactive updates
- **Key packages**: `@ofsdk/sync-sdk`, `@ofsdk/oidc-client`, `@ofsdk/idp-client`, `@ofsdk/storage-client`, `@ofsdk/management-client`
- **Backend services**: Storage Service (REST + WS), IdP (Keycloak), Management Service

## Decisions-so-far

1. **Auth first** — Ticket 01 (OIDC) must complete before sync-sdk init (Ticket 02)
2. **SyncVaultHandle wraps DatabaseHandle** — Not replace; keeps encryption logic, adds sync
3. **Phase 1: SQL only** — KV/Filesystem sync later (Phase 2)
4. **WebSocket transport** — Browser uses WS to Storage Service; Native (Tauri) uses Iroh (separate)
5. **Server-authoritative** — Storage Service runs server-side replica; clients sync to it
6. **No polling** — Reactive via WebSocket push (`changes$` Observable)
7. **Management controls sync** — Admin selects devices per resource; user initiates `syncToDevice`
8. **Migration path** — Existing local vaults become synced on first unlock after update

## Tickets

| # | Ticket | Status | Blocked By |
|---|--------|--------|------------|
| 01 | OIDC Authentication | open | — |
| 02 | Sync SDK Integration | open | 01 |
| 03 | SyncVaultHandle | open | 02 |
| 04 | Vault State Integration | open | 03 |
| 05 | Sync to Device | open | 02, 04 |
| 06 | Cleanup & Testing | open | 03, 04, 05 |

## Frontier

**Next up: Ticket 01** (no dependencies)

## Fog

- Exact IdP config (Keycloak realm, client registration) — need to verify local setup
- Management Service device selection API shape — check `@ofsdk/management-client` generated types
- Conflict resolution UI design — deferred to later iteration
- Tauri/Iroh sync — separate effort, not in this spec
- Offline-first conflict handling — vector clocks in sync-sdk, but UI for manual resolve TBD
