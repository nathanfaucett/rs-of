# HTTP route and consumer inventory

Ticket 02 migration: project-owned production HTTP operations use generated clients; CI enforcement remains ticket 03. This inventory is navigation, not sole coverage evidence: state-free owner registration tests and generated SDK actual-router listener tests (including host-composed Node-join) also exercise these contracts.

## Topologies

Normal owner contracts use verified complete owner bases (defaults append `/idp`, `/management`, `/storage` once). Unified Setup paths include `/idp`, Node-join paths include `/management`, so use host origins for those two documents. Independent-owner documents supplement the corresponding normal owner and use complete owner bases, including any configured prefix. Owner-admission-only listeners use owner-setup. `https://localhost` is a deterministic placeholder, not identity or trust.

## Mounted application operations

### idp-independent

- `POST /setup/admission` — `admission`
- `POST /setup/cancel` — `cancel`
- `POST /setup/create` — `create`
- `POST /setup/independent/challenge` — `challenge`
- `POST /setup/independent/history` — `history`
- `GET /setup/independent/public` — `owner_public`
- `POST /setup/independent/register` — `register`
- `POST /setup/intent` — `recover`
- `POST /setup/join` — `join`
- `GET /setup/status` — `status`

### idp-setup

- `POST /idp/setup/admission` — `admission`
- `POST /idp/setup/cancel` — `cancel`
- `POST /idp/setup/create` — `create`
- `POST /idp/setup/intent` — `intent`
- `POST /idp/setup/join` — `join`
- `GET /idp/setup/openapi.json` — `setup_openapi`
- `GET /idp/setup/status` — `status`

### idp

- `GET /.well-known/jwks.json` — `jwks`
- `GET /.well-known/openid-configuration` — `openid_configuration`
- `GET /applications` — `list_applications`
- `POST /applications` — `create_application`
- `DELETE /applications/{application_id}` — `delete_application`
- `GET /applications/{application_id}` — `get_application`
- `PUT /applications/{application_id}` — `update_application`
- `DELETE /clients/{client_id}/keys` — `revoke_client_keys`
- `GET /clients/{client_id}/keys` — `list_client_keys`
- `POST /clients/{client_id}/keys/rotate` — `rotate_client_key`
- `GET /health` — `health`
- `GET /oauth2/auth` — `authorize_query`
- `POST /oauth2/auth` — `authorize_json`
- `POST /oauth2/auth/allowed-for-user` — `is_allowed_for_user`
- `POST /oauth2/auth/approve-for-user` — `approve_for_user`
- `POST /oauth2/device/auth` — `device_auth`
- `GET /oauth2/device/verify` — `device_verify`
- `POST /oauth2/introspect` — `introspect`
- `POST /oauth2/register` — `register`
- `DELETE /oauth2/register/{client_id}` — `delete_register`
- `GET /oauth2/register/{client_id}` — `get_register`
- `PUT /oauth2/register/{client_id}` — `put_register`
- `POST /oauth2/revoke` — `revoke`
- `GET /oauth2/sessions/logout` — `sessions_logout`
- `POST /oauth2/token` — `token`
- `GET /openapi.json` — `openapi_json`
- `POST /replica-signers` — `enroll_replica_signer`
- `DELETE /replica-signers/{member_id}` — `revoke_replica_signer`
- `PUT /replica-signers/{member_id}` — `rotate_replica_signer`
- `GET /userinfo` — `userinfo`
- `DELETE /users/{user_id}` — `delete_user`
- `GET /users/{user_id}` — `get_user`
- `PUT /users/{user_id}` — `update_user`
- `GET /users/{user_id}/active` — `active_user`
- `GET /users/{user_id}/consents` — `list_consents`
- `DELETE /users/{user_id}/consents/{consent_id}` — `revoke_consent`
- `PUT /users/{user_id}/password` — `reset_password`
- `GET /version` — `version`

### independent-idp

