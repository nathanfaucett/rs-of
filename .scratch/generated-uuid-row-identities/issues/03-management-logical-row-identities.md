# 03: Use generated Logical Row identities in Management persistence

**What to build:** Management creates roles, permissions, assignments, and Selection-related records through generated UUIDv7 Logical Row identities. Stable and idempotent domain operations use natural or composite identity where necessary instead of caller-supplied single-UUID primary keys. Public authorization, provisioning, Resource selection, conflict handling, and restart behavior remain intact.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

Work: open

- [ ] Role, permission, role-permission, and user-role inserts obtain generated UUIDv7 primary keys through the SQL Client.
- [ ] Idempotent create-with-ID and provisioning operations preserve public domain semantics without supplying a single-UUID Logical Row identity.
- [ ] Selection persistence uses generated internal identities and suitable natural or composite keys for conflict targets and stable lookup.
- [ ] ON CONFLICT behavior updates only permitted non-key fields and never preserves a caller-selected Logical Row identity.
- [ ] Authorization, initial administrator provisioning, Selection, revocation, restart, and synchronization tests pass through public Management interfaces.
- [ ] Fresh initialization requires no migration, new storage version, or new protocol version.
- [ ] Focused Management service tests and required formatting and lint checks pass.

## Comments

Implementation integrated. All seven Management service tests passed, including independent provisioning, grant convergence, and recreation without stale grants. The server suite reproduced a baseline SQL translator failure. Authenticated owner transport and full feature and coverage qualification remain open.
