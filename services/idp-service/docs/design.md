# IdP design

The IdP provides conventional OAuth 2.0 and OpenID Connect. Definitions are in the [IdP glossary](../GLOSSARY.md); security invariants are in [local security](local-security.md). Exact HTTP routes and schemas belong to [the IdP OpenAPI contract](../../../openapi/idp.json).

The accepted target is the `rs-of` canonical monorepo, but migration is incomplete. See the [current-versus-target vision](../../../docs/goal.md) and [target architecture](../../../docs/architecture.md).

For implementation navigation, see [runtime IdP route registration](../../idp-server/src/router/openapi_router.rs), [unified owner composition](../../unified-server/src/runtime.rs), and the [owner E2E flow](../../../e2e/owners.e2e.ts).

## Ownership and local trust

IdP owns Users, credentials, Applications, OAuth Clients, grants, and issuer keys. The Device context owns Device records, endpoint identity, approval/revocation, and configuration. Management owns roles and permissions and authorizes administration; it does not own Device records.

Each Node must evaluate identity and policy against trusted local state. The issuing authority also checks current grant state. Bundled composition may call local owner services directly. Independent deployment uses authenticated service APIs. Neither mode may read another owner's repositories or require a live central authority for every operation.

Missing, invalid, conflicted, or locally revoked trust denies. A network partition alone does not revoke valid local trust.

## OAuth Clients and consent

Use standard discovery, authorization, token, UserInfo, JWKS, PKCE, and refresh-token behavior. Key-based authentication does not replace OAuth/OIDC.

Public Client onboarding is permission-gated administration, not proof of authority supplied by an Application URI. Registration requires the Installation-scoped Client-registration permission. Creating a previously unknown Application requires its separate creation permission before any write. Onboarding does not grant later administrative CRUD, infrastructure provisioning, or User consent.

Consent display exposes only allowlisted public Client and Application metadata. Secrets, verifiers, private keys, and administrative grant settings stay excluded. User approval remains bound to the persisted Client, redirect URI, and scopes.

## Identity and issuer keys

A token subject is a stable Principal identity, not a raw authentication key or transport endpoint. Root rotation changes key ancestry without requiring a new User subject.

One IdP authority issues tokens for an issuer. It keeps private signing keys in its local key store and serves public keys through JWKS. The issuer key ring is not part of peer history exchange. A relying Node uses trusted public keys and has no signing capability. Device enrollment and approval do not grant token-signing trust.

Configure a verifier with `oauth2.trusted_issuer_jwks` and no `local_signer`. Use a trusted snapshot from the configured issuer. Install an updated snapshot to accept tokens signed by a new key. Device approval does not add an issuer or provision OAuth Clients.

Access JWT verification uses trusted issuer public keys and signed claims, not records for the User, Client, or authentication root. A verifier with a trusted JWKS snapshot can check a JWT without contacting the issuer. The OAuth token endpoint still checks current Principal, Client, and grant state before it issues or refreshes tokens. Resource and Device authorization remain separate. An actor header or authenticated transport endpoint does not prove API identity.

## Grants during partitions

Consume authorization codes and rotate refresh tokens atomically and durably on the executing Node. Local concurrent requests have one winner, including after reopen.

Only the issuer can consume grants or issue replacements. A relying Node can continue to verify an unexpired access token using a retained trusted key during a partition. Raw codes, raw tokens, password verifiers, and private keys are never synchronization payloads.

## Current migration boundary

Current code and generated contracts still contain legacy single-User Device owner/transfer terminology and behavior. The target removes that model in favor of Installation-level Devices that may hold authorized replicas for multiple Users and Applications. Do not treat existing legacy flows as evidence that the target model is implemented.
