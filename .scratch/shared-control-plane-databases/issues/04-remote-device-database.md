# 04-remote-device-database.md

Status: open

## Problem Statement

Similar to the identity database, the Device database must also be accessible via g-RPC to support out-of-process operation of the Device server and to allow Storage to query it remotely.

## Solution

Expose the Device database via `ofdb-sql` g-RPC, allowing the `device-server` to run as a standalone process and allowing `storage-server` to query it remotely.

## User Stories

1. As a Device server operator, I want Device state served from its own ofdb-sql g-RPC database, so that Device lifecycle can run without the identity database process.
2. As a Storage operator, I want replication admission to ask the Device server, so that Storage does not open the Management or Identity databases.
3. As a Developer, I want the remote `ofdb-sql` client to satisfy the same query surface the in-process handle does, so that server code does not branch on where the database lives except at process startup.

## Implementation Decisions

- Use `ofdb-sql` g-RPC server to front the Device database.
- `storage-server` will be updated to use a remote `device-server` client for admission checks.

## Testing Decisions

- **Remote Admission Test:** Start a standalone `device-server` via g-RPC. Start a `storage-server` and verify that it can successfully admit a device reported by the remote `device-server`.

## Out of Scope

- Changing how user-data replication (the iroh sync path) works.
