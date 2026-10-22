# 01-shared-identity-database.md

Status: open

## Problem Statement

The `idp-server` and `management-server` each open their own local database files (`idp.redb` and `management.redb`). This prevents running them as separate processes against a single, shared identity database. It also prevents a Unified Host from efficiently sharing the identity context between the two services without an expensive gRPC hop.

## Solution

Modify both `idp-server` and `management-server` to accept an injected database handle (`Arc<Database>`) as part of their router state. On a Unified Host, the host will open a single identity database and pass the same handle to both services. When running as separate processes, the services will instead use an `ofdb-sql` g-RPC client to connect to a remote identity database.

## User Stories

1. As a Unified Host operator, I want one identity database handle created at startup and passed to IdP and Management, so that both contexts see the same Users, roles, and permissions without a gRPC hop.
2. As an IdP operator running without the Unified Host, I want to point IdP at an ofdb-sql gRPC identity database, so that IdP does not open a local identity file.
3. As a Management operator running without the Unified Host, I want Management to use that same remote identity database, so that roles and permissions stay consistent with IdP.
4. As a developer, I want IdP and Management repositories to accept the shared database handle, so that Unified composition and remote deployment use the same service code.
5. As a developer, I want the remote ofdb-sql client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.

## Implementation Decisions

- The identity database holds IdP records (Users, clients, sessions, keys) plus Management records (roles, permissions, assignments).
- In-process injection: The Unified Host opens the identity database once and clones the `Arc<Database>` into the router state of both IdP and Management.
- Out-of-process adapter: Each service selects between an in-process `Arc<Database>` or a remote `ofdb-sql` client based on configuration.
- Repository interfaces: The service repositories must remain compatible with both the native `Database` handle and the remote `ofdb-sql` client.

## Testing Decisions

- **Unified Host Test:** Verify that a role written via the Management service is immediately visible to the IdP service on the same host using the shared handle.
- **Remote Test:** Start an `ofdb-sql` gRPC server, then start IdP and Management as separate processes pointing at that g-RPC server. Verify they agree on the same role and permission rows.

## Out of Scope

- Moving any device-related data into the identity database.
- Using a different database engine.
