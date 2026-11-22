# 01: Validate access JWTs with one issuing authority

Status: ready-for-agent
Work: open
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** A Node verifies access JWTs with trusted issuer keys and signed claims, without asking the issuer for current User or OAuth Client state. One authority issues tokens per issuer. Keep JWKS, key rotation, and separate Device approval.

- [ ] With a retained trusted issuer key, accept a valid access JWT without live identity lookups. Reject a wrong signature, algorithm, issuer, audience, token purpose, or validity interval.
- [ ] Check current User, Client, and grant state at issuance and refresh. Blocking a User or disabling a Client stops issuance and refresh but does not invalidate an unexpired access JWT by itself. Keep current Resource and Device authorization checks.
- [ ] Remove `idp_replica_members`, `idp_replica_signers`, and the enrollment and exchange paths used only for independent Node issuance. Do not share private signing keys. Preserve Device enrollment and approval.
- [ ] Verify issuance, JWKS, validation, and rotation through existing owner interfaces. Update affected contracts, generated clients, and docs. Fresh stores only; no migration, legacy path, or version bump.

Evidence: Source review found live principal and Client lookups and per-Node signer trust. Tests were inspected, not run. Disconnected issuance is out of scope.
