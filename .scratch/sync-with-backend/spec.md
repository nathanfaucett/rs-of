Status: ready-for-agent

# Spec: Sync with backend (sync button, backup targets, management view)

## Problem Statement

As a User of an Installation, I have no way in the app to back up my data
or synchronize it with other Devices. There is no Sync button, no way to
configure a sync service if none is set, no way to choose which Devices
receive my data during a backup, and the management pages (Devices, Users,
Applications, Clients) show no sync state. The backend sync machinery
exists (WebSocket sync endpoint on the storage service, iroh Byte Stream
sync between Nodes), but nothing in the app drives it.

## Solution

Add a Sync button to the app. If a sync service is already configured
(the storage API of the local Node), the button uses it directly; if not,
it offers a setup flow (enter a storage service URL, or enroll/pair a
Device). Pressing Sync backs up local data (push) to the configured
service and lets the User pick which trusted Devices also receive it,
honoring existing Resource Selections. Browser-to-device/server sync runs
over the WebSocket sync endpoint; device-to-device sync runs over iroh
Byte Streams between approved Devices. The management pages gain sync
status so the User can see which Devices are selected and when they last
synced.

## User Stories

1. As a User, I want a Sync button on my dashboard, so that I can back up my data with one click.
2. As a User, I want the Sync button to use my already configured sync service automatically, so that I do not have to re-enter anything.
3. As a User with no configured sync service, I want the Sync button to offer a setup flow, so that I can enter a storage service URL or pair a new Device.
4. As a User, I want to choose which Devices receive my backup, so that my data only goes where I want.
5. As a User, I want my Device selections from policy (Resource Selection) to be pre-applied in the backup picker, so that I do not have to reconfigure targets every time.
6. As a User, I want to back up (push) my local database to the sync service, so that a copy exists on the server/Node.
7. As a User, I want to pull data from the sync service, so that a new Device can restore a backup.
8. As a User, I want bidirectional sync, so that changes made on any Device merge everywhere.
9. As a User, I want to see progress and result (success/error) while syncing, so that I know the backup worked.
10. As a User, I want to see when each Device last synced, so that I can tell which Devices are up to date.
11. As a User, I want sync to require my signed-in session (IdP token), so that only I can trigger backups of my data.
12. As a User, I want sync to fail safely when the sync service is unreachable, so that I get a clear error and my local data is untouched.
13. As a User, I want a Device that was revoked to stop receiving my data, so that revoking a Device actually protects my backups.
14. As a User, I want to see which resources (databases/filesystems) will be backed up, so that I know what is included.
15. As an administrator, I want to see sync status per Device on the Devices page, so that I can manage the Installation without opening each Device.
16. As an administrator, I want to see which Applications and their resources are selected for which Devices, so that I understand the replication policy at a glance.
17. As a User, I want the app to detect whether the configured storage service supports sync (capability probe), so that I am told if my backend cannot sync.
18. As a User on a new Device, I want to enroll it via Device Pairing and then pull my backup, so that I can restore my data on that Device.
19. As a User, I want device-to-device sync to happen over the mesh (iroh) between my approved Devices, so that sync works even when one Device is offline from the server.
20. As a User, I want my sync configuration (which service, which Devices) to persist across restarts, so that I do not set it up again.

## Implementation Decisions

- Transport split (decided): browser/app to device/server uses the
  WebSocket sync endpoint exposed by the storage service; device-to-device
  sync uses iroh mesh Byte Streams between approved Device Endpoints via
  the shared iroh-chain endpoint. Mesh Broadcast/Direct Messages are not
  used for sync; mesh forwarding does not imply multihop sync.
- New TypeScript sync client package (browser): connects to the WS sync
  endpoint under the configured storage API URL, implements the framing
  and Hello/Attach handshake, and runs Push/Pull/Bidirectional sessions.
  Authentication uses an IdP access token supplied by a token provider
  fed from the existing signed-in OIDC state.
- App sync state module: holds the configured sync target (derived from
  the configured storage API URL, or set explicitly in setup), probes the
  service's sync capability via its version metadata, and records
  per-resource/per-Device last-run status, progress, and errors.
- Sync button placement: dashboard and/or Devices page. When a configured
  service supports sync, the button runs immediately; when not, it opens
  a setup flow (enter storage service URL, or enroll a Device via the
  existing pairing flow).
- Backup target picker: lists the User's resources (databases/filesystems
  via the storage client) and trusted Devices (via the device client),
  pre-checking Devices that already have a Resource Selection for the
  resource; running sync applies the selection and triggers sync to each
  chosen target.
- Device-to-device: a native sync driver (in the app host / `of`) opens
  iroh Byte Streams to approved Device Endpoints and runs the existing
  database/filesystem sync protocols; server-side admission and
  per-apply authorization already exist and are reused unchanged.
- Management view: Devices page gains a sync status column (last sync,
  state); Applications view links Applications to their resources and the
  Devices selected for them. Management remains roles/permissions only;
  User/Application/Client lifecycle stays in the IdP.
- Configuration: the sync target is derived from the existing storage API
  URL configuration; an explicit override is stored alongside the other
  API URLs in app config.
- Persistence: sync configuration and last-run status persist in the
  app's local state so they survive restarts.

## Testing Decisions

- Primary seam: app-level UI interaction tests (existing pattern: mount
  the Svelte component in vitest with mocked fetch/invoke) covering the
  Sync button states (configured/unconfigured/error), the setup flow,
  the target picker, and status display. Tests assert external behavior
  only: what the User sees and which requests are made.
- Protocol seam: the WS sync handshake and framing are tested against
  the existing in-memory sync test harness / transport pair used by the
  Rust sync client tests, including attach accept/reject and direction
  handling.
- Server seam: storage service router tests (existing axum-style tests)
  cover the WS sync route and the version/capability metadata the app
  probes.
- Device-to-device seam: existing storage-server protocol tests (database
  stream handling, replication admission, per-apply authorization) cover
  the iroh path; the new native driver is tested at the highest seam via
  end-to-end two-instance sync using the test harness.
- Good test = external behavior only: button flow, request shapes, status
  shown, selection effects; no assertions on internal state machines.

## Out of Scope

- Transparent multihop sync through the mesh; any new networking stack or
  relay behavior.
- Changes to context ownership: IdP still owns Users/Applications/Clients,
  Management still owns roles/permissions, Device still owns Devices and
  selections, Storage still owns resources.
- New authorization model: existing IdP tokens, Device approval, and
  per-apply execution authorization are reused as-is.
- Global/immediate revocation guarantees; cleanup semantics stay as
  defined (revocation takes effect when learned).
- Encrypted-at-rest backup format changes; backups are sync copies of
  existing resources, not a new archive format.
- Browser-side iroh (device-to-device from the browser) unless a Device
  must sync directly to another Device without its own server; the
  device server relays via its own iroh endpoint.

## Further Notes

- The Rust sync client (native + browser features) and the sync test
  harness already implement the WS protocol; the TypeScript client can
  either wrap/reuse the protocol definitions or re-implement the thin
  framing + handshake in TS.
- A gap analysis of existing pieces vs. this feature lives in the same
  feature scratch directory.
- Sync capability detection already exists as a version-metadata probe
  used for the per-Device badge; reuse it for the button's
  configured/unconfigured decision.
