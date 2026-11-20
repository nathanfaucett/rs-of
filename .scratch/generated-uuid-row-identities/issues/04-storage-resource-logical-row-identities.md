# 04: Decouple Storage Resource identities from Logical Row identities

**What to build:** Storage namespaces, Resources, and database metadata use generated UUIDv7 Logical Row identities while preserving stable public Resource identity and creation-key behavior. Deterministic UUIDv5 values no longer serve as single-UUID primary keys. Catalog creation, reservation, lookup, Tombstone handling, restart, and Resource selection work through the existing Storage contracts.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: open

- [ ] Namespace creation preserves stable namespace identity without inserting a deterministic UUIDv5 single-UUID primary key.
- [ ] Resource creation consumes generated UUIDv7 Logical Row identities and keeps Resource identifiers consistent across catalog and database metadata records.
- [ ] Creation-key reservation remains idempotent and returns the same public Resource identity without using that identity as the internal single-UUID primary key.
- [ ] Natural or composite primary keys and unique constraints enforce namespace and Resource domain identity where required.
- [ ] Catalog listing, owner isolation, deletion, Tombstone handling, restart, and selected Resource provisioning retain their public behavior.
- [ ] Updated Storage owners synchronize fresh UUIDv7 data without legacy UUIDv5 compatibility support.
- [ ] Focused Storage service tests and required formatting and lint checks pass.

## Comments

Implementation integrated. All 31 Storage service tests passed, including persistence across restart, history reconciliation, and deletion surviving a duplicate Resource. The Storage server suite and focused WebSocket synchronization test passed on earlier implementation tips. Final owner/feature/coverage qualification remains open.
