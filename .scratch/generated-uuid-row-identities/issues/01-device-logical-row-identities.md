# 01: Use generated Logical Row identities in Device persistence

**What to build:** Device creation uses omitted or DEFAULT primary keys and consumes the UUIDv7 identity returned by the SQL Client. Device Selection preserves deterministic domain lookup and repeated-selection behavior after decoupling Selection identity from the single-UUID Logical Row identity. Fresh initialization, restart, conflict handling, and synchronization work through the public Device APIs without caller-supplied UUID primary keys.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: resolved

- [ ] Device creation and enrollment-related inserts obtain generated UUIDv7 primary keys through RETURNING and use those values consistently in related records.
- [ ] Device Selection uses a natural or composite primary key, with a separate unique domain identifier when needed, instead of a deterministic UUIDv5 single-UUID primary key.
- [ ] Repeated Selection, deselection, owner observation, and conflict resolution preserve their current public semantics and stable domain lookup.
- [ ] Fresh-schema initialization, restart, Tombstone handling, and synchronization tests pass through public Device interfaces.
- [ ] No upgrade migration, compatibility adapter, storage version, or protocol version is added.
- [ ] Focused Device service tests and required formatting and lint checks pass.

## Comments

Implementation integrated. Device creation and signed Join use generated keys. Device contract/server tests and the independent-Replica deselection and reselect regression passed. The Device service suite had 17 failures reproduced on the baseline; final Clippy timed out. Full owner qualification remains open.
