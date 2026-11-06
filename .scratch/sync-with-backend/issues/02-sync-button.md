# 02: Sync button with setup flow, backup target picker, and sync status

**What to build:** The user-facing sync feature in the app, built on the
TS sync client from ticket 01. A Sync button on the dashboard/Devices
page uses the already configured storage service when its sync capability
probe passes; otherwise it offers a setup flow (enter a storage service
URL, or enroll/pair a Device via the existing pairing flow). The backup
picker lists the User's resources and trusted Devices, pre-checking
Devices that already have a Resource Selection for the resource, and runs
the chosen sync direction to each selected target with visible progress
and result. Last-sync status per Device is recorded and shown on the
Devices page, and the sync target configuration persists across restarts.

**Blocked by:** 01 (TS WebSocket sync client package)

**Status:** open

- [ ] Sync button runs immediately against the configured storage service when the capability probe reports sync support
- [ ] When no capable sync service is configured, the button opens a setup flow (storage URL entry or Device pairing) and then syncs
- [ ] Backup picker lists resources and trusted Devices, pre-checking existing Resource Selections, and honors per-run target changes
- [ ] Sync runs to each selected target with progress and success/error result visible to the User
- [ ] Unreachable or rejecting sync service produces a clear error and leaves local data untouched
- [ ] Revoked Devices are not offered as targets and stop receiving data
- [ ] Last-sync status per Device persists across restarts and is shown on the Devices page
- [ ] UI behavior covered by app-level interaction tests (mounted component, mocked fetch/WebSocket) asserting what the User sees and which requests are made

## Comments

- Claimed by agent on `implement/sync-app`; base `dce98e290`. Investigating real owner/driver and authorization gaps before UI integration.

## Answer

**Partial implementation only. Ticket remains open.** No resource data has
been transferred or qualified by this change. None of the original acceptance
items is marked complete merely because setup UI exists.

### Implemented safe parts

- Devices-page Sync check/setup panel; saved explicit HTTPS Storage API override
  or existing Storage API configuration. The capability probe omits credentials,
  rejects redirects, and accepts cancellation. Setup links to existing pairing.
- Current signed-in Application resource discovery uses the real `StorageClient`
  and its read-only token exchanges, not fake local databases. Catalog records
  are explicitly not local-copy evidence. An override receives only the unauthenticated
  capability probe; resource discovery remains on the configured source API.
- Approved, non-revoked Device preferences; trust is checked again before saving.
  Unknown Resource Selections remain unknown, not unchecked policy or grants.
- Per-Device **blocked** check results, direction preferences, and target choices
  persist, scoped to issuer/User/source/destination. Session/target changes deny
  stale requests. No tokens, WebSockets, fabricated owner identities, or success
  results are persisted. No attach is attempted without a compatible owner.
- Mounted interaction tests cover discovery/request scopes, unsafe URLs,
  persistence/remount, User isolation, session changes, revocation, and override
  isolation. Progress describes checks, not data transfer.
- WS authorization now checks exact endpoint, owner, Application, database kind
  and ID, administrative permission, pending cleanup, access-token use and time.
  Current authorization is checked before reads/sends and SQL apply. Unknown
  owner protocols are rejected. A real HTTP owner-policy fixture reproduces the
  old owner mismatch and tests current denial/malformed-policy cases.
- Removed the invalid JSX-style per-Device capability badge and its repeated
  Device-address probes. Only the configured Storage API is probed by Sync.

### Investigation and remaining blockers

1. **Real local owner:** `storage-db-collection/src/adapters/ofdb.ts` accepts an
   injected KV-shaped client; it does not open/publish an engine. The app has no
   persistent browser SQL/KV/filesystem owner binding. `ofdb` has the real SQL
   IndexedDB owner and checked sync APIs, but no app JS/WASM publication here.
   Generating fake store identity or replaying TS records would not be a backup.
   Publish the real owner, persistent identity, cancellation, apply guards and
   full completion contract; do not implement CRDT in TS.
2. **Direction and coverage:** WS still starts only default unscoped SQL
   Bidirectional sessions. Push/Pull requires complementary scoped SQL owners
   on both ends; KV and full filesystem content need their own dispatch/driver.
   The Rust WS client/harness still differs from the real server protocol.
   No Push/Pull/Bidirectional resource transfer is delivered by this change.
