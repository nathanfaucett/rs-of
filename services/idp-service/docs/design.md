# IdP design

The IdP provides conventional OAuth 2.0 and OpenID Connect. Definitions are in the [IdP glossary](../GLOSSARY.md); security invariants are in [local security](local-security.md). Exact HTTP routes and schemas belong to [the IdP OpenAPI contract](../../../openapi/idp.json).

The accepted target is the `rs-of` canonical monorepo, but migration is incomplete. See the [current-versus-target vision](../../../docs/goal.md) and [target architecture](../../../docs/architecture.md).

For implementation navigation, see [runtime IdP route registration](../../idp-server/src/router/openapi_router.rs), [unified owner composition](../../unified-server/src/runtime.rs), and the [owner E2E flow](../../../e2e/owners.e2e.ts).

## Ownership and local trust

IdP owns Users, credentials, Applications, OAuth Clients, grants, and Node Signer trust. The Device context owns Device records, endpoint identity, approval/revocation, and configuration. Management owns roles and permissions and authorizes administration; it does not own Device records.

Each Node must evaluate identity, policy, signer approval, and grant state against persistent trusted local state. Bundled composition may call local owner services directly. Independent deployment uses authenticated service APIs. Neither mode may read another owner's repositories or require a live central authority for every operation.

Missing, invalid, conflicted, or locally revoked trust denies. A network partition alone does not revoke valid local trust.

## OAuth Clients and consent

Use standard discovery, authorization, token, UserInfo, JWKS, PKCE, and refresh-token behavior. Key-based authentication does not replace OAuth/OIDC.

Public Client onboarding is permission-gated administration, not proof of authority supplied by an Application URI. Registration requires the Installation-scoped Client-registration permission. Creating a previously unknown Application requires its separate creation permission before any write. Onboarding does not grant later administrative CRUD, infrastructure provisioning, or User consent.

Consent display exposes only allowlisted public Client and Application metadata. Secrets, verifiers, private keys, and administrative grant settings stay excluded. User approval remains bound to the persisted Client, redirect URI, and scopes.

## Identity and Node Signers

A token subject is a stable Principal identity, not a raw authentication key or transport endpoint. Root rotation changes key ancestry without requiring a new User subject.

Each Node has an independent Node Signer. Private signing capability remains local; only approved public signer data may synchronize. Preparing a signer does not approve a Device, and Device approval does not prepare or approve a signer. JWKS publishes approved signing keys, not User authentication secrets.

Access JWT verification uses trusted issuer public keys and signed claims, not records for the User, Client, or authentication root. A verifier with a trusted JWKS snapshot can check a JWT without contacting the issuer. The OAuth token endpoint still checks current Principal, Client, and grant state before it issues or refreshes tokens. Resource and Device authorization remain separate. An actor header or authenticated transport endpoint does not prove API identity.

## Grants during partitions

Consume authorization codes and rotate refresh tokens atomically and durably on the executing Node. Local concurrent requests have one winner, including after reopen.

Two partitioned Nodes can consume separate local copies of the same grant. This design does not promise global single use or undo tokens already issued during a partition. Learned consumption or revocation denies later local operations. Raw codes, raw tokens, password verifiers, and private keys are never synchronization payloads.

## Current migration boundary

Current code and generated contracts still contain legacy single-User Device owner/transfer terminology and behavior. The target removes that model in favor of Installation-level Devices that may hold authorized replicas for multiple Users and Applications. Do not treat existing legacy flows as evidence that the target model is implemented.
