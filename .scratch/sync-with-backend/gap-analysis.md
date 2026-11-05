# Gap analysis: sync-with-backend feature

Goal: a Sync button in the app that (a) uses an already-configured sync
service or lets the user set one up, (b) backs up local data, and
(c) chooses which Devices receive it. The other side is the
device/user/app/client management service.

## What exists today

### Sync backend (exists, server side)

- WS sync endpoint: `of/crates/storage-server/src/sync.rs`, route
  `/storage/sync/ws` (cargo feature `sync`). Framed protocol
  (Hello / AttachRequest / AttachResponse, CRC32, 16 MiB frame cap,
  1 GiB session cap), IdP-token auth, Push/Pull/Bidirectional.
  Advertised via `/version` `features: ["sync"]`.
- Rust sync client: `of/crates/sync-client` (native + browser features)
  plus `of/crates/sync-test-harness`. Wraps the `ofdb-sql` sync engine
  over WebSocket.
- Mesh/peer sync: `of/crates/storage-server/src/database_protocol.rs`,
  `data_protocol.rs`, `replication_admission.rs`,
  `local_replication_authorizer.rs` (iroh streams, authorization checked
  per apply).
- Persistence engines: `ofdb` (`ofdb-sql`/`ofdb-kv` `sync` feature),
  `offs` (filesystem sync).

### Device policy (exists)

- Device service: enroll/approve/revoke/rename, restrictions, and
  Resource Selection per Device+resource
  (`of/crates/device-service/GLOSSARY.md`).
- Generated client: `of/packages/device-client` (selections, restrictions,
  enrollment). UI: `of/apps/app/src/lib/common/components/DevicePolicy.svelte`
  (per-device selection + restriction + "Sync: Yes/No" badge from
  `checkStorageSyncCapability` in
  `of/apps/app/src/lib/common/state/deviceClient.svelte.ts`).
- Devices page: `of/apps/app/src/routes/(auth)/devices/+page.svelte`
  (list, rename, revoke, QR pairing via `SourceApproval.svelte`).

### Management UI (exists)

- Routes: `/users`, `/applications`, `/clients`, `/roles`, `/devices`.
- `of/apps/app/src/lib/common/state/idpAdministration.ts` (applications,
  consents, client keys via `@ofsdk/idp-client`); `packages/management-client`
  for roles/permissions.
- Config: `ConfigForm.svelte` has storage/device/idp/management API URLs.

## Gaps

### Sync button side

1. No sync action in any frontend. No WebSocket/sync code in
   `apps/app`, `apps/password-manager`, or `packages/*`. The Rust
   `sync-client` is not built to WASM (no wasm target in workspace) and
   has no Tauri commands (`apps/app/src-tauri/src` has none).
2. No "sync service" configuration. Config only has the four API URLs;
   the sync endpoint is derived implicitly from the storage URL
   (`/storage/sync/ws`) and only probed for the capability badge. There is
   no explicit sync-target field and no "set one up" flow (setup covers
   Installation, not sync targets).
3. No backup action. Password manager has local JSON import only
   (explicitly "not an encrypted backup"). No "back up now", no
   direction choice, no progress/last-result/last-error state.
4. Target devices exist only as policy (Resource Selection), not tied to
   a sync run: no per-run device picker, no per-device last-sync status.
5. No TS sync package (`packages/` has no `@ofsdk/sync-client`);
   `packages/storage-db-collection` is a plain KV adapter with no sync
   hook.

### Management side

6. Management service is roles/permissions only by design; User/
   Application/Client lifecycle lives in IdP. UI pages exist for all
   four, but none show sync state: no sync status on device records, no
   Application → resources → selected-Devices view, no per-device
   "sync now".

## Transport split (decided)

- Browser → device/server: WebSocket sync endpoint
  `/storage/sync/ws` (`of/crates/storage-server/src/sync.rs`).
- Device → device: iroh mesh Byte Streams via `ofnet` `iroh-chain`
  (`ofnet/crates/iroh-chain`: mesh/pairing/store; server side wired in
  `of/crates/storage-server/src/data_protocol.rs` +
  `database_protocol.rs` + `replication_admission.rs`). Mesh messages do
  not imply transparent multihop sync; sync runs over direct Byte
  Streams between approved Device endpoints.

## Risks / open points

- WS attach needs an IdP token (sync-client doc: `storage:sync`); the
  app already has OIDC token state (`state/oidc.svelte.ts`) to feed a
  token provider.
- Browser WS path exists in Rust (`sync-client` `browser` feature +
  `ofdb-sql/indexeddb`), but nothing publishes it as a TS/WASM package;
  a plain TS WebSocket client against `/storage/sync/ws` is the
  smallest first step.
- Browser iroh path: `ofnet/crates/iroh-chain-wasm` exists (stream,
  web_server) but no `of` package references it yet — needed only if the
  browser must sync device→device directly; otherwise the device
  server relays via its own iroh endpoint.

## Suggested build order

1. `packages/sync-client` (TS): connect to `{storageApiUrl}/sync/ws`,
   framing + Hello/Attach, Push/Pull/Bidirectional, token from existing
   OIDC state.
2. App state `sync.svelte.ts`: configured target (derived from config or
   explicit), per-resource last-run status.
3. Sync button (dashboard or `/devices`): browser → server over WS
   `/storage/sync/ws`; use configured storage service if the capability
   probe passes, else offer setup (enter storage URL / enroll a Device).
4. Backup picker: list resources (`@ofsdk/storage-client`) × devices
   (`listDevices`) honoring existing selections; run WS sync per target.
5. Device → device: native iroh Byte Stream sync between approved
   Devices (server side exists; add client driver in `of`/Tauri).
6. Management side: sync status column on `/devices`; link
   Application → resources → selections.
