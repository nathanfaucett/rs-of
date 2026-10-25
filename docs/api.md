# Node API contracts

Current owner/database boundaries and Device routes: [shared control-plane databases](control-plane.md).

These contracts include the final lifecycle review through integration `a4caa2f71`. Receipt 42 qualifies the independent same-host Ubuntu/API/mock-provider seams; [receipt 43](../.scratch/ui-driven-node-bootstrap/evidence/43-final-native-shared.md) completes final native/shared-host qualification on unchanged Rust. [Receipt 44](../.scratch/ui-driven-node-bootstrap/evidence/44-acceptance.md) accepts the bounded spec and resolves tickets 02/03/04. Neither is full GUI, real OS installation/provider or platform/deployment certification. See [operations](operations.md#qualification-boundary) for evidence and remaining checks, [setup worker design](setup-workers.md) for lifecycle/credential ownership, and [security](security.md) for trust boundaries.

## Owner contracts

IdP owns Users, credentials, Applications, OAuth Clients and token-signing trust. Management owns roles, permissions, and assignments; Device owns Devices, endpoint-ID bindings, User ownership, approval/enrollment, transfer/revocation, restrictions, and selections. Storage owns resource catalogs, namespace isolation, resources and local Residency. Identity administration goes directly to IdP; Device administration goes directly to Device, not an IdP proxy.

Paths below are owner-relative. Unified hosting mounts `/idp`, `/management`, `/device`, and `/storage` once; independent deployments use their actual configured bases/prefixes.

| Owner      | Contract                                                                            | Authorization boundary                                                                                                                                       |
| ---------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| IdP        | User, Application and Client administration                                         | Verified User; exact audience, action and Application/Installation target                                                                                    |
| IdP        | Active-User lookup                                                                  | Authenticated service Client with `idp.user.lookup`                                                                                                          |
| IdP        | `POST /replica-signers`, `PUT`/`DELETE /replica-signers/{member_id}`                | Exact Installation signer enroll/rotate/revoke permissions; public approval supplies no private capability                                                   |
| Device     | Device administration; `GET /devices/identities/{device_id}` and endpoint discovery | Current Device binding and active IdP User; service lookup/list require `management.device.lookup` / `management.device.list`                                |
| Device     | `PUT /devices/{device_id}/owner`                                                    | Exact Installation `management.devices.transfer`; active new User; 204 is mutation, not cleanup completion                                                   |
| Management | `POST /permissions/evaluate`                                                        | Same verified User, or separately registered evaluator Client with `management.permission.evaluate`; `idp-management` audience and exact typed action/target |
| Device     | Selection, resource-specific deselection and restriction read/update                | Distinct permissions plus current Device/owner binding; ownership is not administrator restriction authority                                                 |
| Storage    | Database/filesystem CRUD and filesystem Residency                                   | User/Application namespace, kind audience, exact read/write action and current owner/policy/cleanup gates                                                    |

`/replica-signers` retains its historical spelling; it is not a privileged replica role. Endpoint possession proves transport identity, not permission. An approved endpoint needs no individual IdP token signer or full Node enrollment. Unknown/revoked endpoints deny even with valid OAuth evidence; token issuer/Principal/signer checks remain independently mandatory. Approval alone grants no resource access. See [Storage API](../crates/storage-service/docs/api.md).

`GET /replication/devices/{endpoint_id}/selections` requires a Client token with the configured Storage audience and `management.replication.read`. It returns `deviceId`, `endpointId`, `ownerSubject`, `adminAllowed`, `resources`, and optional `pendingCleanup:{cleanupId,deviceId,endpointId,oldOwnerSubject,newOwnerSubject}`. Missing trust/policy or an inactive/revoked/conflicted binding denies. Pending cleanup returns no resources and `adminAllowed:false`; the concrete retirement evidence is exposed only to that authorized service caller, never as permission to use resources. Ordinary restriction likewise grants nothing.

Independent Storage retires both public SQL/KV and filesystem owner copies before sending `POST /replication/owner-cleanup` through its existing Storage→Device service Client. The request is exactly `{cleanupId, deviceId, endpointId, oldOwnerSubject, newOwnerSubject}`; acknowledgement requires the same configured audience, Client principal and `management.replication.admit`. Device compares the durable pending cleanup ID, retained old policy owner and current approved/non-revoked Device/endpoint/new-owner binding inside one local transaction. Changing the owner or endpoint while cleanup is delayed denies; replaying an acknowledgement from an earlier occurrence of the same owner tuple cannot clear a later fence. A lost successful response supports an idempotent no-op retry when no cleanup remains pending.

Cleanup rebinds an empty **restricted** policy, never administrator allowance, selections or old content. Only the configured local Device endpoint can drive this Storage runtime's cleanup; remote-peer policy reads remain read-only. Interrupted cleanup reuses existing Storage retirement evidence and the same Device pending fact/ID after stopped reopen. Local persistence failure, service denial and shutdown deny without acknowledging unfinished retirement. The pending table now includes `cleanup_id`; an older/malformed retained layout is not silently migrated or repaired, and its fence must not be deleted to manufacture freshness.

Independent Storage uses fresh authenticated Device responses for operation/frame/response checks; local denial never falls back remotely. Only service tokens are cached. Cancellation and local-copy retirement are rechecked after asynchronous evaluation before queued frames are released. [Receipt 42](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md) qualifies actual independent HTTPS transfer and original-owner/replay checks; [receipt 41](../.scratch/ui-driven-node-bootstrap/evidence/41-management-cleanup-client.md) qualifies the generated cleanup SDK. These are recorded results, not tests rerun by this documentation. The acknowledgement route returns 204 on success, 401 for invalid bearer, 403 for stale binding/insufficient service permission, or 503 for unavailable owner evidence.

## Prefixes and application identity

Use verified complete service bases and append owner-relative paths once. Do not add `/idp/idp`, `/management/management` or `/storage/storage` compatibility routes. Canonical Issuer, public Application URI, OAuth audience, HTTPS address, endpoint ID, Device ID, Principal and token signer are distinct.

An independent IdP can use `management_permissions` with verified HTTPS and its separately registered evaluator Client. It validates the exact echoed decision/audit identity; transport/authentication/policy failure denies without database fallback. Bundled evaluation uses local Management and needs no remote evaluator or confidential relationship Client. `idp-management` is the Management Application audience, not its transport prefix. See [service relationships](operations.md#secrets-and-local-identity).

## Setup, join and readiness

Application routing consumes the typed, selected-target-bound `OnboardingObservation` rather than separate native stage, startup-error and login-Client reads. Bundled `get_onboarding_host` returns safe platform failure categories, typed shared status and the existing Client-bound Device preference requirement; browser/remote observation remains direct generated HTTPS. The native observation never contains a Setup code. See the [input, action and failure table](setup-workers.md#onboarding-observation-and-recovery-routing).

The existing web app supports unified and independent-owner Create/Join without Tauri. Desktop bundled selection uses the same HTTPS setup client/shared worker; selected remote bases must never be replaced by bundled bases. Native retains protected code retrieval, trust/elevation, host observation, retry/reset/reference recovery and Device marker completion, not separate Create/Join/approval orchestration. `/setup/bootstrap`, the old `/lidp/setup/status` probe and native mutation commands are removed, with no wrappers.

Normal protected APIs always enforce their own authentication and exact authorization. Listener/status/approval/completion markers, sign-in and Residency acknowledgements are not readiness or resource authority. Current owner trust, protected signing capability and retained state must verify; no global authentication-enable switch or peer freshness lease exists.

### Admission and unified setup

A fresh unified HTTPS host exposes `GET /idp/setup/status` with `stage`, `state`, `ready` and `owners:["idp","device","storage"]`. Independent status has the actual singleton owner. Invalid admission reports 503 rather than fresh permission; on an independent worker this can be an empty response, not a recoverable ready state. `POST /independent/setup/admission` (unified `/idp/setup/admission`; neutral owner-only `/setup/owner/admission`) returns 204 for a valid code; that verifies capability only, not completion.

Retrieve the host-generated 256-bit code through **protected host/deployment management**. Send exactly one `Authorization: Setup <64 lowercase hex>` header to that code's exact validated target. Independent owners have separate codes even on one origin. Codes are not OAuth bearers; never expose them in status, URLs, logs, ordinary console output or browser persistence. Browser setup requires the configured exact HTTPS UI Origin; the bundled native host additionally admits its exact Tauri Origin. No cookies or credentialed CORS. Origin/TLS/loopback alone is not admission.

Unified `POST /idp/setup/create` and `/join` return **202 only after durable exact intent binding**. Accepted work belongs to the host and survives request/browser disconnect. Create takes nonce, administrator, Node and transient password; Join takes nonce, Node and the canonical pinned peer endpoint, authenticates issuer discovery before binding and does not seed a new administrator. Poll status; `creating`/`joining` includes retained owner reopen, not just initial work. Only verified ready status includes unified public login metadata.

`POST /idp/setup/intent` requires current Setup authorization: 200 returns the exact nonsecret retained Create/Join intent, 204 means none, malformed evidence returns 503 without repair. Recovery does not resume work. Recheck scope/status before adopting it; exact retry requires newly entered capability and any required Create password, never a replacement nonce/identity. Complete-but-unretired restart stays unfinished until exact authorized finalization. Retired codes deny setup recovery/new mutations; the unified completed Create retry path separately uses ordinary password authentication and returns 200.

`POST /idp/setup/cancel` requires current code and the exact active intent; 409 means no matching active operation, 204 means cancellation signaled, **not rollback or finished cleanup**. Cancellation/finalization serialize with one winner. Closing a page is not cancellation. Missing/corrupt material or conflicting purpose/peer/identity denies unchanged. Completed reopen is existing-only and works without the source; it never reseeds or silently repairs.

Optional CLI Create/Join remains an alternate entry point, not an app prerequisite. A neutral awaiting-choice host cannot be adopted by those CLI commands; they refuse rather than overwrite its prerequisites. See [operations](operations.md#automatic-unified-cli-bootstrap).

### Independent Create worker contract

Each owner exposes `POST /independent/setup/create`, `/independent/setup/intent`, `/independent/setup/cancel` on its existing verified HTTPS listener. Every mutation/recovery requires **that owner's own** Setup code and configured Origin when present. The app sends the same exact nonsecret tuple to all three owners:

- `purpose:"create"`, `operation_nonce` (32 lowercase hexadecimal characters), `administrator`, `node`;
- canonical distinct HTTPS `idp`, `device`, `storage` bases;
- `issuer` equal to the selected IdP base; `peer:null`.

Only IdP receives transient `password`. Each owner durably binds before returning 202; exact active retries return 202, conflicting tuples 409. Status states are `awaiting_choice`, `creating`, `joining`, `unfinished`, `ready`, `unavailable`; only ready sets `ready:true`. Recovery returns the retained tuple or 204, never credentials. Cancel returns 204 when signaled. Retired codes deny mutation/recovery; completed-but-unretired state needs exact authorized finalization.

`GET /independent/setup/independent/public` publishes operation-bound public registrations/facts, including IdP `facts.login_client`. The app correlates the operation, issuer and all target bases across every owner before navigating to sign-in: three unrelated ready flags are insufficient. Recovery tolerates genuinely unbound awaiting-choice owners and reads public evidence for already-ready/retired owners without sending their retired codes.

Worker-only `/independent/setup/independent/challenge` and `/independent/setup/independent/register` exchange consumer credentials directly over verified HTTPS, with redirects disabled and outstanding exact single-use capability checks. They return no secret/challenge to the caller and are included in the shared contract and both SDK families; their availability in JavaScript grants no browser authority. Public metadata/nonce equality is not registration authority. See [worker design](setup-workers.md).

Workers reuse owner-local handles for seed/serving, retain one actual Management Device/endpoint, and create no Storage resources or per-process Nodes/Devices. Fresh Create needs no peer/history/content exchange. Separate listeners/stores do not imply separate domain enrollment or copied endpoint keys.

### Independent Join worker contract

Each owner accepts `POST /independent/setup/join` under its own Setup code with:

```json
{
  "purpose": "join",
  "operation_nonce": "0123456789abcdef0123456789abcdef",
  "administrator": "",
  "node": "Joining Node",
  "idp": "https://target.example/idp",
  "device": "https://target.example/device",
  "storage": "https://target.example/storage",
  "issuer": "",
  "peer": "https://source.example/device#<canonical-pinned-endpoint-id>"
}
```

`peer` is the source's exact HTTPS **Device base**, prefix included, plus the independently verified canonical endpoint fingerprint; it is not unified's bare endpoint choice. Every target authenticates signed purpose/nonce-bound discovery before initial durable admission/202. Empty issuer resolves to the source Canonical Issuer; a conflicting supplied issuer denies. Password is rejected; administrator must be empty. No source password verifier/private signer is imported or replacement administrator seeded.

Code-authorized `POST /independent/setup/intent` has no body: 200 returns the exact tuple with authenticated nonempty issuer, 204 means none. Retry/cancel use that tuple. A retained retry with empty issuer reuses the retained issuer without rediscovery, permitting source-offline finalization. Purpose, nonce, names, bases, peer or nonempty issuer conflicts deny. Cancel returns 204 for signaling or 409 without a matching active operation. Retired codes return 401.

Status has exactly `stage`, `state`, `ready`, singleton `owners`, e.g. `{"stage":"installation","state":"joining","ready":false,"owners":["device"]}`. It contains no offer or login Client; IdP public login discovery uses `/independent/setup/independent/public` at `facts.login_client`. Poll/correlate all owners. Public history import is approval-bound, owner-allowlisted and distinct from resource-content sync. Completed stopped reopen uses retained state without source contact/reseeding.

The durable `initialized.json` fence and `worker.json` must be preserved, even before domain creation. Lost initialized worker evidence refuses without replacement IDs/provider writes. Callback provider/network work occurs outside the lifecycle gate, then revalidates cancellation, retirement and the exact active operation before progress writes. Normal OAuth/Bearer authentication is unchanged.

### Independent current readiness and retained schemas

Completed `GET /independent/setup/status` rechecks current owner evidence. Failure returns **503** with `{"stage":"installation","state":"unavailable","ready":false,"owners":["<owner>"]}`. A later successful observation can report ready again without rewriting setup, reseeding or restoring admission. This is not an authentication-mode switch: normal routers retain their exact OAuth/Bearer and owner/resource checks.

- IdP checks exact worker/provider material, current approved signer/private capability, active setup User, public login Client and current confidential registration/root metadata locally.
- Management checks exact worker/consumer/endpoint material, current approved Device binding, current owner/policy agreement, administrator allowance and pending-cleanup fence. Completed readiness follows the current Device owner, not an immutable original-owner grant.
- Storage checks retained worker/registration receipts, protected service material and catalog roots, plus current Management policy for its configured endpoint. This needs the Node's configured Management/IdP services, not the remote Installation source. Existing-only reconstruction does not perform peer discovery/import or a full token/health qualification suite.

The native-root HTTPS listener probe sends **OPTIONS** to the existing status path and accepts 200 (CORS) or 405 (method router); it never invokes readiness-bearing GET or interprets readiness JSON. Native-root TLS, no redirects/proxy and existing connect/request bounds remain required. Slow/unavailable configured owner services can make GET time out or return concrete `503 unavailable` without shutting down the listener; the same running host can recover current readiness without reseeding. Public operation facts and a successful listener probe never override current status or grant API authority. See [receipt 42](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md).

**Retained worker schemas changed without migration:** IdP and Management `worker.json` are version **2**; Storage remains version **1**. IdP `credential` exists only for Create and is absent on Join. Management Create alone retains `create:{role,permissions}`; Join omits it and old top-level `role`/`permissions` are removed. Ordinary opens refuse older/contradictory state unchanged. Explicit stopped reset alone supports exact validated historical version-1 references and their attempted protected slots; it never converts them for serving or guesses missing inventories. Preserve `initialized.json`, admission, receipts and attempt fences. See [independent reset](operations.md#stopped-independent-owner-reset).

### Public operator-login Client

Create/Join retain a public Client with token endpoint authentication `none`, password/refresh/token-exchange grants and `openid profile email` login scopes. Unified ready status supplies it; independent IdP public facts supply `facts.login_client` after correlated all-owner completion; CLI prints its actual ID/scopes/audiences. No manual registration or confidential secret is needed.

Use ordinary `POST /idp/oauth2/token` form fields `grant_type=password`, actual `client_id`, existing User `username`, securely handled `password`, allowed `scope`. Token exchange selects the permitted audience/action before owner calls; the Client audience allowlist is not a User permission grant. Independent Join does not copy source password credentials: source-password login at the target is not established by public import. See [login operations](operations.md#public-operator-login).

### Production Node-join approval routes

Append these owner-relative paths to the **selected source Device base**. Browser callers select the tagged module from checked IdP setup-scope evidence, not a URL suffix (independent owners may also mount at `/management`). Unified mounts them at `/device/node-joins/...`; independent sources mount the corresponding distinct operations at `/independent/node-joins/...` relative to their configured Device base. The `node_join` and `independent_join` modules use distinct operation IDs and response contracts; obsolete IdP paths are absent.

| Method/path                     | Contract                                                                                                              |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `GET /node-joins/acceptance`    | `{"accepting":true/false}`                                                                                            |
| `PUT /node-joins/acceptance`    | Unified returns state; independent `PUT /independent/node-joins/acceptance` returns 204 (read back)                   |
| `GET /node-joins/pending`       | Array of `offer_id`, `authenticated_endpoint`, exact public `request`                                                 |
| `POST /node-joins/{id}/approve` | Unified 200 `NodeJoinApproval`; `POST /independent/node-joins/{id}/approve` returns 202 accepted work, not completion |
| `POST /node-joins/{id}/reject`  | 204                                                                                                                   |

Every route requires an ordinary authorized User bearer with **Management audience** (`idp-management`) and both exact Device pairing-update and separate signer-enrollment permissions. IdP-audience tokens or target Setup codes do not authorize source approval. Unified missing bearer is 401, permission denial 403, missing offer 404, conflict 409, other owner failure 503; independent missing/unauthorized User evidence denies with 403.

Approval binds distinct Device, endpoint, owner, signer member/key and Installation identities through their respective owners. It preserves existing restrictions/selections, initializes only absent default policy, refreshes admission and rechecks current authorization/enrollment before signed reply. Failure after enrollment stays partial for exact retry; rejected enrollment creates no policy. Discovery/acceptance/pairing is not membership or resource permission. All platforms use these HTTPS routes, not native source mutation commands.

## Generated clients

Rust route/schema definitions remain authoritative. `openapi/*.json` are deterministic derived contracts shared by both languages; never hand-edit contracts or generated output. See the [route and consumer inventory](openapi-inventory.md) for topology boundaries and the reconciled consumer audit.

```sh
pnpm install --frozen-lockfile --ignore-scripts
cargo run -p unified-server --example export-openapi -- openapi
pnpm --filter @aicacia/idp-client run build:esm
pnpm openapi
pnpm openapi:check
pnpm --filter @aicacia/idp-client --filter @aicacia/management-client --filter @aicacia/storage-client run build:esm
cargo check -p of-idp-client -p of-management-client -p of-storage-client
node --test scripts/generate-openapi-client.test.mjs scripts/generated-*.test.mjs
cargo test -p unified-server generated_
```

Export needs no listener, Installation, credentials or peer. Generation reads only committed local documents; `OPENAPI_SPEC_FILE` is an explicit local-file override for validation/failure tests. Java and OpenAPI Generator 7.25.0 (wrapper 2.41.0) are required. Initial tool/dependency acquisition may need network access; cached generation is offline. The generator stages and validates both languages, then transactionally installs the staged artifacts and checks the generated Rust crates as root workspace members with the root `Cargo.lock` (`cargo fetch --locked`, then `cargo check --offline --locked`). Failed validation/generation/format/build rolls back both SDK families. No separate workspace or lockfile is created. Run generation serially.

### Developer and CI gate

Run `pnpm openapi:check` from the repository root to validate the current working tree without requiring a commit. CI runs `pnpm openapi:check --committed` in its independent **OpenAPI contracts and SDKs** job (45-minute limit, read-only repository permission, no deployment credentials).

The check exports all three owner documents twice into temporary directories and compares bytes with each other and `openapi/`; builds SDK prerequisites for a clean checkout; regenerates all three TypeScript/Rust SDK pairs twice and compares complete artifact trees; optionally (`--committed`) rejects tracked, staged or untracked artifact drift against HEAD; rebuilds ESM/declarations; loads every package export; runs the existing generation-failure and SDK tests; compiles real owner/Unified Rust consumers; and runs production owner registration, independent handoff, fresh-host publication, Setup security, generated transport/actual-owner and host-composed Node-join tests. A failure exits nonzero. Update Rust definitions/templates, export, regenerate, review both contract and SDK changes, then rerun the gate. Do not fix drift by hand-editing generated artifacts or updating only one language.

Prerequisites: Node from `.nvmrc` (26), pnpm 12.4.1, stable Rust with rustfmt, Java 21, and the pinned generator above. Install dependencies with the frozen lockfile; use `--offline` on pnpm install and `CARGO_NET_OFFLINE=true` after dependencies and the generator JAR are cached. No running deployment, provisioned Installation or peer is needed. Controlled loopback listeners are used by integration tests; this is not a network-isolation test. Initial package/crate/JAR acquisition needs network access. Each qualification subprocess is bounded to ten minutes; generator stages have three-minute bounds. Do not run generation/check commands concurrently.

When adding a route, register/document it through the existing owner/host router seam, retain actual-router SDK tests and update the inventory with any new outbound consumer. Every project-owned operation, including service-only handoffs, must use generated operations/builders. Direct current-state in-process owner calls, arbitrary external-provider/native fetch transport, framework HEAD/OPTIONS and non-HTTP message protocols are separate boundaries, not fallback HTTP implementations. Completeness tests derive owner operations from production registration; the Markdown inventory is not the sole oracle. Contract checks do not prove every authorization scenario: retain local/offline, exact Setup/Join authorization and transport tests. No full native GUI, cross-platform or production acceptance is implied.

Rust crates `of-idp-client`, `of-management-client`, `of-device-client`, and `of-storage-client` live under `crates/{owner}-client` as normal root workspace members and have no server dependencies. Their configurations accept existing reqwest 0.12 clients and request/response byte bounds, preserving transport-policy injection without handwritten operations. Default clients use native-root verified HTTPS, no redirect or proxy, 5-second connect/15-second request deadlines, and 1 MiB bounds. Response limits apply incrementally, before unbounded buffering, to successes and errors. Migrated consumers retain each caller's stricter limits, cached credentials and issuer/audience/type/scope/expiry/domain checks. Decoding does not authorize a call.

Exactly four canonical documents and SDK products exist: IdP, Management, Device, and Storage. Live owner `/openapi.json` discovery serves the corresponding checked-in canonical artifact, including on fresh unified and independent hosts; only the server URL is deployment-specific. Rust registrations remain authoritative for export, and the shared gate compares exports with these served artifacts. There is no separate `/idp/setup/openapi.json` endpoint. TypeScript uses the existing owner package roots (Storage also retains its generated-module export). Setup, owner-local admission, independent setup, independent Join/history and host-coordinated Node Join are tagged modules in those owner SDKs, not subpath products. All generated operations take complete verified **owner bases**, including the configured mount prefix exactly once. Host setup stays at `/idp/setup/...` and is documented as host-coordinated setup in the IdP contract; Node Join stays in Device. Independent routes use `/independent/setup/...` and `/independent/node-joins/...`; neutral owner admission/status uses `/setup/owner/...`. Old SDK products and route aliases are removed. Stable `https://localhost/{owner}` defaults are placeholders, not discovered identities.

Independent challenge/register/history handoffs are included, not hidden browser exclusions. Existing active-operation, Setup, recipient, challenge, bearer and permission checks are unchanged. WebSocket upgrade contracts describe only HTTP handshakes, not subsequent Storage/history messages or Iroh/SQL/filesystem streams. Built JavaScript exports and generated Rust operations are tested against actual owner/Setup listeners; transport tests cover TLS, redirects, deadlines and streaming bounds. These are not native GUI or production acceptance claims.

## Storage client consumption

Use explicit `idpBaseUrl` and `storageBaseUrl`, not legacy `baseUrl`/`audience` or origin fallback. Password-manager requires complete `PUBLIC_LIDP_BASE_URL` and `PUBLIC_STORAGE_BASE_URL`; `PUBLIC_STORAGE_AUDIENCE` is no longer consumed. Filesystem token exchange uses `resource=storage:filesystems`; catalog/socket paths append once.

Storage exports resolve through `esm`/`types`. Build explicitly before publication (no automatic `prepack`) and distribute complete compiled generated dependencies. Workspace source tests alone do not prove package exports. The shared check builds ESM/declarations, loads every public JavaScript export, and exercises actual owner listeners; historical export failures are superseded by ticket 02 qualification.

## Source anchors

- Shared setup/admission/independent contracts: `crates/api/src/`, including `independent_openapi.rs`.
- IdP and Management owner routers: `crates/idp-server/src/router/`, `crates/management-server/src/router/`.
- Unified setup worker/runtime: `crates/unified-server/src/setup_runtime.rs`, `local_provisioning.rs`, `local_join.rs`, `runtime.rs`. Source approval belongs to `crates/device-server/src/node_join_source.rs`.
- Native lifecycle boundary: [NATIVE-SETUP.md](../apps/app/src-tauri/NATIVE-SETUP.md).
- Frontend setup transport: `apps/app/src/lib/common/state/setupApi.ts`, `independentSetupApi.ts`.
