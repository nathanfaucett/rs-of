# Node security boundaries

[System design](design.md) states the equal offline-first requirements. This source audit records current boundaries without claiming full host/platform acceptance. Detailed contracts live in [IdP local security](../crates/idp-service/docs/local-security.md), [Storage security](../crates/storage-service/docs/security.md) and [public persistence](public-persistence.md).

## Identity is not interchangeable

A token subject identifies a User or Client Principal. A Node Signer identifies its issuer. An authenticated Iroh endpoint identifies the connection peer. None may substitute for another, and an endpoint ID or message-source claim is not a User actor or resource permission.

IdP administration uses validated bearer identity with exact audience, action and target. Management permission evaluation binds a User caller to the same requested User; only the separately registered least-privilege evaluator Client may assert a User under its explicit evaluator scope/audience. Alternate actor/subject headers grant no authority. Existing `x-actor-subject` impersonation-denial tests are intentional negative evidence, not obsolete authentication features. Selection supplies a kind-specific Storage token in the typed `storageAccessToken` body field, not an `X-Storage-Authorization` override.

`/replica-signers` is the current authorized public Node-signer API. Its historical name, `DbReplicaSignerRepo`, persisted membership table names, `replica` persistence features and CRDT source/receiver roles do not create a designated authority/privileged replica topology. Public signer approval is not private signing capability or blanket synchronization permission.

