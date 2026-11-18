# Storage design

Storage provides offline-first database and filesystem Resources. Definitions are in the [Storage glossary](../GLOSSARY.md); required protections are in [security](security.md). Current source structure is not proof of target completion or runtime qualification.

## Target model

The accepted `rs-of` target assigns ownership as follows:

- Device owns Device records, endpoint identity, approval/revocation, and configuration.
- Management owns roles and permissions and authorizes administration. It does not own Device records.
- Storage owns Resource catalogs, Resource namespaces, and Resource access.
- IdP owns User identity, OAuth/OIDC grants, and Node Signer trust.

A Device is an Installation-level approved endpoint. With permission, it may hold replicas for multiple Users and Applications. Selection maps a Resource to a Device as desired replication state. It does not change Resource ownership or grant access by itself.

Current code and contracts still contain the legacy single-User Device owner/transfer model. That model is target for removal. Current legacy cleanup or transfer flows must not be documented as shared-Device support or completed migration. See [target architecture](../../../docs/architecture.md) and [ADR 0002](../../../docs/adr/0002-shared-installation-devices.md).

## Resource isolation

Every database and filesystem belongs to one `(User subject, Application)` namespace. OAuth Clients of one Application share the User's namespace. Callers cannot select an arbitrary owner or Application override for API access.

Validate the token subject, Application, audience, lifetime, and required action on every request. Storage API actions do not authorize mesh replication. There are no Device-key delegation grants, folder ACLs, Unix permission bits, or implicit cross-User shares.

Catalog discovery does not prove local content possession or permission. Database IDs and Filesystem IDs are kind-specific. Deletion creates a Tombstone for the logical Resource; deselection concerns local desired replication state.

## Selection and replication

Selection must remain within the Resource namespace and pass the required Device, Management, and Storage checks. Each peer evaluates its own trusted policy. Handshake claims about owners, endpoints, or selections are not evidence.

Both endpoint approval and Resource authorization are required. Endpoint allowlisting and pairing establish no general Resource access. Recheck authorization at send, receive, and apply boundaries, including on existing connections.

An already admitted operation may finish. Learned revocation, deselection, restriction, or deletion blocks later operations. An unaware offline peer can temporarily act on stale trusted state; immediate global revocation is not promised.

## Cleanup, Residency, and deletion

Device revocation does not transfer Resource ownership or remotely erase copies. Deselection and authorized local cleanup can remove managed local bytes, but they cannot erase copies held by unaware or unrelated Devices.

Full/Passthrough Residency is local filesystem policy, not Selection or synchronized ownership. Passthrough retains metadata, denies writes, and requires an authorized available Full peer for content reads. A mode acknowledgement does not prove that all content is available or erased.

## Persistence and deployment boundaries

Storage owns domain catalogs and access decisions. ofdb owns SQL/KV persistence and synchronization; offs owns filesystem persistence and synchronization. Transport and domain adapters must not become alternate persistence engines.

The target public ofdb boundary is only `ofdb-sql` and `ofdb-kv`: `Database` for lifecycle/synchronization and `Client` for queries/transactions. Services must not import internal engine or redb crates. Current dependencies are not fully migrated. See [ADR 0003](../../../docs/adr/0003-ofdb-facade-boundary.md).

One server process does not require one physical database file or one shared database host. Current deployment layouts and their limits are in [backend deployments](../../../docs/backend-deployments.md).
