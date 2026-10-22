Status: ready-for-agent

## Problem Statement

A Node's IdP and Management each open their own local database, and the Unified Host opens both and shares the Management handle with Storage admission. Device identity, approval, and replication policy live in the Management database even though they are a separate write domain from Users, credentials, roles, and permissions. Operators cannot run IdP and Management as separate processes against one identity database, and Storage cannot admit replication without holding Management's database in memory.

## Solution

Give the Installation two control-plane databases, each opened once and injected as router state.

The identity database holds Users, credentials, Applications, OAuth Clients, grants, token-signing identity, roles, permissions, and assignments. On a Unified Host, one database handle is created and passed to both the IdP router and the Management router. No in-process gRPC hop. When IdP and Management run as separate processes, that same database is served with ofdb-sql gRPC and both servers use the remote client.

Device identity, endpoint binding, approval, transfer, revocation, restrictions, selections, and replication policy move to a Device server with its own database. The Unified Host passes that second handle into the Device router and into Storage admission. A standalone Device server exposes the same database over ofdb-sql gRPC. Storage never opens a control-plane database. Management keeps only role and permission HTTP and stops owning Device state.

User Storage Resources stay on the existing iroh synchronization path. ofdb-sql gRPC is only the control-plane query API.

## User Stories

1. As a Unified Host operator, I want one identity database handle created at startup and passed to IdP and Management, so that both contexts see the same Users, roles, and permissions without a gRPC hop.
2. As a Unified Host operator, I want a second database handle for Devices, passed to the Device server and to Storage admission, so that Device state is not stored in the identity database.
3. As an IdP operator running without the Unified Host, I want to point IdP at an ofdb-sql gRPC identity database, so that IdP does not open a local identity file.
4. As a Management operator running without the Unified Host, I want Management to use that same remote identity database, so that roles and permissions stay consistent with IdP.
5. As a Device server operator, I want Device state served from its own ofdb-sql gRPC database, so that Device lifecycle can run without the identity database process.
6. As a Storage operator, I want replication admission to ask the Device server, so that Storage does not open Management's or the Device database.
7. As an Installation maintainer, I want Users, credentials, Applications, OAuth Clients, and token-signing keys to remain IdP-owned records in the identity database, so that authentication stays in the IdP context.
8. As an Installation maintainer, I want roles, permissions, and assignments stored in the identity database, so that Management can authorize without a private database.
9. As an Installation maintainer, I want Management to expose role and permission HTTP only, so that Management is not a second owner of Device or User records.
10. As a Device owner, I want Device create, name, approval, transfer, and revocation on the Device server, so that a Device stays a Management-shaped domain identity with one authority.
11. As a Device owner, I want restrictions and whole-resource selections on the Device server, so that what a Device retains is not mixed into identity records.
12. As a Trusted Device, I want Storage to admit replication only while the Device server reports that Device as approved and in policy, so that endpoint possession alone still does not grant synchronization.
13. As an operator resetting one runtime, I want Reset Device to clear that runtime's local setup and its client connection state, so that other Devices and the shared databases are left intact.
14. As an operator provisioning a Node, I want Installation Setup to open the identity database once and the Device database once, so that IdP owner setup and Device enrollment do not create duplicate files.
15. As a developer, I want IdP and Management repositories to accept the shared database handle, so that Unified composition and remote deployment use the same service code.
16. As a developer, I want the remote ofdb-sql client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.
17. As a tester, I want a Unified Host test that writes a role through Management and reads it through IdP authorization on the same handle, so that sharing is proven without gRPC.
18. As a tester, I want a separate-process test where IdP and Management both use one gRPC identity database, so that the out-of-process path is proven.
19. As a tester, I want Storage admission to fail closed when the Device server denies the Device, so that the in-process database shortcut cannot regress.
20. As a Unified Host operator, I want the host to keep composing IdP, Management, Device, and Storage on one endpoint, so that a Node remains one Installation member.
21. As a security reviewer, I want the identity database and the Device database to use separate schemas and migration owners, so that a Device change cannot migrate credential tables.
22. As an operator, I want gRPC used only when the database process is not the server process, so that the Unified Host does not pay a network hop for router state it already holds.

## Implementation Decisions

- Two databases, two ownership domains. The identity database holds IdP records plus roles, permissions, and assignments. The Device database holds Devices, endpoint bindings, approval and revocation, restrictions, selections, and replication policy. Device rows do not move into the identity database.
- In-process injection is an `Arc` to the ofdb-sql database, stored as router state (IdP already keeps the engine on router state; Management builds its services from a handle created outside the router). The Unified Host opens each database once and clones the handle into every router that needs it. Storage admission receives the Device handle or a Device-server client, not the identity handle.
- gRPC is the out-of-process adapter only. A standalone ofdb-sql server fronts each database. IdP, Management, and the Device server select the in-process handle or the remote client at startup. Request paths do not open database files.
- Management's HTTP surface after the split is roles, permissions, and assignments. Device, pairing, restriction, selection, and replication routes move to the Device server. Management does not open a database file and does not own Device lifecycle.
- The Device server is the authority Storage calls for endpoint trust and replication policy. Storage's user SQL and KV replication stays on the existing iroh sync protocol.
- The Unified Host remains a composer: listener, prefixes, TLS, and protocol adapters. It stops owning two control-plane files as private service databases. It still creates the two handles during local bootstrap and passes them down.
- Schema migration runs in the database owner for that domain: identity migrations with the identity database, Device migrations with the Device database. HTTP servers do not embed a second copy of those migrations against a private file.
- Existing IdP and Management repository interfaces stay the boundary. They are constructed with the injected handle. A remote client is acceptable only when it implements that same query behavior.
- Reset of one runtime drops local server state and disconnects clients. It does not delete the shared identity database or the Device database when those are hosted for other servers.

## Testing Decisions

Test external behavior at the Unified Host and at separately started servers. Do not assert repository types, file names, or whether a call stack crossed gRPC.

Good tests show:

- One in-process identity handle: a Management role or permission write is visible to IdP authorization on the same host, with no gRPC listener.
- One remote identity database: IdP and Management started as separate processes against one ofdb-sql gRPC server agree on the same role and permission rows.
- Device state is readable from the Device server and from Storage admission, and is absent from the identity database's schema.
- Storage admission allows a Trusted Device reported by the Device server and refuses a revoked or unknown endpoint, without Storage opening either control-plane database.
- Reset of a single server process does not remove rows still served to the other server.

Prior art is the Unified Host composition tests and each server's independent create, join, and reset tests. Extend those seams. Do not add tests that only construct a database handle and call a repository method.

## Out of Scope

- Changing user Storage Resource replication, filesystem residency, or the iroh sync protocol.
- Merging the identity database and the Device database into one file or one gRPC server.
- Replacing ofdb-sql with another engine.
- Running gRPC inside the Unified Host for a database the host already holds.
- Moving OAuth, token signing, or User credentials onto the Device server.
- A general query API for operators beyond what IdP, Management, Device, and Storage already need.

## Further Notes

The glossary keeps Device authority distinct from IdP. This spec moves that authority from Management's embedded database to a Device server, and leaves Management as the role and permission API over the shared identity database. "Management does not need its own data" means Management has no private database file. It still reads and writes roles and permissions through the injected identity handle.
