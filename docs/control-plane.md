# Shared control-plane databases


Unified composes `/idp`, `/management`, `/device`, and `/storage`. One injected identity database serves IdP and Management roles, permissions, and assignments. A separate injected Device database serves Device lifecycle, endpoint trust, restrictions, whole-resource selections, and replication policy. Device tables and migrations never enter identity; identity policy tables never enter Device history.

Device and pairing, Node-join, restriction, selection, and replication HTTP use `/device`, not `/management`. Management exposes role/permission/assignment HTTP only (plus public metadata and permission evaluation). There are no old-route redirects. Device administration continues to require the existing `idp-management` actor audience and exact installation/application permissions; existing `management.device.*` and `management.replication.*` service scopes remain authorization identifiers, not route ownership.

Storage receives a Device authority callback in Unified, or an authenticated `DeviceClient` for a separate service. It opens neither control-plane database. Unknown, inactive-owner, unapproved, revoked, conflicting, missing-policy, and pending-cleanup evidence fails closed; owner transfers retain the copy-retirement fence and binding rechecks. Reset does not remove the shared identity database.

`device.data_dir` is the second Unified database root (`devices.redb`). Storage uses `device_api_base_uri`, `device_oauth_client_id`, `device_oauth_client_secret`, `device_service_audience`, and `device_endpoint_id`. Browser Device calls use `PUBLIC_DEVICE_BASE_URL` or the explicit Device API URL; desktop composition mounts `/device`. Existing installations require fresh setup; there is no legacy file/schema migration. Remote Device SQL gRPC startup is ticket04, not part of the in-process extraction.
