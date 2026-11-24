# 01: Validate access JWTs with one issuing authority

Status: ready-for-agent
Work: resolved
**Blocked by:** None (can start immediately).
Spec: [Simplify IdP, Management, and Storage persistence](../spec.md)

**What to build:** A Node verifies access JWTs with trusted issuer keys and signed claims, without asking the issuer for current User or OAuth Client state. One authority issues tokens per issuer. Keep JWKS, key rotation, and separate Device approval.

- [x] With a retained trusted issuer key, accept a valid access JWT without live identity lookups. Reject a wrong signature, algorithm, issuer, audience, token purpose, or validity interval.
- [x] Check current User, Client, and grant state at issuance and refresh. Blocking a User or disabling a Client stops issuance and refresh but does not invalidate an unexpired access JWT by itself. Keep current Resource and Device authorization checks.
- [x] Remove `idp_replica_members`, `idp_replica_signers`, and the enrollment and exchange paths used only for independent Node issuance. Do not share private signing keys. Preserve Device enrollment and approval.
- [x] Verify issuance, JWKS, validation, and rotation through existing owner interfaces. Update affected contracts, generated clients, and docs. Fresh stores only; no migration, legacy path, or version bump.

Evidence: The issuer cutover and server Join removal are integrated. Retained-key JWT verification, independent first-Installation Create and reopen, issuer rotation, owner contract checks, and real-owner signed Device approval passed. The broad Rust suite was not rerun after focused fixes. Disconnected issuance and automatic second-server setup are out of scope.

## Comments

The maintainer chose to remove obsolete second-server setup instead of adding issuer-side Client provisioning. A new Device uses endpoint-key proof and authorized Device approval. No migration, legacy path, or version bump.

2026-11-24: Focused IdP library Clippy passed; 71 all-feature and 16 no-default tests passed. Device passed 73 tests. OpenAPI regeneration and contract checks passed. Broader workspace and CRAP limits are recorded in the generated-identity qualification issue.
