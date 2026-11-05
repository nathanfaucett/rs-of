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

**Status:** ready-for-agent

- [ ] Sync button runs immediately against the configured storage service when the capability probe reports sync support
- [ ] When no capable sync service is configured, the button opens a setup flow (storage URL entry or Device pairing) and then syncs
- [ ] Backup picker lists resources and trusted Devices, pre-checking existing Resource Selections, and honors per-run target changes
- [ ] Sync runs to each selected target with progress and success/error result visible to the User
- [ ] Unreachable or rejecting sync service produces a clear error and leaves local data untouched
- [ ] Revoked Devices are not offered as targets and stop receiving data
- [ ] Last-sync status per Device persists across restarts and is shown on the Devices page
- [ ] UI behavior covered by app-level interaction tests (mounted component, mocked fetch/WebSocket) asserting what the User sees and which requests are made
