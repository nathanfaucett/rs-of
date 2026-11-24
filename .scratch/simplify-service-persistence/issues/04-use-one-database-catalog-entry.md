# 04: Use one scoped database catalog entry

Status: ready-for-agent
Work: resolved
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** Each database has one catalog entry in its `(User subject, Application)` namespace. A creation retry returns the same database ID. Once a peer learns of deletion, a stale or late registration cannot restore that ID.

- [x] Put scope, ID, name, and lifecycle on the database entry. Remove the `namespaces`, `resources`, and `database_resource_tombstones` tables and unused SQL filesystem catalog metadata. Keep the existing filesystem catalog and typed Resource identity where needed.
- [x] Use ofdb reconciliation for the canonical lifecycle. Prove deletion wins against independent registration of the same ID and when a peer learns of deletion before registration. No extra service-side deletion table.
- [x] Verify scoped creation retries, User and Application isolation, and deletion across restart and authorized native synchronization. Keep deletion distinct from deselection and local copy cleanup.
- [x] Update the initial schema and affected contracts, callers, and docs in place. No migration, compatibility code, or version bump.

Evidence: All 33 Storage service tests passed after review, including independent registration, deletion before registration, restart, and refusal to initialize over uncataloged data. Focused native Storage checks passed before the issuer merge; the final native rerun timed out during build.

2026-11-24: All 33 current all-feature Storage tests and focused library Clippy passed. Device now has an explicit canonical schema boundary shared with unified migration. No migration, compatibility code, or version bump was added.