- `POST /node-joins/enroll` — `enroll_node_join`
- `POST /node-joins/history/request` — `request_history`
- `GET /node-joins/history/stream` — `history_stream`

### independent-management

- `GET /node-joins/acceptance` — `accepting`
- `PUT /node-joins/acceptance` — `set_accepting`
- `POST /node-joins/discover` — `discover`
- `POST /node-joins/history/request` — `request_history`
- `GET /node-joins/history/stream` — `history_stream`
- `GET /node-joins/pending` — `pending`
- `POST /node-joins/verify` — `verify`
- `POST /node-joins/{id}/approve` — `approve`
- `POST /node-joins/{id}/reject` — `reject`

### independent-storage

- `POST /node-joins/history/request` — `request_history`
- `GET /node-joins/history/stream` — `history_stream`

### management

- `GET /applications/{application_id}/permissions` — `list_permissions`
- `POST /applications/{application_id}/permissions` — `create_permission`
- `DELETE /applications/{application_id}/permissions/{permission_id}` — `delete_permission`
- `GET /applications/{application_id}/roles` — `list_roles`
- `POST /applications/{application_id}/roles` — `create_role`
- `DELETE /applications/{application_id}/roles/{role_id}` — `delete_role`
- `GET /applications/{application_id}/roles/{role_id}/permissions` — `list_role_permissions`
- `DELETE /applications/{application_id}/roles/{role_id}/permissions/{permission_id}` — `revoke_permission_from_role`
- `POST /applications/{application_id}/roles/{role_id}/permissions/{permission_id}` — `assign_permission_to_role`
- `GET /applications/{application_id}/users/{user_id}/roles` — `list_user_roles`
- `DELETE /applications/{application_id}/users/{user_id}/roles/{role_id}` — `revoke_role_from_user`
- `POST /applications/{application_id}/users/{user_id}/roles/{role_id}` — `assign_role_to_user`
- `GET /device` — `device`
- `GET /devices` — `list_devices`
- `GET /devices/endpoints` — `list_approved_device_endpoints`
- `GET /devices/endpoints/{endpoint_id}` — `lookup_device_endpoint`
- `POST /devices/enrollments` — `enroll_device`
- `GET /devices/identities/{device_id}` — `lookup_device_identity`
- `GET /devices/pairing-accepting` — `pairing_acceptance`
- `PUT /devices/pairing-accepting` — `set_pairing_acceptance`
- `POST /devices/revoke-self` — `revoke_self`
- `GET /devices/trusted` — `trusted_devices`
- `PUT /devices/{device_id}/owner` — `transfer_device`
- `GET /devices/{device_id}/restriction` — `get_device_restriction`
- `PUT /devices/{device_id}/restriction` — `put_device_restriction`
- `DELETE /devices/{device_id}/selection` — `delete_device_selection`
- `PUT /devices/{device_id}/selection` — `put_device_selection`
- `DELETE /devices/{device_id}/selection/{application_id}/{kind}/{resource_id}` — `delete_device_resource_selection`
- `DELETE /devices/{id}` — `revoke_device`
- `PATCH /devices/{id}` — `update_device`
- `GET /health` — `health`
- `GET /openapi.json` — `openapi_json`
- `POST /permissions/evaluate` — `evaluate_permission`
- `POST /replication/admission` — `replication_admission`
- `GET /replication/devices/{endpoint_id}/selections` — `selected_resources`
- `POST /replication/owner-cleanup` — `owner_cleanup`
- `GET /users/{user_id}/roles` — `list_user_roles_across_applications`
- `GET /version` — `version`

### node-join

- `GET /management/node-joins/acceptance` — `acceptance`
- `PUT /management/node-joins/acceptance` — `set_acceptance`
- `GET /management/node-joins/pending` — `pending`
- `POST /management/node-joins/{id}/approve` — `approve`
- `POST /management/node-joins/{id}/reject` — `reject`

