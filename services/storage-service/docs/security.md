# Storage security invariants

These are required protections for current review and target implementation. They are not a blanket statement that every deployment or application flow is qualified. See [Storage design](design.md) for ownership and migration status.

## Identity and Resource access

Derive the `(User subject, Application)` namespace from validated IdP evidence. Check token signature, issuer, audience, use, lifetime, Principal state, and exact `read`/`write` action. Never accept a caller-supplied owner, actor header, or Application override as identity evidence.

Endpoint keys identify transport peers, not Users or Resources. Device approval, endpoint allowlisting, pairing, catalog discovery, and Resource Selection are separate from Resource access. Management authorizes administration but does not own Device records.

Do not trust handshake claims about Device owners, endpoints, selections, or Resource namespaces. Resolve the authenticated peer through current trusted Device state and validate the Resource through Storage's catalog and namespace rules.

## Execution and reconnect

Authorize at the actual read, send, receive, import, and apply boundaries, not only during connection admission. Recheck after waits that can learn new policy. Failed local evaluation denies without mutation.

Both peers evaluate their own trusted state. Existing connections receive no exemption. An admitted operation may finish; newly learned denial blocks later work. No freshness timer or live central authorization call substitutes for durable local trust and execution checks.

Preserve owner protocol integrity, atomic apply, earlier valid progress, actionable rejection, and cancellation cleanup. Network waits must not hold database write transactions. SQL text, parameters, results, tokens, private keys, and owner error details must not enter logs or redacted client failures.

## Revocation and local copies

Selection starts as desired replication state and never changes the Resource namespace. Deselection and managed local cleanup are not logical Resource deletion. Tombstones prevent deleted state from returning when stale peers reconnect.

Device revocation prevents later authorized work when learned. It does not transfer Resource ownership, guarantee immediate global blocking, or remotely erase existing copies. Legacy single-User owner/transfer cleanup remains in current code but is target for removal, not evidence of completed migration.

## ofdb and transport boundaries

Consume only public `ofdb-sql` and `ofdb-kv` facades in the target: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Do not build service authorization on internal engine transactions or redb APIs. The current dependency migration is incomplete.

Filesystem metadata completion is not proof of Full-content backup. Passthrough reads require a currently authorized Full peer. WebSocket attachment is not proof of a completed owner exchange, durable replica identity, or backup.

## Evidence boundary

Focused tests do not qualify WAN behavior, real OS credential stores, rendered applications, every crash point, or multi-User shared Devices. GitHub Actions records results for each tested commit. Do not present a past run as current evidence.
