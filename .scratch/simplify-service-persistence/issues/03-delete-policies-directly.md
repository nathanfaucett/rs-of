# 03: Delete roles and permissions directly

Status: ready-for-agent
Work: open
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** Management provisions each role or permission once, replicates that Logical Row, and deletes it with SQL. A replacement with the same name gets a new ID and no old grants.

- [ ] Update initial administrator provisioning and other policy creation paths so peers do not independently create rows for the same policy ID.
- [ ] Remove `rbac_revocations`, policy incarnations, and incarnation-derived grant IDs. Grants reference the policy ID; grants for an absent policy give no authority.
- [ ] Through Management and real ofdb reconciliation, prove that a disconnected peer cannot restore a deleted policy or its authority after reconnecting. Prove that a replacement needs new grants.
- [ ] Verify initial administrator setup still works. Update the initial schema and affected history filters, docs, and tests in place. No migration, legacy path, or version bump.

Evidence: Current Management tests cover deletion when separate rows share a policy identity. The replacement test must cover the new single-row contract. No tests were run for this review.
