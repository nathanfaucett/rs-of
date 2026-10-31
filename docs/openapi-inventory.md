# HTTP route and consumer inventory

Rust production registration and `openapi/*.json` are authoritative; this is a
navigation inventory. See [generation and qualification](api.md#generated-clients).

## Topologies

Exactly four canonical contracts/SDK products exist: IdP, Management, Device, Storage.
All use complete verified owner bases, including the configured prefix once. Unified
mounts `/idp`, `/management`, `/device`, `/storage`; standalone uses configured bases.
IdP, Device and Storage each expose `/setup/owner/...` under their own capability;
Management is not a setup owner. No aggregate setup/readiness contract or old path
alias exists. Source approval for this lifecycle uses Device's
`/independent/node-joins/...` operations in both layouts. Separate domain Node Join
operations do not establish a second Installation Setup lifecycle. Public OpenAPI
is served before setup; only its deployment server URL varies.

## Mounted application operations

### idp

- `GET /.well-known/jwks.json` — `jwks` (200)
- `GET /.well-known/openid-configuration` — `openid_configuration` (200)
- `GET /applications` — `list_applications` (200)
- `POST /applications` — `create_application` (200)
- `DELETE /applications/{application_id}` — `delete_application` (200)
- `GET /applications/{application_id}` — `get_application` (200)
- `PUT /applications/{application_id}` — `update_application` (200)
- `DELETE /clients/{client_id}/keys` — `revoke_client_keys` (200)
- `GET /clients/{client_id}/keys` — `list_client_keys` (200)
- `POST /clients/{client_id}/keys/rotate` — `rotate_client_key` (200)
- `GET /health` — `health` (200)
- `POST /independent/node-joins/enroll` — `independent_join_enroll_node_join` (204, 400, 401, 403, 409, 503)
- `POST /independent/node-joins/history/request` — `independent_join_request_history` (202, 400, 403, 503)
- `GET /independent/node-joins/history/stream` — `independent_join_history_stream` (101, 400, 401, 403)
- `GET /oauth2/auth` — `authorize_query` (303)
- `POST /oauth2/auth` — `authorize_json` (200)
- `POST /oauth2/auth/allowed-for-user` — `is_allowed_for_user` (200)
- `POST /oauth2/auth/approve-for-user` — `approve_for_user` (200)
- `GET /oauth2/client/{client_id}` — `get_client` (200)
- `POST /oauth2/device/auth` — `device_auth` (200)
- `GET /oauth2/device/verify` — `device_verify` (302)
- `POST /oauth2/introspect` — `introspect` (200, 401, 403)
- `POST /oauth2/register` — `register` (200)
- `DELETE /oauth2/register/{client_id}` — `delete_register` (200)
- `GET /oauth2/register/{client_id}` — `get_register` (200)
- `PUT /oauth2/register/{client_id}` — `put_register` (200)
- `POST /oauth2/revoke` — `revoke` (200)
- `GET /oauth2/sessions/logout` — `sessions_logout` (302)
- `POST /oauth2/token` — `token` (200, 400, 401)
- `GET /openapi.json` — `openapi_json` (200)
- `POST /replica-signers` — `enroll_replica_signer` (201, 403)
- `DELETE /replica-signers/{member_id}` — `revoke_replica_signer` (200)
- `PUT /replica-signers/{member_id}` — `rotate_replica_signer` (200)
- `POST /setup/owner/admission` — `owner_setup_admission` (204, 401, 503)
- `POST /setup/owner/cancel` — `owner_setup_cancel` (204, 401, 409, 503)
- `POST /setup/owner/challenge` — `owner_setup_challenge` (204, 400, 403, 409, 503)
- `POST /setup/owner/create` — `owner_setup_create` (202, 400, 401, 409, 503)
- `POST /setup/owner/history` — `owner_setup_history` (202, 403, 409)
- `POST /setup/owner/intent` — `owner_setup_recover` (200, 204, 401, 503)
- `POST /setup/owner/join` — `owner_setup_join` (202, 400, 401, 409, 502, 503)
- `GET /setup/owner/public` — `owner_setup_owner_public` (200, 409)
- `POST /setup/owner/register` — `owner_setup_register` (204, 400, 403, 409, 503)
- `GET /setup/owner/status` — `owner_setup_status` (200, 503)
- `GET /userinfo` — `userinfo` (200)
- `GET /users` — `list_users` (200)
- `DELETE /users/{user_id}` — `delete_user` (200)
- `GET /users/{user_id}` — `get_user` (200)
- `PUT /users/{user_id}` — `update_user` (200)
- `GET /users/{user_id}/active` — `active_user` (204, 401, 403, 404)
- `GET /users/{user_id}/consents` — `list_consents` (200)
- `DELETE /users/{user_id}/consents/{consent_id}` — `revoke_consent` (200)
- `PUT /users/{user_id}/password` — `reset_password` (200)
- `GET /version` — `version` (200)

### management

- `GET /applications/{application_id}/permissions` — `list_permissions` (200)
- `POST /applications/{application_id}/permissions` — `create_permission` (201)
- `DELETE /applications/{application_id}/permissions/{permission_id}` — `delete_permission` (204)
- `GET /applications/{application_id}/roles` — `list_roles` (200)
- `POST /applications/{application_id}/roles` — `create_role` (201)
- `DELETE /applications/{application_id}/roles/{role_id}` — `delete_role` (204)
- `GET /applications/{application_id}/roles/{role_id}/permissions` — `list_role_permissions` (200)
- `DELETE /applications/{application_id}/roles/{role_id}/permissions/{permission_id}` — `revoke_permission_from_role` (204)
- `POST /applications/{application_id}/roles/{role_id}/permissions/{permission_id}` — `assign_permission_to_role` (204)
- `GET /applications/{application_id}/users/{user_id}/roles` — `list_user_roles` (200)
- `DELETE /applications/{application_id}/users/{user_id}/roles/{role_id}` — `revoke_role_from_user` (204)
- `POST /applications/{application_id}/users/{user_id}/roles/{role_id}` — `assign_role_to_user` (204)
- `GET /health` — `health` (200)
- `GET /openapi.json` — `openapi_json` (200)
- `POST /permissions/evaluate` — `evaluate_permission` (200, 401, 403, 503)
- `GET /users/{user_id}/roles` — `list_user_roles_across_applications` (200)
- `GET /version` — `version` (200)

### device

- `GET /device` — `device` (200)
- `GET /devices` — `list_devices` (200)
- `GET /devices/endpoints` — `list_approved_device_endpoints` (200, 401, 403)
- `GET /devices/endpoints/{endpoint_id}` — `lookup_device_endpoint` (200, 401, 403, 404)
- `POST /devices/enrollments` — `enroll_device` (200)
- `GET /devices/identities/{device_id}` — `lookup_device_identity` (200)
- `GET /devices/pairing-accepting` — `pairing_acceptance` (200)
- `PUT /devices/pairing-accepting` — `set_pairing_acceptance` (200)
- `POST /devices/revoke-self` — `revoke_self` (200)
- `GET /devices/trusted` — `trusted_devices` (200)
- `PUT /devices/{device_id}/owner` — `transfer_device` (204)
- `GET /devices/{device_id}/restriction` — `get_device_restriction` (200)
- `PUT /devices/{device_id}/restriction` — `put_device_restriction` (204)
- `DELETE /devices/{device_id}/selection` — `delete_device_selection` (204)
- `PUT /devices/{device_id}/selection` — `put_device_selection` (204)
- `DELETE /devices/{device_id}/selection/{application_id}/{kind}/{resource_id}` — `delete_device_resource_selection` (204)
- `DELETE /devices/{id}` — `revoke_device` (204)
- `PATCH /devices/{id}` — `update_device` (200)
- `GET /health` — `health` (200)
- `GET /independent/node-joins/acceptance` — `independent_join_accepting` (200, 403)
- `PUT /independent/node-joins/acceptance` — `independent_join_set_accepting` (204, 400, 403)
- `POST /independent/node-joins/discover` — `independent_join_discover` (200, 400, 403, 503)
- `POST /independent/node-joins/history/request` — `independent_join_request_history` (202, 400, 403, 503)
- `GET /independent/node-joins/history/stream` — `independent_join_history_stream` (101, 400, 401, 403)
- `GET /independent/node-joins/pending` — `independent_join_pending` (200, 403, 409)
- `POST /independent/node-joins/verify` — `independent_join_verify` (204, 400, 403)
- `POST /independent/node-joins/{id}/approve` — `independent_join_approve` (202, 403, 409, 503)
- `POST /independent/node-joins/{id}/reject` — `independent_join_reject` (204, 403, 409)
- `GET /node-joins/acceptance` — `node_join_acceptance` (200, 401, 403, 404, 409, 503)
- `PUT /node-joins/acceptance` — `node_join_set_acceptance` (200, 401, 403, 404, 409, 503)
- `GET /node-joins/pending` — `node_join_pending` (200, 401, 403, 404, 409, 503)
- `POST /node-joins/{id}/approve` — `node_join_approve` (200, 401, 403, 404, 409, 503)
- `POST /node-joins/{id}/reject` — `node_join_reject` (204, 401, 403, 404, 409, 503)
- `GET /openapi.json` — `openapi_json` (200)
- `POST /replication/admission` — `replication_admission` (204, 401, 403, 503)
- `GET /replication/devices/{endpoint_id}/selections` — `selected_resources` (200, 401, 403, 503)
- `POST /replication/owner-cleanup` — `owner_cleanup` (204, 401, 403, 503)
- `POST /setup/owner/admission` — `owner_setup_admission` (204, 401, 503)
- `POST /setup/owner/cancel` — `owner_setup_cancel` (204, 401, 409, 503)
- `POST /setup/owner/challenge` — `owner_setup_challenge` (204, 400, 403, 409, 503)
- `POST /setup/owner/create` — `owner_setup_create` (202, 400, 401, 409, 503)
- `POST /setup/owner/history` — `owner_setup_history` (202, 403, 409)
- `POST /setup/owner/intent` — `owner_setup_recover` (200, 204, 401, 503)
- `POST /setup/owner/join` — `owner_setup_join` (202, 400, 401, 409, 502, 503)
- `GET /setup/owner/public` — `owner_setup_owner_public` (200, 409)
- `POST /setup/owner/register` — `owner_setup_register` (204, 400, 403, 409, 503)
- `GET /setup/owner/status` — `owner_setup_status` (200, 503)
- `GET /version` — `version` (200)

### storage

- `GET /` — `upgrade_socket` (101, 401, 503)
- `GET /databases` — `list_databases` (200, 401, 403, 503)
- `POST /databases` — `create_database` (201, 401, 403, 503)
- `POST /databases/ensure` — `ensure_database` (200, 400, 401, 403, 409, 410, 413, 422, 503)
- `DELETE /databases/{database_id}` — `delete_database` (204, 401, 403, 404, 503)
- `GET /databases/{database_id}` — `get_database` (200, 401, 403, 404, 503)
- `POST /databases/{database_id}/sql` — `execute_sql` (200, 400, 401, 403, 404, 409, 413, 503)
- `GET /filesystems` — `list_file_systems` (200, 401, 403, 503)
- `POST /filesystems` — `create_file_system` (201, 401, 403, 503)
- `DELETE /filesystems/{filesystem_id}` — `delete_file_system` (204, 401, 403, 404, 503)
- `GET /filesystems/{filesystem_id}` — `get_file_system` (200, 401, 403, 404, 503)
- `GET /filesystems/{filesystem_id}/residency` — `get_file_system_residency` (200, 401, 403, 404, 409, 500, 503)
- `PUT /filesystems/{filesystem_id}/residency` — `put_file_system_residency` (204, 401, 403, 404, 409, 500, 503)
- `GET /health` — `health` (200)
- `POST /independent/node-joins/history/request` — `independent_join_request_history` (202, 400, 403, 503)
- `GET /independent/node-joins/history/stream` — `independent_join_history_stream` (101, 400, 401, 403)
- `GET /openapi.json` — `openapi_json` (200)
- `POST /setup/owner/admission` — `owner_setup_admission` (204, 401, 503)
- `POST /setup/owner/cancel` — `owner_setup_cancel` (204, 401, 409, 503)
- `POST /setup/owner/challenge` — `owner_setup_challenge` (204, 400, 403, 409, 503)
- `POST /setup/owner/create` — `owner_setup_create` (202, 400, 401, 409, 503)
- `POST /setup/owner/history` — `owner_setup_history` (202, 403, 409)
- `POST /setup/owner/intent` — `owner_setup_recover` (200, 204, 401, 503)
- `POST /setup/owner/join` — `owner_setup_join` (202, 400, 401, 409, 502, 503)
- `GET /setup/owner/public` — `owner_setup_owner_public` (200, 409)
- `POST /setup/owner/register` — `owner_setup_register` (204, 400, 403, 409, 503)
- `GET /setup/owner/status` — `owner_setup_status` (200, 503)
- `GET /version` — `version` (200)

## Native lifecycle consumers

- `apps/app/src-tauri/src/native_host.rs`: generated owner-local `GET /idp/setup/owner/status` observation; framework OPTIONS startup TLS verification.
- `apps/app/src-tauri/src/setup_peer_revocation.rs`: IdP discovery, OAuth token exchange, signer revocation, and Device revocation. Preserve peer/Installation checks before sending bearer credentials.
- `apps/app/src/lib/common/state/idpClient.svelte.ts`: password/OAuth token acquisition and generated IdP operations.
- `apps/app/src/lib/common/state/devices.svelte.ts`: source Node-join acceptance/pending/approve/reject.
- `apps/app/src/lib/common/state/localhostBaseUrl.svelte.ts`: health/Setup reachability probes.
- `setupApi.ts`, `independentSetupApi.ts` and associated state modules: owner-local Setup clients in both layouts; inspect generated serializer use separately from handwritten operations.

## Boundaries and evidence

- Public metadata, health, discovery/OIDC and JWKS are in owner documents. Setup schema publication is in idp-setup. Framework GET-generated HEAD and CORS/OPTIONS are not separate application operations.
- Storage owner `/` (mounted `/storage`) and independent history/stream describe HTTP upgrades only. WebSocket messages, bounded binary history, Iroh pairing/ALPN and filesystem/SQL/KV synchronization are not generated HTTP implementations.
- setup_https_fixture.rs, full_node_probe.rs and qualification/test modules are test consumers, not production fallback adapters.
- Generated Rust configuration accepts reqwest 0.12 policy injection, 1 MiB default request/response limits, verified native-root HTTPS, no proxy/redirect, 5-second connect and 15-second request deadlines. Migrated consumers inject their existing transport clients, preserve stricter 64 KiB operation bounds, and retain token/domain validation.
- Evidence: production owner registration tests, independent complete-contract test, generated Rust and built JavaScript actual owner/Setup and unified Node-join listener tests, generated transport tests, generation-failure tests. No GUI/production/cross-platform acceptance is implied.

## Ticket 02 qualification notes

- A production-source audit found no remaining handwritten Rust owner request sends. The sole production `.send()` outside generated transport is the explicitly excluded Setup `OPTIONS` TLS probe. Remaining browser `fetch` calls are generic transport or the explicit external-provider OIDC branch.
- Rust generation preserves `allOf` + `oneOf` JWK composition instead of flattening every key variant into one required-field struct. Both SDKs honor closed object schemas; the Rust operation success path accepts documented success statuses only. TypeScript token grant literals include every grant in the canonical form union.
- Canonical Storage `/` denotes the mounted owner base itself. Generated upgrade request builders do not append a trailing slash to that complete base; the actual Storage listener distinguishes these paths. History and Storage request builders own HTTP URL/header/query construction, not the subsequent WebSocket/binary protocols.
- Red/green regressions cover request bounds before credential transmission, exact cleanup acknowledgement status, valid JWK variants against the actual IdP router, discovery refusing a foreign credential recipient, and built Storage consumer list/create/upgrade against actual routers.
- Existing actual independent-Create/Join HTTPS fixtures exercise operation-bound registration, signed discovery, source enrollment, generated history handshakes, public import, and source-offline reopen. Existing service/local-owner and transport-failure tests retain the security boundaries.
- Full-suite, feature-powerset, formatting/lint, reproducibility and complexity-check results and any bounded-run limitations are recorded in ticket 02. These checks do not imply native GUI, production deployment or cross-platform acceptance.

## Historical ticket 03 reconciliation (superseded product layout)

At ticket 03 time, re-audited 128 method/path operations across the then-ten committed documents against production owner registration and the separately composed Setup, independent-owner, history and Node-join routers. The gate now runs the existing owner registration comparisons, complete independent Setup contract test and fresh-host publication/Setup security tests in addition to actual SDK listeners. Storage resources/socket and history use their existing runtime/contract adapters; the inventory is navigation, not an independent coverage framework.

Re-inspected Rust request sends and browser/native fetch call sites across crates, the native app and packages. The remaining production HTTP send outside generated SDK transport is the Setup OPTIONS TLS probe. Other sends are test fixtures, channels, database execution or non-HTTP WebSocket messages. OIDC raw fetches remain only in explicit external-provider mode; project-owned applications opt into generated operations. Deep-link/native fetch is arbitrary transport. No additional undocumented operation or handwritten project-owned fallback was found. Runtime owner calls remain direct/current-state; publishing schemas adds no authorization or central/live dependency.

The current shared gate repeats complete export and both-language generation, compares the working-tree artifacts (and optionally HEAD/staged/untracked drift with `--committed`), builds all SDKs and real Rust consumers, loads every built JavaScript export, and reruns failure-preservation/transport/authorization qualification at the existing seams. See ticket 03 for exact fresh-run evidence and bounded validation limits; ticket 02 retains the broader local/offline and independent Join evidence without rerunning expensive full suites.
