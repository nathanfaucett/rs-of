# Sync with backend map

## Notes

- [Spec](spec.md), [gap analysis](gap-analysis.md).
- Ticket 01 base: `implement/sync-with-backend` at
  `a03e37f331509641f2670de9b6cd5e30a75403dc`.
- Change assessment: isolated TS package; moderate parser/auth risk, no data
  migration or runtime dependency. Tests cover the public protocol boundary.

## Decisions-so-far

- [01: TS WebSocket sync client](issues/01-ts-ws-sync-client.md): resolved.
  `@ofsdk/sync-client` uses actual Rust server framing/handshake fixtures.
  Authenticated attach exposes an open send/receive/close owner transport and
  requested/granted directions. Result is **attached**, never **backed up**.
  See the package README for verified SQL owner entry points and blockers.

- [02: Sync button](issues/02-sync-button.md): **open; partial implementation**.
  Safe Devices-page setup/capability checks, current-Application resource
  discovery, approved Device preferences, and User/target-scoped persisted
  **blocked** results. No attach or transfer is presented as success. WS checks
  exact current resource policy before owner reads/sends and SQL apply.
  See the ticket Answer for investigation and qualification limits.

- [02: Sync button](issues/02-sync-button.md): **open; partial implementation**.
  Safe Devices-page setup/capability checks, current-Application resource
  discovery, approved Device preferences, and User/target-scoped persisted
  **blocked** results. No attach or transfer is presented as success. WS checks
  exact current resource policy before owner reads/sends and SQL apply.
  Native `StorageSyncDriver` exists and passes engine test (SQL + KV at reopen).
  Composed Create/Join/native sync qualification blocked by IdP login Client
  persistence in reopened database.

## Fog


- [02: Sync button](issues/02-sync-button.md) needs a real owner binding or
  native host driver before it can report backup success. WS currently runs
  SQL only, not KV/filesystems. Push/Pull needs complementary scoped engines.
- Confirm token exchange to `storage:sync`, real Device endpoint selection,
  persistent owner identity, and durable completion. WS pre-apply checks now
  reauthorize, but the real endpoint binding and transfer are not qualified.
- Add an authorized User-readable selection directory and generated client.
  Local preferences are not policy; the Applications resource/Device view,
  full picker, and authenticated native manual-run driver remain open.
- Fix IdP login Client persistence across Create/Join reopen for composed
  native sync qualification.
- Live-server transfer, browser owner binding, KV, filesystem content, and
  mesh sync are not qualified by ticket 01 protocol tests.