Storage transport bases are not token audiences. The password-manager caller delivery exchanges the current User token for `resource=storage:filesystems`, with exact typed Storage actions: read for listing, write for creation, read/write for the editable filesystem socket. Explicit IdP/Storage service bases replace the audience/origin fallback; no actor override or wider permission is introduced. Explicit application SQL access additionally exchanges for `resource=storage:databases`
and uses Authorization-header HTTP batches, with parsed action authorization. The
application alone chooses its creation key and schema; obtaining SDK access creates
nothing. See the [Storage SQL identity/lifecycle contract](../crates/storage-service/docs/api.md#explicit-application-sql-contract)
for durable retry, Tombstones, unavailable-copy refusal and disconnected-Node limits.
Historical caller/package checks do not qualify GUI approval or current deployment acceptance.

## Local trust and learned denial

Each Node checks persistent locally trusted IdP/Management owner state. Optional independent IdP permission evaluation uses the established distinct evaluator Client over native-trust HTTPS, verifies the exact echoed request/audit identity and fails closed on remote failure. It never falls back to reading a remotely deployed Management database. Unified/local evaluation remains authoritative when present, including local denial, with no remote dependency or fallback. Unified service relationships use the local contexts; unavailable or invalid local evidence denies. Loss of a remote peer alone is not authorization loss, and there is no 30-second trust freshness gate. Normal token expiry/renewal margins, purpose-specific pairing controls and bounded test waits are not trust leases.

Connection admission and approved endpoint allowlisting are not resource authorization. Pairing is purpose-bound approval exchange, not general access. Security-history exchange checks exact local context-sync permissions, authenticated membership and public-history integrity. Resource protocols independently check ownership, namespace, selection and restriction at actual apply/read/write boundaries, including existing connections. Learned denial blocks subsequent work; already-authorized work may finish. Partitioned Nodes may temporarily diverge, and OAuth consumption is atomic locally rather than globally single-use.

Independent current status is not an authentication-mode switch. A completed owner's observation can return nonsecret `503 unavailable`, `ready:false` when protected material, current identity/policy or a required configured owner service is unavailable. Normal routers retain their own exact authentication/authorization. Observation recovery never reseeds state, restores admission or substitutes old public facts for current trust. Existing-only reopen needs no Installation source peer; Storage's current policy observation still needs its configured Management/IdP services. The native-root HTTPS startup probe uses OPTIONS on the existing status path (200/405), without running readiness-bearing GET. Slow/unavailable configured owners can time out GET or return unavailable while the listener and normal route authentication remain active; neither the probe nor public facts confer readiness/permission.

Independent transfer uses service-only `pendingCleanup` evidence under `management.replication.read`. Storage retires old-owner SQL/KV and filesystem copies before `POST /replication/owner-cleanup` under the configured Storage audience, Client principal and `management.replication.admit`. The exact durable cleanup ID/Device/endpoint/old/new-owner binding prevents stale acknowledgement from clearing a later fence, including repeated owner tuples. Recheck current binding before acknowledgement and policy after it; pending, restricted, failed or cancelled cleanup grants no access. Acknowledgement leaves policy empty/restricted and never restores allowance, selection or retired content. Remote-peer reads cannot erase this runtime's local copies. See [API contract](api.md#owner-contracts).

## Credentials and persistence

IdP/Management independent worker schemas are now version 2; Storage remains version 1. Ordinary older/contradictory state refuses unchanged, without migration. Explicit stopped reset supports only exact validated historical inventories/attempted slots for deletion, never serving compatibility or reference guessing. The local cleanup fact's `cleanup_id` is also required; deleting an old/malformed fence is not recovery. Ordinary prepare refuses retained `setup-reset` or `setup-reset.json` before any fresh admission/TLS, including interrupted final cleanup windows. Only exact explicit reset continuation may remove that evidence.

Keep independent private signers, raw passwords, raw secrets and confidential-client authentication material local in supported secure storage. Only reviewed public/domain projections and validated scoped history may synchronize. Removed mixed-row credential fields are rejected by current schema checks; externally dropping columns does not sanitize historical CRDT disclosure. Do not delete active local-only credential repositories, secret references, credentials metadata, or rejection fixtures merely because they mention a removed field.

`of` depends on public `ofdb-sql` and `ofdb-kv` facades. Owner-internal transitive dependencies, facade backend feature names and `.redb` data-file extensions are legitimate, not direct internal persistence dependencies. `offs` owns filesystem persistence, `ofnet` owns reusable endpoint/mesh/pairing/routing, and `of` owns domain authorization and protocol adapters. Preserve separate context roots and ordinary least-privilege local service-client scopes/audiences; no compatibility wrappers or central-role fallback are required.

## Owner-local setup authority

IdP, Device, and Storage each issue and retire one setup capability in every
layout. Management issues none. One request carries only its issuing owner's code;
only IdP receives the administrator password. Protected owner-only nonsymlink
admission state and process-lifetime locks fence cooperating workers. Ambiguous IO,
missing attempted material, malformed state and conflicting operations deny without
replacement. Status, URLs, logs, stdout and browser persistence contain no codes.

Each owner durably binds its exact operation before mutation/202. Public nonce,
operation equality, HTTPS and Origin are not admission authority. Recipient-scoped
single-use challenges authorize direct consumer→IdP registration; secrets remain
provider-local and never pass through the app. Configured exact UI origins and
noncredentialed CORS do not replace setup capability or ordinary OAuth checks.

Unified hosts the same owner workers, with separate owner state and one shared
local TLS authority. It has no parallel host lifecycle, journal, metadata database,
credential-bootstrap file, or deployment broker. Previous host layouts are refused
intact before fresh secret production; no migration/dual-read/automatic reprovisioning
exists. The IdP base is the canonical Create issuer. Join authenticates purpose/
nonce-bound discovery from the exact Device HTTPS base plus pinned endpoint and
retains the authenticated issuer. No private source signer/password is imported.

## Trust footprint

Desktop installs only its retained local dynamic setup server's certificate
authority. Hosted owners sit behind that endpoint and need no additional authority.
Explicit consent explains the OS anchor/elevation impact. Missing/expired/invalid
identity and arbitrary TLS errors do not authorize replacing or installing trust;
the native UnknownIssuer path verifies the same retained identity after consent.
Remote selection never retrieves bundled codes or substitutes local services.

CLI Create/Join uses `--ca-path` to pin the local authority in its HTTPS client.
It does not install OS trust. Standalone owners retain their own local authorities;
callers must verify each actual endpoint. No certificate-warning bypass, plaintext
fallback or relaxed endpoint verification is permitted. Native-root listener OPTIONS
is not readiness; owner GET/public correlation and normal authentication remain
separate. OS trust alone grants no membership, signer trust or resource permission.

## Reset and recovery boundaries

Each owner is resettable through protected stopped-owner management, not HTTP setup.
The full desktop reset closes/join workers and invokes the three owner boundaries
separately; it does not add a deployment setup/reset broker. Reset retains exact
inventories and completed-owner evidence across failure. Provider deletion precedes
data deletion, with no cross-owner enumeration or shared-row removal. Hosted resets
preserve the one shared CA and installed OS trust. Standalone explicit reset requires
`--remove-os-trust` and removes only its retained owner CA after protected cleanup.
Missing/contradictory evidence refuses unchanged; reference recovery validates
retained owner state rather than synthesizing keys, histories or identities.

Reset/cancellation is not remote revocation, rollback, peer erasure, or a permission
grant. Device and IdP signer revocation require their ordinary authorized owner
operations. Completed reopen needs no original Join source, but current local trust
and configured owner dependencies still apply; learned denial is enforced.

## Qualification boundary

[Ticket03](../.scratch/unified-owner-lifecycle/ticket03-validation.md) records the
nine passing process scenarios before the unrelated sibling manifest edit blocked
Cargo. [Ticket04](../.scratch/unified-owner-lifecycle/ticket04-validation.md) records
current consumer/client checks and exact validation limits. Historical receipts
are scoped evidence, not current platform certification. Mock providers/injected
trust and mounted forms do not certify actual OS installation/provider persistence,
GUI approval, WAN, containers, other platforms, crash recovery, or filesystem sync
between Nodes. No privileged trust operation was run for this change.
