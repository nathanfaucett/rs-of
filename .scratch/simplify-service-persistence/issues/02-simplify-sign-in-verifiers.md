# 02: Simplify password and Client secret storage

Status: ready-for-agent
Work: resolved
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** Password sign-in uses one current verifier per User. Only OAuth Clients that authenticate with a shared secret have a Client verifier.

- [ ] Replace `credentials` and `local_credential_secrets` with one current User password verifier. Reset replaces it; the old password fails and the new one works. Do not retain old verifier rows.
- [ ] Key Client verifiers by Client ID. Do not create rows for public Clients. Reject an old secret after rotation; if replicated registration can change separately, reject a stale local verifier.
- [ ] Keep verifiers out of peer exchange. Remove `idp_conflict_resolutions` and its unused model. Use the existing ofdb conflict interface if resolution is needed.
- [ ] Verify sign-in, reset, and Client authentication through the existing IdP interface, and confirm that peer exchange excludes verifier data. Update the initial schema in place; no migration, compatibility code, or version bump.

Evidence: IdP service and local owner sign-in tests passed. The focused two-Node test passed after integration: public history exchanged, neither local verifier table crossed peers, and local password verifiers survived restart.
