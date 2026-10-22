# Map: Shared Control-Plane Databases

## Notes
The goal is to split the control-plane into two distinct domains: Identity (IdP + Management) and Devices. This allows running them as separate processes via ofdb-sql gRPC while still supporting a unified in-process host via injected database handles.

## Decisions-so-far
- Split the control-plane into two databases: `Identity` (Users, Credentials, Roles, Permissions) and `Device` (Devices, Approvals, Policies).
- Use in-s-process injection of `Arc<Database>` (router state) for the Unified Host to avoid gRPC overhead.
- Use `ofdb-sql` gRPC as the adapter for out-of-process operation.
- Management becomes a stateless API for roles and permissions over the shared identity database.
- A new `Device` server becomes the authority for device lifecycle and replication policy.

## Fog
- Determining the exact crate structure for the `device-server`.
- Identifying the exact set of routes to migrate from `management-server` to `device-server`.
- Defining the exact API surface for the `Device` server that `storage-server` requires.
