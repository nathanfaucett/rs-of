# 07 - Integration test: browser ↔ native sync via Management

**Status: needs-triage**

## Problem

Need end-to-end verification that all pieces work together: browser app creates SQL DB, native app opens it, Management selection enables sync, data replicates.

## Solution

Build minimal test app (or extend password-manager) that:

### Test Scenario

1. **Setup**:
   - Start unified server (IdP + Device + Management + Storage)
   - Create test user, app client
   - Register two Devices: Device-A (browser), Device-B (native)

2. **Browser (Device-A)**:
   - Sign in via IdP
   - `storage.create_sql_db("test-vault")` → gets `SqlDatabaseHandle`
   - Insert test rows via `execute()`
   - `management.select_resource(device_a, vault_db)`

3. **Native (Device-B)**:
   - Sign in via IdP (same user)
   - `storage.open_sql_db(vault_db_id)` → gets `SqlDatabaseHandle`
   - `management.select_resource(device_b, vault_db)`

4. **Verify**:
   - Storage Server sees both selections → starts sync
   - Data appears on Device-B (query returns browser-inserted rows)
   - Insert on Device-B → appears on Device-A
   - Disconnect/reconnect → catch-up works

### Test Implementation

- Use existing `ofdb/tests/browser/src/sql_sync.rs` as model
- Add native counterpart in `ofdb/tests/native/src/sql_sync.rs`
- Run via `cargo test --test integration_sync` (requires running services)

### CI Integration

- Spin up unified server in test container
- Run browser test via Playwright (headless)
- Run native test via cargo
- Verify bidirectional sync

## Acceptance Criteria

1. Browser creates DB, inserts data
2. Native opens same DB (by ID)
3. Management selection on both devices
4. Data replicates bidirectionally
5. Reconnect after disconnect works
6. Runs in CI

## Dependencies

- Tickets 01-06 (all pieces implemented)

## Comments
