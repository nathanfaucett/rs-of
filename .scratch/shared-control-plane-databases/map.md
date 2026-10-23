# Map: Shared Control-Plane Databases

## Notes

The goal is to split the control-plane into two distinct domains: Identity (IdP + Management) and Devices. This allows running them as separate processes via ofdb-sql gRPC while still supporting a unified in-process host via injected database handles.

## Decisions-so-far

- Split the control-plane into two databases: `Identity` (Users, Credentials, Roles, Permissions) and `Device` (Devices, Approvals, Policies).
- Keep `Arc<Database>` with local owners for migration/sync/lifecycle; inject its cloneable `db::Client` into identity repositories and query state. The Unified Host still shares one local identity owner without a gRPC hop.
- Use `ofdb-sql` gRPC as the adapter for out-of-process operation.
- Management becomes a stateless API for roles and permissions over the shared identity database.
- A new `Device` server becomes the authority for device lifecycle and replication policy.
- [Ticket 01](issues/01-shared-identity-database.md) resolved for in-process identity sharing: Unified IdP and Management use one identity handle, Device migrations stay isolated, and bootstrap transfers identity policy separately from Device history (security/bootstrap protocol v2).
- Focused acceptance passed: HTTP role assignment → IdP allow → reopen → revoke → deny, identity migration isolation, and joined Node offline reopen. The feature-enabled Device migration rerun timed out compiling; broader historical suite failures remain unqualified. See ticket 01 Answer.
- [Ticket 03](issues/03-remote-identity-database.md) resolved: existing identity repositories accept `db::Client` locally/remotely; validated standalone startup attaches to shared SQL, and the `identity-database` owner binary runs domain migrations. Required remote conflict/transaction parity is committed separately in `ofdb` on `integration/shared-control-plane-databases` at `59ac5a5`.
- Ticket03 external-process HTTP acceptance passed: Management assignment → IdP allow → consumer reset/restart with rows preserved → revocation → deny, and SQL-owner outage fails closed. In-process Unified regression, all 11 db feature combinations, 12 SQL remote/conflict tests, focused compilation and affected-library Clippy passed. See the ticket Answer for exact commands, merge-sensitive files and limits.
- Remote consumers attach to an initialized Installation and retain IdP-local signing material. Remote independent Create/Join wizard, remote Device implementation, default-stack/native-provider deployment, and full workspace/CRAP qualification are not claimed. Device-server extraction and remote Device operation remain tickets 02/04.

## Fog

- Determining the exact crate structure for the `device-server`.
- Identifying the exact set of routes to migrate from `management-server` to `device-server`.
- Defining the exact API surface for the `Device` server that `storage-server` requires.
