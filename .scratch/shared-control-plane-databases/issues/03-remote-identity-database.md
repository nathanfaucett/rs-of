# 03-remote-identity-database.md

Status: open

## Problem Statement

While the Unified Host uses in-process handles, we must ensure the architecture works correctly when services are deployed as separate processes. This requires verifying that the remote `ofdb-sql` g-RPC client correctly implements the same service surface as the local `Database` handle.

## Solution

Provide a path for IdP and Management to operate against a remote `ofdb-sql` g-RPC server. This involves configuring the services to use a remote client instead of opening a local file.

## User Stories

1. As an IdP operator running without the Unified Host, I want to point IdP at an ofdb-sql g-RPC identity database, so that IdP does not open a local identity file.
2. As a Management operator running without the Unified Host, I want Management to use that same remote identity database, so that roles and permissions stay consistent with IdP.
3. As a developer, I want the remote ofdb-sql client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.

## Implementation Decisions

- Use `ofdb-sql` g-RPC server to front the identity database.
- Configure services to switch between `Arc<Database>` (local) and `ofdb-sql` client (remote) based on the presence of a remote connection string.

## Testing Decisions

- **Multi-Process Test:** Start an `ofdb-sql` g-RPC server. Start IdP and Management as two independent processes, both configured to use the remote g-RPC server. Verify that a change made in one is visible in the other.

## Out of Scope

- Using a different database engine.
