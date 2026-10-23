# 01-shared-identity-database.md

Status: resolved

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

## Comments

Implemented on `implement/shared-control-plane-01`. Remote stories 2, 3, and 5 are tracked by [ticket 03](03-remote-identity-database.md); Device extraction is ticket 02.

## Answer

The Unified Host now injects its single identity database handle into IdP authorization and Management role/permission repositories. Device repositories and Storage admission retain the separate Device handle. Initial administrator provisioning writes policy into the identity database. Identity-owned policy migrations are shared by standalone Management initialization; Device-only migrations do not create identity policy tables.

Security synchronization and join bootstrap split Management history into identity-policy and Device sessions, preserving grants across offline reopen. The security handshake and bootstrap purpose advance to v2; mixed v1/v2 peers are not supported. Standalone Management still initializes its local combined database pending tickets 02 and 03. No remote adapter or separate-process acceptance is claimed here.

Focused validation passed on 2026-10-23:

```sh
cargo test -p unified-server --lib shared_identity_management_writes_change_idp_authorization -- --nocapture
cargo test -p idp-model identity_owner_migrates_roles_without_device_schema

RUST_MIN_STACK=16777216 cargo test -p unified-server --lib owner_cutover_node_bootstrap_reopens_offline
```

The HTTP acceptance covers initial denial, Management role creation/permission assignment/User assignment, immediate IdP allowance, retained-router access after owner shutdown, persisted allowance after reopen, and immediate denial after revocation. The identity migration test verifies idempotence and absence of Device schema. The join test verifies bootstrap and offline reopen. The existing Device migration test covers the reverse isolation boundary, but its feature-enabled rerun (`cargo test -p management-service --features replica --lib device_owner_does_not_migrate_identity_policy`) timed out during compilation at 120 seconds; no pass is claimed. Initial invocations without `replica` selected zero tests.

The worktree used the prior agent's temporary `python3 .scratch/validate.py` prefix to rebase sibling dependency paths for Cargo; that helper and its four temporary check logs were removed after validation. The logs identify Cargo test executables and test filters, but do not preserve complete shell commands or environment. The join test initially overflowed the default test-thread stack; the explicit 16 MiB stack rerun passed. Changed Rust files were formatted and `git diff --check` passed.

Qualification remains focused: earlier agent logs reported failures in broader provisioning/security suites (create 8/10, outbound 3/4, security 14/24); those suites were not rerun or certified by this completion pass. Full feature-powerset, clippy, and CRAP validation were not run. Remote configuration/query behavior and separate-process testing remain deferred to ticket 03.
