# IdP security invariants

These invariants guide implementation and review. They are not a claim of full-system acceptance. See [IdP design](design.md) for scope and [the IdP OpenAPI contract](../../../openapi/idp.json) for wire details.

## Administration and permissions

Bind each authenticated administrative request to the verified actor, service audience, action, and target. Application-scoped requests must also match the actor's Client Application. Evaluate permissions before mutation; missing policy and evaluation errors deny.

Bundled composition may use local Management and IdP service interfaces without HTTP self-calls. Independent deployment uses authenticated HTTPS relationships. IdP never opens Management or Device repositories. Management authorizes administration but does not own Device records.

The IdP active-User check denies blocked or missing Users and failed status lookups. The local interface and authenticated HTTP route use the same check. This check does not change access token verification or User profile lookups.

Client registration and Application creation require their exact Installation permissions. Application-scoped administration, authentication, and consent do not imply Installation authority. Existing Installations need explicit policy grants; startup must not invent an all-User grant or silently backfill permissions.

## Signers and tokens

Only the issuing authority installs or rotates issuer keys. The private key must match the active public key. Missing private material fails closed. Device approval and endpoint approval are separate. Rotation keeps the previous public key for the longest configured token lifetime.

JWKS contains public issuer keys only. Keep private signers, entity master keys, derived private keys, password verifiers, Client secret verifiers, and raw grant credentials out of public responses and synchronization.

Access, ID, and refresh tokens have different uses. Validate access JWTs against trusted issuer keys. Require the expected algorithm, issuer, audience, purpose, and validity interval. Take the subject, Application, and scopes from signed claims. The API owner still checks current Resource, Device, and Management authorization as applicable.

Issuance and refresh check current User, Client, and grant state. Blocking a User, disabling a Client, or changing its Application does not change an unexpired access JWT. Learning that an issuer key was withdrawn can deny a token before expiry. A verifier with an older JWKS does not know about the withdrawal. ID and refresh tokens are not access tokens.

## Grants and reconnect

Store grant hashes rather than raw authorization codes or refresh tokens. Consume and rotate them in durable local transactions. Recheck Client, redirect/PKCE, scope, Principal, signer, and signature evidence before issuance or consumption.

Unresolved security-record conflicts deny authentication, authorization, or issuance; last-writer-wins ordering must not erase consumption or revocation evidence.

## Local sign-in verifiers

Each User has at most one local password verifier. Reset replaces that verifier in the current row. It does not erase the local database history. User identity sync does not include password verifier data.

Only confidential OAuth Clients that use `client_secret_basic` or `client_secret_post` have a local secret verifier. Public registration shares a non-secret revision for shared-secret Clients. The local verifier must match that revision. A peer that learns of a rotation cannot authenticate with its old verifier.

## Public synchronization projections

Synchronize only explicit public projections. Do not export opaque whole rows whose private fields or history could escape validation. Reject unknown fields, private JWK parameters, malformed identities, and raw credentials presented as hashes.

Allowlisted data may include public Users, Applications, Client metadata, public authentication keys, consent state, and hashed grant lifecycle state. Public Client data does not provide confidential-client authentication capability. Each Node provisions its own required secrets.

Transport authorization uses the authenticated connection peer and current Device policy, not a wire-supplied actor, endpoint, signer, or pairing claim. Repeat permission checks at the actual import or apply boundary.

## Retained state and recovery

Persist the bindings required for offline restart. Missing or contradictory retained evidence fails closed. Do not delete journals, append random permission identities, replace signers, or infer missing setup modes during startup.

The SQL Database generates UUIDv7 Logical Row identities. A predetermined User subject remains a public domain identifier, separate from the generated row identity. Replicas retain the generated row identity. This schema requires a fresh store; it does not upgrade existing IdP stores.

Unsupported retained state requires an explicit supported recovery or a deliberate fresh root. A restart is not a migration, and a permission-schema change is not proof that older Installations were upgraded safely.

## Qualification boundary

Repository tests can prove local transaction, replay, reopen, and permission behavior without proving OS secure stores, rendered applications, WAN partitions, or multi-Device target behavior. GitHub Actions records browser and runtime results for each tested commit. Do not present a past run as current evidence.
