# 02-device-server-with-its-own-database.md

Status: open

## Problem Statement

The `management-server` currently owns Device state, including identity, approval, and replication policy. This data is part of a separate write domain from the Identity database. Furthermore, `storage-server` currently depends on the Management database directly (via in-process handles) for replication admission, which prevents scaling the device lifecycle independently and makes the architecture harder to decouple.

## Solution

Extract all device-related logic and data into a new `device-server`. This server will manage its own database containing device identities, endpoint bindings, approvals, revocations, restrictions, and replication policies. The `storage-server` will then query the `device-server` (instead of opening the Management database) to perform replication admission.

## User Stories

1. As a Unified Host operator, I want a second database handle for Devices, passed to the Device server and to Storage admission, so that Device state is not stored in the identity database.
2. As a Device owner, I want Device create, name, approval, transfer, and revocation on the Device server, so that a Device stays a Management-shaped domain identity with one authority.
3. As a Trusted Device, I want Storage to admit replication only while the Device server reports that Device as approved and in policy, so that endpoint possession alone still does not grant synchronization.
4. As a Storage operator, I want replication admission to ask the Device server, so that Storage does not open the Management or Identity databases.

## Implementation Decisions

- The Device database holds: Devices, endpoint bindings, approval/revocation status, restrictions, and selection/replication policies.
- The `device-server` is a standalone service that can be run in-process (via injected `Arc<Database>`) or out-of-process (via `ofdb-sql` gRPC).
- `management-server` is stripped of device-related routes and logic; it only retains role and permission HTTP.
- `storage-server` is updated to call the `device-server` (via HTTP or gRPC) for admission checks instead of reading a local database.

## Testing Decisions

- **Unified Host Test:** Verify that a device can be approved on the Device server and that Storage recognizes it as a trusted device, using the second injected handle.
- **Separation Test:** Verify that device-specific records (like a new device) are not visible when querying the Identity database.

## Out of Scope

- Moving OAuth, token signing, or User credentials onto the Device server.
- Changing how user-data replication (the iroh sync path) works.
