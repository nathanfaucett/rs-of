# Map: Shared Control-Plane Databases

## Notes

The goal is to split the control-plane into two distinct domains: Identity (IdP + Management) and Devices. This allows running them as separate processes via ofdb-sql gRPC while still supporting a unified in-process host via injected database handles.

## Decisions-so-far

- Split the control-plane into two databases: `Identity` (Users, Credentials, Roles, Permissions) and `Device` (Devices, Approvals, Policies).
- Use in-process injection of `Arc<Database>` (router state) for the Unified Host to avoid gRPC overhead.
- Use `ofdb-sql` gRPC as the adapter for out-of-process operation.
- Management becomes a stateless API for roles and permissions over the shared identity database.
- A new `Device` server becomes the authority for device lifecycle and replication policy.
- [Ticket 01](issues/01-shared-identity-database.md) resolved for in-process identity sharing: Unified IdP and Management use one identity handle, Device migrations stay isolated, and bootstrap transfers identity policy separately from Device history (security/bootstrap protocol v2).
- Focused acceptance passed: HTTP role assignment → IdP allow → reopen → revoke → deny, identity migration isolation, and joined Node offline reopen. The feature-enabled Device migration rerun timed out compiling; broader historical suite failures remain unqualified. See ticket 01 Answer.
- Remote identity configuration, query adaptation, and separate-process acceptance are deferred to [ticket 03](issues/03-remote-identity-database.md); ticket 01 does not claim remote support. Device-server extraction remains ticket 02.

- [Ticket 02](issues/02-device-server-with-its-own-database.md) resolved on `implement/shared-control-plane-02`: extracted `device-service`/`device-server` and Device SDKs, isolated Device schema/history, reduced Management HTTP to identity policy administration, and composed `/device` with a shared Device authority for Storage admission. Device HTTP approval/restriction → Storage trust → revocation denial and identity-schema isolation passed, as did offline learned-revocation admission and the shared identity regression.
- Ticket02 focused validation includes 56 Device-service tests, 33 Device/Management server tests, 15 Storage authority/client tests, Unified acceptance/composition, native Tauri compilation, 43 generated SDK/generator tests, and 106 frontend tests. The Device feature matrix stopped on an incomplete-catalog conflict-import fixture, CRAP compilation timed out, and full app type-checking lacks generated/workspace artifacts; see its Answer for exact commands and limits. Remote Device SQL startup remains ticket04.

## Fog

- Ticket04 must wire and qualify remote Device SQL gRPC startup without adding a control-plane database opener to Storage.
- Device feature-powerset integration currently stops on the sync/conflict fixture's incomplete catalog import; broader independent-host/native-provider and full workspace/CRAP qualification remain open validation work.
