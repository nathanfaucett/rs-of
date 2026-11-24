# 04: Decouple Storage Resource identities from Logical Row identities

**What to build:** Storage namespaces, Resources, and database metadata use generated UUIDv7 Logical Row identities while preserving stable public Resource identity and creation-key behavior. Deterministic UUIDv5 values no longer serve as single-UUID primary keys. Catalog creation, reservation, lookup, Tombstone handling, restart, and Resource selection work through the existing Storage contracts.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved

- [x] Namespace creation preserves stable namespace identity without inserting a deterministic UUIDv5 single-UUID primary key.
- [x] Resource creation consumes generated UUIDv7 Logical Row identities and keeps Resource identifiers consistent across catalog and database metadata records.
- [x] Creation-key reservation remains idempotent and returns the same public Resource identity without using that identity as the internal single-UUID primary key.
- [x] Natural or composite primary keys and unique constraints enforce namespace and Resource domain identity where required.
- [x] Catalog listing, owner isolation, deletion, Tombstone handling, restart, and selected Resource provisioning retain their public behavior.
- [x] Updated Storage owners synchronize fresh UUIDv7 data without legacy UUIDv5 compatibility support.
- [x] Focused Storage service tests and required formatting and lint checks pass.

## Comments

Implementation integrated. All 31 Storage service tests passed, including persistence across restart, history reconciliation, and deletion surviving a duplicate Resource. The Storage server suite and focused WebSocket synchronization test passed on earlier implementation tips. Final owner/feature/coverage qualification remains open.

2026-11-24: Focused Storage library Clippy passed with the two pre-existing workspace-policy allowances. All 33 current all-feature library tests passed, including creation-key identity and deletion reconciliation. Formatting passed.
