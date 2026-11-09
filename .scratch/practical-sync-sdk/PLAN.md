# Practical Sync SDK — Implementation Plan for Remaining Issues

## Summary

Three remaining issues from the spec that need fixing:

1. **Sync Server relay** — `authorize_sync()` returns `target_device_id` but `run_sync_session()` ignores it; server only syncs against local DB
2. **Migration verification** — `20250101000000_sync_selections.up.sql` exists but needs confirmation it runs
3. **Integration test (Ticket 07)** — Browser app ↔ native app sync via Management selection

---

## Issue 1: Sync Server Relay

### Current State
- `authorize_sync()` in `of/crates/storage-server/src/sync.rs` queries Management for target device and returns `SyncAuth { target_device_id, ... }`
- `run_sync_session()` receives `target_device_id` but **does nothing with it** — only syncs against local database
- `sync_connect` endpoint in `resources.rs` returns mock session, doesn't initiate real sync

### Root Cause
The architecture assumes the storage server holds the **authoritative replica**. Browser clients sync *to* the server via WebSocket. The server doesn't relay to other devices.

### Decision: Option B (Authoritative Replica) — No Relay Needed for Phase 1

**Rationale:**
- Current code already implements this: server has canonical DB, clients sync to it
- `target_device_id` in `SyncAuth` is logged for admin visibility (already done)
- Iroh mesh handles native↔native peer sync separately
- Browser→browser sync goes through server (server is the "other device")
- Phase 1 scope: SQL over WebSocket only; KV/offs later

**Changes Required:**
1. **Document the architecture** — add comment in `sync.rs` explaining server is authoritative replica
2. **Fix `sync_connect` endpoint** — make it actually verify Management selection and return real session info (not mock)
3. **Remove unused `target_device_id` from `run_sync_session`** — or keep for logging only
4. **Update `SyncAuth` docs** — clarify `target_device_id` is informational for Phase 1

### Files to Modify
- `of/crates/storage-server/src/sync.rs` — document, remove unused param from `run_sync_session`
- `of/crates/storage-server/src/router/resources.rs` — fix `sync_connect` to verify selection

---

## Issue 2: Migration Verification

### Current State
- Migration file exists: `of/crates/idp-model/replica_migrations/20250101000000_sync_selections.up.sql`
- `idp_model::replica::up()` iterates over **all files** in `replica_migrations` directory via `include_dir!`
- File will be picked up automatically

### Verification Needed
1. Confirm migration runs by checking `MIGRATIONS` dir includes the file
2. Test that `up()` executes it (can verify via unit test or manual run)

### Files to Check
- `of/crates/idp-model/src/replica.rs` — already correct, no changes needed
- Migration file already exists

**Action:** Add a test or document that migration runs automatically. No code changes needed.

---

## Issue 3: Integration Test (Ticket 07)

### Requirements
- Browser app creates SQL DB via `storage.createSqlDatabase("vault")`
- Native app opens same DB via `storage.openSqlDatabase(id)`
- Management selection on both devices via `POST /management/devices/{id}/sync-selections`
- Verify bidirectional data replication
- Test reconnect/catch-up

### Test Strategy
Use existing patterns from `ofdb/tests/browser/src/sql_sync.rs`

### Components Needed
1. **Test harness** — spin up Storage Service, Management Service, IDP
2. **Browser client** — use `@ofsdk/storage-client` + `@ofsdk/sql` (WASM)
3. **Native client** — use `of-storage-client` (Rust) + `ofdb-sql` (native)
3. **Management client** — call sync selection APIs
4. **Assertions** — data written on one side appears on other

### Files to Create
- `of/tests/integration/sync_e2e.rs` (Rust native test)
- `of/tests/integration/sync_e2e_browser.test.ts` (WASM/browser test via wasm-bindgen-test or playwright)
- Or single test using both clients against running services

---

## Additional Fixes Found During Exploration

### Management Server sync_selection Routes Have Bugs
`of/crates/management-server/src/router/routes/sync_selection.rs`:
- Line 33: `let device_id = request.resource_id;` — **BUG**: uses request body instead of path param
- Lines 45-46: `Id::nil()`, `""` — TODOs for application_id and owner_subject
- Missing authorization checks

### Files to Fix
- `of/crates/management-server/src/router/routes/sync_selection.rs` — fix device_id parsing, add auth context

---

## Implementation Order (Dependencies)

```
1. Fix management-server sync_selection routes (blocking for integration test)
   ↓
2. Document Sync Server authoritative architecture (no code relay needed)
   ↓
3. Verify migration runs (documentation/test)
   ↓
4. Integration test (Ticket 07)
```

---

## Verification Commands

```bash
# 1. Check Rust sync crates compile
cd of && cargo check -p of-storage-client -p storage-sync-client -p storage-server -p management-service -p management-server

# 2. Verify migration is included
cd of && cargo test -p idp-model replica::up -- --nocapture 2>&1 | grep -i sync_selections

# 3. TypeScript builds (already passing)
cd of && pnpm --filter @ofsdk/storage-client build:esm
cd of && pnpm --filter @ofsdk/sync-sdk build

# 4. Run integration test (after implementation)
cd of && cargo test -p of-storage-client sync_e2e -- --nocapture
```
