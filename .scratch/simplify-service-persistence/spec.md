# Simplify IdP, Management, and Storage persistence

Status: ready-for-agent
Work: open
Milestone: [M3 — Two-Device offline use and administration](../../docs/goal.md#milestones)
Dependencies: Use the existing generated Logical Row identities and public Database and Client interfaces. No external dependencies identified.

## Problem Statement

Access-token verification reads live IdP account, principal-key, and OAuth Client state even after it verifies the token. Credential and signer table names obscure the difference between signing in, issuing tokens, and verifying tokens. Management keeps policy incarnations because provisioning can create separate rows for one public policy ID. Storage splits database metadata across three tables and keeps whole-database deletion in a fourth. A separate, mounted Management selection path duplicates Device selection and does not consistently scope queries to the authorized Application and owner.

## Solution

Validate access JWTs with trusted issuer keys and signed claims. Keep sign-in, token issuance, and Resource authorization separate. Create each role and permission once and use SQL deletion on that row. Give each database one scoped catalog entry whose deletion cannot be undone by stale replica state. Keep one Resource Selection path. Make the changes for fresh stores only.

## User Stories

1. As an API caller, I want a valid access token to work without a live IdP account lookup, so that a Node can verify it when the issuer is unavailable and its trusted key is already available.
2. As an API owner, I want tokens with the wrong issuer, audience, type, or validity interval rejected, so that a valid signature cannot grant unintended access.
3. As an administrator, I want blocking a User or disabling a Client to stop new token issuance and refresh, so that existing access expires within its documented lifetime.
4. As a User, I want password sign-in and reset to use the current verifier, so that the old password does not work after reset.
5. As an OAuth Client developer, I want public Clients to work without secret records and confidential Clients to authenticate by their configured method, so that only Clients that use shared secrets require verifiers.
6. As a Node operator, I want JWKS to publish public issuer keys only, so that a verifier never needs the private signing key.
7. As an administrator, I want deleting a role or permission to remove its authority after peers learn the deletion, so that an offline copy cannot restore the policy.
8. As an administrator, I want a replacement policy with the same name to start without the old grants, so that recreating a policy does not restore access.
9. As an application developer, I want authorized OAuth Clients of one Application to share a User's database namespace, so that different Clients can access the same data without crossing User or Application scope.
10. As a User, I want retrying database creation with the same creation key to resolve to the same database ID, so that a retry does not create a second database.
11. As a User, I want deletion of a database to prevail over stale catalog state, even on a peer that had not yet registered it, so that synchronization cannot restore the database.
12. As a User, I want deselection to stop replication for the selected Device without deleting the Resource, so that other authorized Devices keep access.
13. As an administrator, I want one scoped Selection path, so that permission for one Application cannot list, replace, or remove another Application's selection.
14. As a maintainer, I want only the current schema and contracts, so that old stores, routes, and wire formats require no maintenance.

## Implementation Decisions

- **Access JWTs:** Verify the signature with a key from the configured issuer's trusted JWKS. Enforce the expected algorithm, issuer, audience, expiry, applicable time claims, and access-token purpose. Use signed claims for identity and Application context; remove live User, principal-root-key, and OAuth Client lookups from access-token validation. Continue to enforce scopes and current Management, Device, and Storage authorization at the operation. Do not treat an ID or refresh token as an access token.
- **Issuance and refresh:** Check current User, Client, and grant state when issuing or refreshing tokens. Keep the existing refresh consumption and revocation state separate from access JWTs. Do not add a per-access-token record or denylist. A User block or Client disable does not invalidate an already issued access JWT by itself. Do not promise immediate invalidation on disconnected Nodes, including after signing-key withdrawal.
- **Password verifiers:** Preserve supported password sign-in. Replace `credentials` and `local_credential_secrets` with one current password verifier per User on the authenticating IdP. Reset replaces it; do not retain historical verifiers or pick one of several active rows. Keep verifiers out of replicated identity data. Remove `idp_conflict_resolutions` and the unused audit model; use the existing ofdb conflict interface where an owner needs it.
- **OAuth Client verifiers:** Keep one verifier keyed by OAuth Client ID only for confidential Clients that use a shared secret. Keep secrets out of replicated registration data and out of bearer validation. Public Clients have no verifier row. Remove misleading credential references; if a replicated registration can still change independently of a local verifier, preserve the stale-verifier rejection invariant without a shared User credential model.
- **Issuer keys:** Use one token-issuing authority per issuer for this scope. Nodes can verify with retained trusted public keys; they do not independently issue tokens while disconnected from the authority. Remove `idp_replica_members`, `idp_replica_signers`, and the Node Signer enrollment and trust-exchange paths that exist only for independent issuers under one issuer identity. Preserve ordinary issuer key rotation, JWKS, and separate Device enrollment, approval, and endpoint binding. Do not share private issuer keys to replace the removed path.
- **Management:** Give each role and permission one immutable policy ID. Provision once and replicate the same Logical Row; do not recreate a policy by supplying a retained public ID on another Node. Use SQL `DELETE` on that row. Remove `rbac_revocations`, policy incarnations, and incarnation-derived grant IDs. Grants reference the actual policy ID. A replacement gets a fresh ID and no inherited grants. Authorization must ignore grants whose policy no longer exists.
- **Storage catalog:** Keep the `(User subject, Application)` namespace as a domain scope, not a separate SQL record. Fold database identity, scope, display name, and lifecycle into one database catalog entry. Remove the `namespaces` and `resources` indirection and unused SQL filesystem catalog metadata; preserve the existing filesystem catalog. Keep `ResourceKind` where selection or routing needs it. Preserve the stable database ID returned for a repeated scoped creation key.
- **Whole-database deletion:** Remove the separate `database_resource_tombstones` table. Store deletion in the canonical catalog lifecycle and use ofdb reconciliation. Deletion for a database ID must win over stale, concurrent, or later registration of that same ID, including on a peer that learns the deletion before registration. A plain SQL row flag or KV key deletion is insufficient if independent rows or a new Generation can replace the deleted state. Demonstrate the invariant through the existing public seams before removing the table. Keep local copy cleanup and deselection distinct from whole-database deletion.
- **Selection:** Remove `sync_selections`, its Management routes, and the Storage caller that uses them. Keep the existing scoped Device-facing Selection path and the Storage authorization checks that consume its state. Management supplies administrative permissions, not a second Selection store. Preserve the target ownership distinction: Storage owns Resource access and Selection; Device owns Device approval and configuration.
- **Cutover:** Update the initial schema, affected call sites, current documentation, OpenAPI, and generated clients in place. Remove obsolete paths and fixtures. Initialize fresh stores and exchange only current state. Do not add an upgrade migration, version bump for compatibility, legacy route, or adapter.

## Testing Decisions

- Test through the existing owner HTTP interfaces and authenticated synchronization paths. Assert what callers can do and what peers observe, not private record layouts. Use the public ofdb Database and Client interfaces for focused reconciliation tests before running owner tests.
- **IdP and API owners:** Extend the real-owner OIDC and signer tests. Verify a valid access JWT with a retained trusted JWK and without a live identity lookup. Reject wrong issuer, audience, signature, token purpose, and expired tokens. Prove that User block or Client disable prevents new issuance and refresh without silently invalidating an unexpired access JWT. Exercise supported signing-key rotation, password reset, public Client registration, and confidential Client authentication. Verify that no verifier or signing private key enters peer history exchange.
- **Management:** Extend the real-ofdb policy reconciliation test. Create and replicate a policy, disconnect a peer, delete the policy, then reconnect. Prove the stale policy and grants do not restore authority. Recreate the name and prove the replacement has a new ID and grants apply only after explicit assignment. Exercise initial administrator provisioning through the existing setup path.
- **Storage and Device:** Extend the independent database registration, deletion-before-registration, restart, and native selected-sync tests. Verify same-scope creation retries, cross-scope isolation, and that an authorized peer cannot reopen or provision a deleted ID after learning the deletion. Verify deselection prevents later synchronization but does not delete the Resource. Test that Selection cannot cross User or Application scope and that the removed routes are no longer available.
- **Contracts and workflow:** Check the generated contracts and affected clients against the owner routes. Run the real-backend browser vault workflow and a focused real-backend two-Node selection and deletion scenario. Use the existing browser test for ciphertext and native tests for selected replication as prior art. Record test results and any limits; passing focused tests does not qualify broader offline convergence.

## Out of Scope

- Backward compatibility, store upgrades, migration tooling, legacy routes or code, and storage or wire versioning for this change.
- Multiple disconnected Nodes independently issuing tokens under one issuer, immediate global revocation, access-token denylisting, and remote erasure.
- A new audit trail, a redesign of ofdb identity, or additional filesystem features.
- Changes to the public refresh flow beyond removing its dependence on obsolete credential and signer models.

## Further Notes

This spec records the result of source review, not shipped behavior. The single-authority issuer is the proposed baseline for this scope. If disconnected issuance is a product requirement, resolve that before removing Node signing trust; it does not change the JWT verification seam. The catalog lifecycle representation is an implementation choice, subject to the reconciliation checks above. The review inspected existing tests but did not run them.
