# 03: Delete roles and permissions directly

Status: ready-for-agent
Work: resolved
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** Management provisions each role or permission once, replicates that Logical Row, and deletes it with SQL. A replacement with the same name gets a new ID and no old grants.

- [x] Update initial administrator provisioning and other policy creation paths so peers do not independently create rows for the same policy ID.
- [x] Remove `rbac_revocations`, policy incarnations, and incarnation-derived grant IDs. Grants reference the policy ID; grants for an absent policy give no authority.
- [x] Through Management and real ofdb reconciliation, prove that a disconnected peer cannot restore a deleted policy or its authority after reconnecting. Prove that a replacement needs new grants.
- [x] Verify initial administrator setup still works. Update the initial schema and affected history filters, docs, and tests in place. No migration, legacy path, or version bump.

Evidence: Management reconciliation and administrator setup reopen tests passed. The focused native history test also passed: repeated exchange did not restore a deleted policy, and removing a permission denied a later exchange.

2026-11-24: Focused Management library Clippy and five all-feature tests passed, including cross-application isolation coverage. No migration, compatibility code, or version bump was added.