### owner-setup

- `POST /setup/admission` — `admission`
- `GET /setup/status` — `status`

### storage

- `GET /` — `upgrade_socket`
- `GET /databases` — `list_databases`
- `POST /databases` — `create_database`
- `DELETE /databases/{database_id}` — `delete_database`
- `GET /databases/{database_id}` — `get_database`
- `GET /filesystems` — `list_file_systems`
- `POST /filesystems` — `create_file_system`
- `DELETE /filesystems/{filesystem_id}` — `delete_file_system`
- `GET /filesystems/{filesystem_id}` — `get_file_system`
- `GET /filesystems/{filesystem_id}/residency` — `get_file_system_residency`
- `PUT /filesystems/{filesystem_id}/residency` — `put_file_system_residency`
- `GET /health` — `health`
- `GET /openapi.json` — `openapi_json`
- `GET /version` — `version`

## Migrated outbound consumers

| Source | Generated operations and retained boundary |
| --- | --- |
| `crates/management-service/src/hosted_control_plane.rs` | IdP `token`, `introspect`, `active_user`, `jwks`; Storage `get_database`, `get_file_system`. Retains cached service credentials, signature/claim/resource checks, and direct current-state local IdP/Management calls. |
| `crates/storage-server/src/idp_client.rs` | IdP `token`, `introspect`. Injects the existing TLS/deadline client and 64 KiB response bound; retains local inspector, renewal cache, issuer/type/scope/expiry checks and current owner binding. |
| `crates/storage-server/src/management_client.rs` | IdP `token`; Management `list_approved_device_endpoints`, `selected_resources`, `replication_admission`, `owner_cleanup`. Retains renewal, 64 KiB bounds, canonical/unique endpoint and resource-policy checks. |
| `crates/idp-server/src/management_permission_client.rs` | Management `evaluate_permission`. Token issuance/validation remains a direct IdP owner call; returned request, actor and service audit identity are rechecked. |
| `crates/api/src/independent_create.rs` | Independent Setup `owner_public`, bounded to 64 KiB; exact operation, owner and absence of Client secrets are rechecked. |
| `crates/api/src/independent_exchange.rs` | Independent Setup `challenge`, `register`; exact recipient, active operation, one-use challenge, TTL and cancellation checks remain. |
| `crates/api/src/independent_history.rs` | Source-owner `request_history`, selected for the IdP or Management owner; exact approval and callback nonce remain application state. |
| `crates/api/src/independent_peer.rs` | Management-owner `discover`; verifies the pinned endpoint signature and exact discovery nonce. |
| `crates/management-service/src/join_history.rs` | Independent Setup `history` callback and source-owner `history_stream_request` generated handshake builder. Native-root WebSocket connection and bounded binary SQL protocol remain separate. |
| `crates/idp-server/src/independent_join.rs` | Management-owner `verify`; exact current enrollment checks remain on both sides. |
| `crates/management-server/src/independent_join.rs` | IdP-owner `enroll_node_join`; permission and operation checks still run before and after enrollment. |
| `crates/api/src/setup_server.rs` | Audited exclusion: production sends framework `OPTIONS` to verify native TLS without running remote readiness. There is no production GET operation here; embedded test requests remain test fixtures. |
| `crates/unified-server/src/api_create.rs` | IdP `token` for an exact completed-Create password retry. Existing host lifecycle/domain checks remain; there is no independent status GET in this module. |
| `crates/unified-server/src/bootstrap_verification.rs; runtime.rs; owner_health.rs` | Generated owner `health` probes using the existing verified transport. Runtime owner authorization remains direct and in-process. |
| `apps/app/src-tauri/src/native_host.rs` | Unified Setup `status` for observation and bounded startup verification, including the documented 503 unavailable status. |
| `apps/app/src-tauri/src/setup_peer_revocation.rs` | IdP `openid_configuration`, `token`, `revoke_replica_signer`; Management `revoke_device`. Checks peer/Installation identity before transmitting the User token, and retains the target until both owners acknowledge. |
| `apps/app/src/lib/common/state/setupApi.ts; independentSetupApi.ts` | Unified and independent generated status, admission, Create/Join, intent recovery, cancel and public metadata operations. Generated decoders feed existing exact-intent/readiness/login-Client checks; requests omit credentials and forbid redirects/cache reuse. |
| `apps/app/src/lib/common/state/devices.svelte.ts` | Generated host-composed Node-join operations, or independent-owner operations for an arbitrary complete Management base. Current User audience exchange and offer/endpoint/operation checks remain. |
| `apps/app/src/lib/common/state/idpClient.svelte.ts; localhostBaseUrl.svelte.ts` | Generated password `token`, owner token exchange, IdP `health` and Setup `status` discovery. Other normal owner wrappers already use generated SDK operations. |
| `packages/storage-client/src/index.ts` | Generated IdP `token`, Storage `list_file_systems`, `create_file_system` and `upgrade_socket` request options. The native browser WebSocket implementation supplies protocol handshake headers; the generated builder owns its path/query. |
| `packages/oidc-client/src/OidcClient.ts` | Explicit `projectOwned: true` uses generated discovery, userinfo, token and registration operations. Both project applications select this mode. Discovery endpoints must match the generated owner endpoint before credentials are sent. External-provider/native-protocol behavior is a separate supported mode, not a project-operation fallback. No handwritten JWKS or revoke operation exists in this package at this revision. |
| `packages/native-fetch; crates/tauri-plugin-fetch-api; apps/app/src/lib/common/util/handleDeepLink.ts` | Audited exclusions: generic arbitrary-request/deep-link transport, not owner operation implementations. Project-owned callers above use generated operations; the bridge does not gain a hand-maintained owner endpoint dispatcher. |