3. **Native driver:** the Tauri `NativeHost` runs `SetupHost`; the real existing
   `StorageReplicationRuntime` already owns policy-driven SQL+KV/filesystem iroh
   workers and session arbitration. There is no authenticated User command with
   a manual-run completion/progress contract exposed to Tauri. A safe driver must
   route exact resource/Device requests into these existing workers, preserve
   admission, per-frame/per-apply checks, cancellation and concurrent-session
   arbitration, and prove full file-content availability. No second endpoint,
   raw filesystem copy, or bypass command was added. This work is not completed.
4. **Policy directory and management view:** the public Device API exposes
   selection mutation, not an authorized User-readable selection directory;
   the internal selected-resources route uses Storage service credentials.
   DeviceInfo also does not expose resource-owner bindings to the picker.
   Do not send service secrets to the browser or treat local preferences as
   Resource Selections. Add the owner read contract and generated client before
   implementing policy prechecks, resource×Device selection, or the Applications
   resource-to-Device view. That Applications view is not implemented.
5. **WS admission and identity:** `local` is still a hard-coded endpoint, not a
   verified current Device binding. Attach still supplies a random backend UUID
   before opening the real owner. The public login Client's allowed exchange to
   `storage:sync` is not qualified. These require coordinated owner/server work,
   not relaxed auth. Added current-policy checks are not live-transfer proof.
6. **Qualification:** add two real persistent owners and the real router/iroh
   driver; prove Push, Pull and Bidi with committed data after reopen, SQL+KV,
   file metadata **and content**, rejection/unreachable behavior, and learned
   revocation. Earlier atomic applies can remain after a later failure; do not
   promise whole-session rollback. Only owner completion can create a success
   timestamp. The panel currently records local blocked checks, not last sync.

### Change assessment

Moderate UI/auth risk; no data migration or new runtime dependencies. No local
resource writes or Device policy mutations are issued by the new panel. Server
changes are fail-closed policy checks, not a new authorization model. Native
owner/driver work and real transfer are deliberately not claimed complete.

### Validation

Passed:

- `pnpm exec vitest run --config vitest.residency.config.ts` in `apps/app`:
  25 mounted interaction tests, including 6 new sync tests.
- `pnpm exec vitest run --config vitest.auth.config.ts` in `apps/app`:
  116 tests.
- App `check`: 0 errors / 0 warnings. App production `build`: passed after
  building existing workspace packages and removing the invalid badge.
- Focused Biome lint on changed app code/config: passed. Svelte autofixer:
  no issues/suggestions on new component/state and changed components/page.
- `cargo test -p storage-server --no-default-features --features sync sync::`:
  4 tests passed. The new authorization test failed on the old owner mismatch
  before the fix, then passed. Initial cold compilations needed multiple bounded
  attempts; the completed test run is separate from those timeouts.
- `cargo clippy -p storage-server --no-default-features --features sync --lib`:
  passed with existing dependency/dead-code warnings.
- `rustfmt --check` on changed Rust files and `git diff --check`: passed.
- `@ofsdk/sync-client` test: 57 passed; `fixtures:check`: passed. Wire framing
  and handshake types were not changed.

Not passed / incomplete:

- App-wide `pnpm lint`: existing diagnostics, including the favicon SVG without
  an accessible title. Focused changed-file lint passed; unrelated files were
  not changed to silence lint.
- All-target Storage Clippy: 7 existing `unused_io_amount` errors in test code:
  `device_client.rs:733,765,799`, `idp_client.rs:606,637,670`, and
  `router/resources.rs:585`. These files are unchanged.
- Final `cargo hack test --feature-powerset --all-targets`: **120-second timeout**
  during compilation, first `idp-unified` feature run. No complete powerset result.
- Final `just crap`: **120-second timeout** during coverage compilation after
  SDK build. No CRAP score or completed coverage result.
- No real browser/native owner transfer or two-Node full-content qualification;
  no restored-data, Push/Pull, or successful-backup claim.

Integration tip is checked/merged on `implement/sync-app` before handoff. No
integration worktree files are changed by this task.
