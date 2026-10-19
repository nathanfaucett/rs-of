# Node API contracts

These are current contracts at integration `41ad8bc47`, not full GUI/provider/deployment acceptance. See [operations](operations.md#qualification-boundary) for evidence and remaining checks, [setup worker design](setup-workers.md) for lifecycle/credential ownership, and [security](security.md) for trust boundaries.

## Owner contracts

IdP owns Users, credentials, Applications, OAuth Clients and token-signing trust. Management owns Devices, endpoint-ID bindings, User ownership, approval/enrollment, transfer/revocation, grants, restrictions and selections. Storage owns resource catalogs, namespace isolation, resources and local Residency. Identity administration goes directly to IdP; Device administration goes directly to Management, not an IdP proxy.

Paths below are owner-relative. Unified hosting mounts `/idp`, `/management` and `/storage` once; independent deployments use their actual configured bases/prefixes.

| Owner      | Contract                                                                            | Authorization boundary                                                                                                                                       |
| ---------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| IdP        | User, Application and Client administration                                         | Verified User; exact audience, action and Application/Installation target                                                                                    |
| IdP        | Active-User lookup                                                                  | Authenticated service Client with `idp.user.lookup`                                                                                                          |
| IdP        | `POST /replica-signers`, `PUT`/`DELETE /replica-signers/{member_id}`                | Exact Installation signer enroll/rotate/revoke permissions; public approval supplies no private capability                                                   |
| Management | Device administration; `GET /devices/identities/{device_id}` and endpoint discovery | Current Management Device binding and active IdP User; service lookup/list require `management.device.lookup` / `management.device.list`                     |
| Management | `PUT /devices/{device_id}/owner`                                                    | Exact Installation `management.devices.transfer`; active new User; 204 is mutation, not cleanup completion                                                   |
| Management | `POST /permissions/evaluate`                                                        | Same verified User, or separately registered evaluator Client with `management.permission.evaluate`; `idp-management` audience and exact typed action/target |
| Management | Selection, resource-specific deselection and restriction read/update                | Distinct permissions plus current Management Device/owner binding; ownership is not administrator restriction authority                                      |
| Storage    | Database/filesystem CRUD and filesystem Residency                                   | User/Application namespace, kind audience, exact read/write action and current owner/policy/cleanup gates                                                    |

`/replica-signers` retains its historical spelling; it is not a privileged replica role. Endpoint possession proves transport identity, not permission. An approved endpoint needs no individual IdP token signer or full Node enrollment. Unknown/revoked endpoints deny even with valid OAuth evidence; token issuer/Principal/signer checks remain independently mandatory. Approval alone grants no resource access. See [Storage API](../crates/storage-service/docs/api.md).

`GET /replication/devices/{endpoint_id}/selections` requires a Client token with the configured Storage audience and `management.replication.read`. It returns `deviceId`, `endpointId`, `ownerSubject`, `adminAllowed`, `resources`, and optional `pendingCleanup`. Missing trust/policy or an inactive/revoked/conflicted binding denies. Pending cleanup returns no resources and `adminAllowed:false`; the concrete retirement evidence is exposed only to that authorized service caller, never as permission to use resources. Ordinary restriction likewise grants nothing.

Independent Storage retires both public SQL/KV and filesystem owner copies before sending `POST /replication/owner-cleanup` through its existing Storage→Management service Client. The request is exactly `{cleanupId, deviceId, endpointId, oldOwnerSubject, newOwnerSubject}`; acknowledgement requires the same configured audience, Client principal and `management.replication.admit`. Management compares the durable pending cleanup ID, retained old policy owner and current approved/non-revoked Device/endpoint/new-owner binding inside one local transaction. Changing the owner or endpoint while cleanup is delayed denies; replaying an acknowledgement from an earlier occurrence of the same owner tuple cannot clear a later fence. A lost successful response supports an idempotent no-op retry when no cleanup remains pending.

Cleanup rebinds an empty **restricted** policy, never administrator allowance, selections or old content. Only the configured local Management endpoint can drive this Storage runtime's cleanup; remote-peer policy reads remain read-only. Interrupted cleanup reuses existing Storage retirement evidence and the same Management pending fact/ID after stopped reopen. Local persistence failure, service denial and shutdown deny without acknowledging unfinished retirement. The pending table now includes `cleanup_id`; an older/malformed retained layout is not silently migrated or repaired, and its fence must not be deleted to manufacture freshness.

Independent Storage uses fresh authenticated Management responses for operation/frame/response checks; local denial never falls back remotely. Only service tokens are cached. Cancellation and local-copy retirement are rechecked after asynchronous evaluation before queued frames are released. The new transfer implementation and tests are recorded in [the transfer receipt](../.scratch/ui-driven-node-bootstrap/evidence/independent-transfer.md); Rust/HTTPS execution remains pending parent build permission, not qualified by this documentation.

## Prefixes and application identity

Use verified complete service bases and append owner-relative paths once. Do not add `/idp/idp`, `/management/management` or `/storage/storage` compatibility routes. Canonical Issuer, public Application URI, OAuth audience, HTTPS address, endpoint ID, Device ID, Principal and token signer are distinct.

An independent IdP can use `management_permissions` with verified HTTPS and its separately registered evaluator Client. It validates the exact echoed decision/audit identity; transport/authentication/policy failure denies without database fallback. Bundled evaluation uses local Management and needs no remote evaluator or confidential relationship Client. `idp-management` is the Management Application audience, not its transport prefix. See [service relationships](operations.md#secrets-and-local-identity).

## Setup, join and readiness

The existing web app supports unified and independent-owner Create/Join without Tauri. Desktop bundled selection uses the same HTTPS setup client/shared worker; selected remote bases must never be replaced by bundled bases. Native retains protected code retrieval, trust/elevation, host observation, retry/reset/reference recovery and Device marker completion, not separate Create/Join/approval orchestration. `/setup/bootstrap`, the old `/lidp/setup/status` probe and native mutation commands are removed, with no wrappers.

Normal protected APIs always enforce their own authentication and exact authorization. Listener/status/approval/completion markers, sign-in and Residency acknowledgements are not readiness or resource authority. Current owner trust, protected signing capability and retained state must verify; no global authentication-enable switch or peer freshness lease exists.

### Admission and unified setup

A fresh unified HTTPS host exposes `GET /idp/setup/status` with `stage`, `state`, `ready` and `owners:["idp","management","storage"]`. Independent status has the actual singleton owner. Invalid admission/state reports 503 rather than fresh permission. `POST /setup/admission` (unified `/idp/setup/admission`) returns 204 for a valid code; that verifies capability only, not completion.

Retrieve the host-generated 256-bit code through **protected host/deployment management**. Send exactly one `Authorization: Setup <64 lowercase hex>` header to that code's exact validated target. Independent owners have separate codes even on one origin. Codes are not OAuth bearers; never expose them in status, URLs, logs, ordinary console output or browser persistence. Browser setup requires the configured exact HTTPS UI Origin; the bundled native host additionally admits its exact Tauri Origin. No cookies or credentialed CORS. Origin/TLS/loopback alone is not admission.

Unified `POST /idp/setup/create` and `/join` return **202 only after durable exact intent binding**. Accepted work belongs to the host and survives request/browser disconnect. Create takes nonce, administrator, Node and transient password; Join takes nonce, Node and the canonical pinned peer endpoint, authenticates issuer discovery before binding and does not seed a new administrator. Poll status; `creating`/`joining` includes retained owner reopen, not just initial work. Only verified ready status includes unified public login metadata.

`POST /idp/setup/intent` requires current Setup authorization: 200 returns the exact nonsecret retained Create/Join intent, 204 means none, malformed evidence returns 503 without repair. Recovery does not resume work. Recheck scope/status before adopting it; exact retry requires newly entered capability and any required Create password, never a replacement nonce/identity. Complete-but-unretired restart stays unfinished until exact authorized finalization. Retired codes deny setup recovery/new mutations; the unified completed Create retry path separately uses ordinary password authentication and returns 200.

`POST /idp/setup/cancel` requires current code and the exact active intent; 409 means no matching active operation, 204 means cancellation signaled, **not rollback or finished cleanup**. Cancellation/finalization serialize with one winner. Closing a page is not cancellation. Missing/corrupt material or conflicting purpose/peer/identity denies unchanged. Completed reopen is existing-only and works without the source; it never reseeds or silently repairs.

Optional CLI Create/Join remains an alternate entry point, not an app prerequisite. A neutral awaiting-choice host cannot be adopted by those CLI commands; they refuse rather than overwrite its prerequisites. See [operations](operations.md#automatic-unified-cli-bootstrap).

### Independent Create worker contract

Each owner exposes `POST /setup/create`, `/setup/intent`, `/setup/cancel` on its existing verified HTTPS listener. Every mutation/recovery requires **that owner's own** Setup code and configured Origin when present. The app sends the same exact nonsecret tuple to all three owners:

- `purpose:"create"`, `operation_nonce` (32 lowercase hexadecimal characters), `administrator`, `node`;
- canonical distinct HTTPS `idp`, `management`, `storage` bases;
- `issuer` equal to the selected IdP base; `peer:null`.

Only IdP receives transient `password`. Each owner durably binds before returning 202; exact active retries return 202, conflicting tuples 409. Status states are `awaiting_choice`, `creating`, `joining`, `unfinished`, `ready`; only ready sets `ready:true`. Recovery returns the retained tuple or 204, never credentials. Cancel returns 204 when signaled. Retired codes deny mutation/recovery; completed-but-unretired state needs exact authorized finalization.

`GET /setup/independent/public` publishes operation-bound public registrations/facts, including IdP `facts.login_client`. The app correlates the operation, issuer and all target bases across every owner before navigating to sign-in: three unrelated ready flags are insufficient. Recovery tolerates genuinely unbound awaiting-choice owners and reads public evidence for already-ready/retired owners without sending their retired codes.

Worker-only `/setup/independent/challenge` and `/setup/independent/register` exchange consumer credentials directly over verified HTTPS, with redirects disabled and outstanding exact single-use capability checks. They return no secret/challenge to the caller and are excluded from the browser SDK. Public metadata/nonce equality is not registration authority. See [worker design](setup-workers.md).

Workers reuse owner-local handles for seed/serving, retain one actual Management Device/endpoint, and create no Storage resources or per-process Nodes/Devices. Fresh Create needs no peer/history/content exchange. Separate listeners/stores do not imply separate domain enrollment or copied endpoint keys.

### Independent Join worker contract

Each owner accepts `POST /setup/join` under its own Setup code with:

```json
{
  "purpose": "join",
  "operation_nonce": "0123456789abcdef0123456789abcdef",
  "administrator": "",
  "node": "Joining Node",
  "idp": "https://target.example/idp",
  "management": "https://target.example/management",
  "storage": "https://target.example/storage",
  "issuer": "",
  "peer": "https://source.example/management#<canonical-pinned-endpoint-id>"
}
```

`peer` is the source's exact HTTPS **Management base**, prefix included, plus the independently verified canonical endpoint fingerprint; it is not unified's bare endpoint choice. Every target authenticates signed purpose/nonce-bound discovery before initial durable admission/202. Empty issuer resolves to the source Canonical Issuer; a conflicting supplied issuer denies. Password is rejected; administrator must be empty. No source password verifier/private signer is imported or replacement administrator seeded.

Code-authorized `POST /setup/intent` has no body: 200 returns the exact tuple with authenticated nonempty issuer, 204 means none. Retry/cancel use that tuple. A retained retry with empty issuer reuses the retained issuer without rediscovery, permitting source-offline finalization. Purpose, nonce, names, bases, peer or nonempty issuer conflicts deny. Cancel returns 204 for signaling or 409 without a matching active operation. Retired codes return 401.

Status has exactly `stage`, `state`, `ready`, singleton `owners`, e.g. `{"stage":"installation","state":"joining","ready":false,"owners":["management"]}`. It contains no offer or login Client; IdP public login discovery uses `/setup/independent/public` at `facts.login_client`. Poll/correlate all owners. Public history import is approval-bound, owner-allowlisted and distinct from resource-content sync. Completed stopped reopen uses retained state without source contact/reseeding.

The durable `initialized.json` fence and `worker.json` must be preserved, even before domain creation. Lost initialized worker evidence refuses without replacement IDs/provider writes. Callback provider/network work occurs outside the lifecycle gate, then revalidates cancellation, retirement and the exact active operation before progress writes. Normal OAuth/Bearer authentication is unchanged.

### Public operator-login Client

Create/Join retain a public Client with token endpoint authentication `none`, password/refresh/token-exchange grants and `openid profile email` login scopes. Unified ready status supplies it; independent IdP public facts supply `facts.login_client` after correlated all-owner completion; CLI prints its actual ID/scopes/audiences. No manual registration or confidential secret is needed.

Use ordinary `POST /idp/oauth2/token` form fields `grant_type=password`, actual `client_id`, existing User `username`, securely handled `password`, allowed `scope`. Token exchange selects the permitted audience/action before owner calls; the Client audience allowlist is not a User permission grant. Independent Join does not copy source password credentials: source-password login at the target is not established by public import. See [login operations](operations.md#public-operator-login).

### Production Node-join approval routes

Append these owner-relative paths to the **selected source Management base**. Unified mounts them at `/management/node-joins/...`; obsolete IdP paths are absent.

| Method/path                     | Contract                                                                              |
| ------------------------------- | ------------------------------------------------------------------------------------- |
| `GET /node-joins/acceptance`    | `{"accepting":true/false}`                                                            |
| `PUT /node-joins/acceptance`    | Same request; unified returns state, independent returns 204 (read back)              |
| `GET /node-joins/pending`       | Array of `offer_id`, `authenticated_endpoint`, exact public `request`                 |
| `POST /node-joins/{id}/approve` | Unified 200 `NodeJoinApproval`; independent 202 host-owned acceptance, not completion |
| `POST /node-joins/{id}/reject`  | 204                                                                                   |

Every route requires an ordinary authorized User bearer with **Management audience** (`idp-management`) and both exact Device pairing-update and separate signer-enrollment permissions. IdP-audience tokens or target Setup codes do not authorize source approval. Unified missing bearer is 401, permission denial 403, missing offer 404, conflict 409, other owner failure 503; independent missing/unauthorized User evidence denies with 403.

Approval binds distinct Device, endpoint, owner, signer member/key and Installation identities through their respective owners. It preserves existing restrictions/selections, initializes only absent default policy, refreshes admission and rechecks current authorization/enrollment before signed reply. Failure after enrollment stays partial for exact retry; rejected enrollment creates no policy. Discovery/acceptance/pairing is not membership or resource permission. All platforms use these HTTPS routes, not native source mutation commands.

## Generated clients

Generate complete actual-router contracts through `scripts/generate-openapi-client.mjs`; never hand-edit generated files. Current regenerated normal owners expose IdP **38** operations (identity, active User, signer; no Devices) and Management **37** (including Device authority). Storage was not regenerated in this reconciliation. Host-composed Node-join coordination is not implied by normal owner OpenAPI.

The separate unified `@aicacia/idp-client/setup` SDK uses a verified **host origin** because its paths contain `/idp`. In `packages/idp-client`, use `OPENAPI_SPEC_URL=https://node.example/idp/setup/openapi.json pnpm run openapi:setup`, then the normal package build.

The generated `@aicacia/idp-client/independent` SDK (`pnpm run openapi:independent` with `OPENAPI_SPEC_URL` pointing to the actual exported independent document) uses verified **owner bases**. Its seven public paths are `/setup/status`, `/setup/admission`, `/setup/create`, `/setup/join`, `/setup/intent`, `/setup/cancel`, `/setup/independent/public`; operations are `status`, `admission`, `create`, `join`, `recover`, `cancel`, `ownerPublic`. The app consumes its generated serializers, including `operationNonce` → `operation_nonce`. Internal challenge/register/history routes are deliberately excluded. Normal IdP generation preserves both setup subtrees. Run generation serially and build package exports before consumption.

Missing/incomplete schemas, generator or staged formatting failures must leave checked-in sources unchanged. See [owner-client evidence](../.scratch/ui-driven-node-bootstrap/evidence/34-owner-client-cutover.md) and [independent SDK/web evidence](../.scratch/ui-driven-node-bootstrap/evidence/35-independent-web.md); contract tests are not full GUI/native acceptance.

## Storage client consumption

Use explicit `idpBaseUrl` and `storageBaseUrl`, not legacy `baseUrl`/`audience` or origin fallback. Password-manager requires complete `PUBLIC_LIDP_BASE_URL` and `PUBLIC_STORAGE_BASE_URL`; `PUBLIC_STORAGE_AUDIENCE` is no longer consumed. Filesystem token exchange uses `resource=storage:filesystems`; catalog/socket paths append once.

Storage exports resolve through `esm`/`types`. Build explicitly before publication (no automatic `prepack`) and distribute complete compiled generated dependencies. Workspace source tests do not prove installed exports; current inherited Storage export errors are recorded in [operations](operations.md#qualification-boundary).

## Source anchors

- Shared setup/admission/independent contracts: `crates/api/src/`, including `independent_openapi.rs`.
- IdP and Management owner routers: `crates/idp-server/src/router/`, `crates/management-server/src/router/`.
- Unified worker/runtime/source approval: `crates/unified-server/src/api_create.rs`, `runtime.rs`, `node_join_source.rs`.
- Native lifecycle boundary: [NATIVE-SETUP.md](../apps/app/src-tauri/NATIVE-SETUP.md).
- Frontend setup transport: `apps/app/src/lib/common/state/setupApi.ts`, `independentSetupApi.ts`.