### Browser/native wrapper locations

- `apps/app/src/lib/common/state/devices.svelte.ts`
- `apps/app/src/lib/common/state/idpClient.svelte.ts`
- `apps/app/src/lib/common/state/independentSetupApi.ts`
- `apps/app/src/lib/common/state/localhostBaseUrl.svelte.ts`
- `apps/app/src/lib/common/state/managementClient.svelte.ts`
- `apps/app/src/lib/common/state/setupApi.ts`
- `apps/app/src/lib/common/util/handleDeepLink.ts`
- `apps/app/src/routes/(auth)/+layout.ts`
- `apps/app/src/routes/setup/+page.ts`
- `apps/app/src/routes/setup/device/+page.ts`
- `apps/app/src/routes/signin/+page.ts`
- `apps/app/vitest.auth.config.ts`
- `apps/password-manager/src/lib/collections/secrets/storage.ts`
- `apps/password-manager/src/lib/common/state/idpClient.svelte.ts`
- `packages/oidc-client/src/OidcClient.ts`
- `packages/storage-client/src/index.ts`
- `packages/storage-client/src/owner/runtime.ts`

## Native lifecycle consumers

- `apps/app/src-tauri/src/native_host.rs`: unified `GET /idp/setup/status` observation and startup verification.
- `apps/app/src-tauri/src/setup_peer_revocation.rs`: IdP discovery, OAuth token exchange, signer revocation, and Management Device revocation. Preserve peer/Installation checks before sending bearer credentials.
- `apps/app/src/lib/common/state/idpClient.svelte.ts`: password/OAuth token acquisition and generated IdP operations.
- `apps/app/src/lib/common/state/devices.svelte.ts`: source Node-join acceptance/pending/approve/reject.
- `apps/app/src/lib/common/state/localhostBaseUrl.svelte.ts`: health/Setup reachability probes.
- `setupApi.ts`, `independentSetupApi.ts` and associated state modules: unified and independent Setup wrappers; inspect generated serializer use separately from handwritten operations.

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
