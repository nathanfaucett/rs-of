# Node API contracts

Current owner/database boundaries and Device routes: [shared control-plane databases](control-plane.md).

Current lifecycle qualification and exact limits are recorded in
[ticket03](../.scratch/unified-owner-lifecycle/ticket03-validation.md) and
[ticket04](../.scratch/unified-owner-lifecycle/ticket04-validation.md). Historical
receipts below retain their original scope, not current GUI/platform certification.

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
| Storage    | Database/filesystem CRUD, explicit keyed database ensure, atomic SQL batches and filesystem Residency                                   | User/Application namespace, kind audience, exact read/write action and current owner/policy/cleanup gates                                                    |

`/replica-signers` retains its historical spelling; it is not a privileged replica role. Endpoint possession proves transport identity, not permission. An approved endpoint needs no individual IdP token signer or full Node enrollment. Unknown/revoked endpoints deny even with valid OAuth evidence; token issuer/Principal/signer checks remain independently mandatory. Approval alone grants no resource access. See [Storage API](../crates/storage-service/docs/api.md).

`GET /replication/devices/{endpoint_id}/selections` requires a Client token with the configured Storage audience and `management.replication.read`. It returns `deviceId`, `endpointId`, `ownerSubject`, `adminAllowed`, `resources`, and optional `pendingCleanup:{cleanupId,deviceId,endpointId,oldOwnerSubject,newOwnerSubject}`. Missing trust/policy or an inactive/revoked/conflicted binding denies. Pending cleanup returns no resources and `adminAllowed:false`; the concrete retirement evidence is exposed only to that authorized service caller, never as permission to use resources. Ordinary restriction likewise grants nothing.

Independent Storage retires both public SQL/KV and filesystem owner copies before sending `POST /replication/owner-cleanup` through its existing Storage→Device service Client. The request is exactly `{cleanupId, deviceId, endpointId, oldOwnerSubject, newOwnerSubject}`; acknowledgement requires the same configured audience, Client principal and `management.replication.admit`. Device compares the durable pending cleanup ID, retained old policy owner and current approved/non-revoked Device/endpoint/new-owner binding inside one local transaction. Changing the owner or endpoint while cleanup is delayed denies; replaying an acknowledgement from an earlier occurrence of the same owner tuple cannot clear a later fence. A lost successful response supports an idempotent no-op retry when no cleanup remains pending.

Cleanup rebinds an empty **restricted** policy, never administrator allowance, selections or old content. Only the configured local Device endpoint can drive this Storage runtime's cleanup; remote-peer policy reads remain read-only. Interrupted cleanup reuses existing Storage retirement evidence and the same Device pending fact/ID after stopped reopen. Local persistence failure, service denial and shutdown deny without acknowledging unfinished retirement. The pending table now includes `cleanup_id`; an older/malformed retained layout is not silently migrated or repaired, and its fence must not be deleted to manufacture freshness.

Independent Storage uses fresh authenticated Device responses for operation/frame/response checks; local denial never falls back remotely. Only service tokens are cached. Cancellation and local-copy retirement are rechecked after asynchronous evaluation before queued frames are released. [Receipt 42](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md) qualifies actual independent HTTPS transfer and original-owner/replay checks; [receipt 41](../.scratch/ui-driven-node-bootstrap/evidence/41-management-cleanup-client.md) qualifies the generated cleanup SDK. These are recorded results, not tests rerun by this documentation. The acknowledgement route returns 204 on success, 401 for invalid bearer, 403 for stale binding/insufficient service permission, or 503 for unavailable owner evidence.

## Prefixes and application identity

Use verified complete service bases and append owner-relative paths once. Do not add `/idp/idp`, `/management/management` or `/storage/storage` compatibility routes. Canonical Issuer, public Application URI, OAuth audience, HTTPS address, endpoint ID, Device ID, Principal and token signer are distinct.

An independent IdP can use `management_permissions` with verified HTTPS and its separately registered evaluator Client. It validates the exact echoed decision/audit identity; transport/authentication/policy failure denies without database fallback. Composed owner workers use the same configured identity/Device interfaces and operation-bound consumer relationships; topology does not waive authorization. `idp-management` is the Management Application audience, not its transport prefix. See [service relationships](operations.md#shared-control-plane-databases).

## Setup, join and readiness

Both layouts use the owner-local contract below. There is no Unified setup endpoint,
aggregate readiness document, or `SetupApi` SDK group. The app collects the four
context bases and three owner codes once, submits one exact operation separately
to IdP, Device, and Storage, and sends the password only to IdP. Management is not
a setup owner and requires no additional ceremony. Native provides verified local
origin/failure observation, owner-scoped protected code retrieval, trust consent,
shutdown, and stopped-owner reset; it does not proxy Create/Join or source approval.

### Owner-local Create worker contract

The owner-relative setup surface is `/setup/owner/...`: a standalone owner appends it to its origin, and a composed owner appends it to its configured IdP, Device, or Storage prefix exactly once. The generated module is `owner_setup` (`OwnerSetupApi` in TypeScript). No previous topology-named setup routes remain.

Each owner exposes `POST /setup/owner/create`, `/setup/owner/intent`, `/setup/owner/cancel` on its existing verified HTTPS listener. Every mutation/recovery requires **that owner's own** Setup code and configured Origin when present. The app sends the same exact nonsecret tuple to all three owners:

- `purpose:"create"`, `operation_nonce` (32 lowercase hexadecimal characters), `administrator`, `node`;
- canonical distinct HTTPS `idp`, `device`, `storage` bases;
- `issuer` equal to the selected IdP base; `peer:null`.

Only IdP receives transient `password`. Each owner durably binds before returning 202; exact active retries return 202, conflicting tuples 409. Status states are `awaiting_choice`, `creating`, `joining`, `unfinished`, `ready`, `unavailable`; only ready sets `ready:true`. Recovery returns the retained tuple or 204, never credentials. Cancel returns 204 when signaled. Retired codes deny mutation/recovery; completed-but-unretired state needs exact authorized finalization.

`GET /setup/owner/public` publishes operation-bound public registrations/facts, including IdP `facts.login_client`. The app correlates the operation, issuer and all target bases across every owner before navigating to sign-in: three unrelated ready flags are insufficient. Recovery tolerates genuinely unbound awaiting-choice owners and reads public evidence for already-ready/retired owners without sending their retired codes.

Worker-only `/setup/owner/challenge` and `/setup/owner/register` exchange consumer credentials directly over verified HTTPS, with redirects disabled and outstanding exact single-use capability checks. They return no secret/challenge to the caller and are included in the shared contract and both SDK families; their availability in JavaScript grants no browser authority. Public metadata/nonce equality is not registration authority. See [worker design](setup-workers.md).

Workers reuse owner-local handles for seed/serving, retain one actual Device/endpoint, and create no Storage resources or per-process Nodes/Devices. Fresh Create needs no peer/history/content exchange. Separate listeners/stores do not imply separate domain enrollment or copied endpoint keys.

### Owner-local Join worker contract

Each owner accepts `POST /setup/owner/join` under its own Setup code with:

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

`peer` is the source's exact HTTPS **Device base**, prefix included, plus the independently verified canonical endpoint fingerprint; a bare endpoint choice is not accepted. Every target authenticates signed purpose/nonce-bound discovery before initial durable admission/202. Empty issuer resolves to the source Canonical Issuer; a conflicting supplied issuer denies. Password is rejected; administrator must be empty. No source password verifier/private signer is imported or replacement administrator seeded.

Code-authorized `POST /setup/owner/intent` has no body: 200 returns the exact tuple with authenticated nonempty issuer, 204 means none. Retry/cancel use that tuple. A retained retry with empty issuer reuses the retained issuer without rediscovery, permitting source-offline finalization. Purpose, nonce, names, bases, peer or nonempty issuer conflicts deny. Cancel returns 204 for signaling or 409 without a matching active operation. Retired codes return 401.

Status has exactly `stage`, `state`, `ready`, singleton `owners`, e.g. `{"stage":"installation","state":"joining","ready":false,"owners":["device"]}`. It contains no offer or login Client; IdP public login discovery uses `/setup/owner/public` at `facts.login_client`. Poll/correlate all owners. Public history import is approval-bound, owner-allowlisted and distinct from resource-content sync. Completed stopped reopen uses retained state without source contact/reseeding.

The durable `initialized.json` fence and `worker.json` must be preserved, even before domain creation. Lost initialized worker evidence refuses without replacement IDs/provider writes. Callback provider/network work occurs outside the lifecycle gate, then revalidates cancellation, retirement and the exact active operation before progress writes. Normal OAuth/Bearer authentication is unchanged.

### Owner-local current readiness and retained schemas

Completed `GET /setup/owner/status` rechecks current owner evidence. Failure returns **503** with `{"stage":"installation","state":"unavailable","ready":false,"owners":["<owner>"]}`. A later successful observation can report ready again without rewriting setup, reseeding or restoring admission. This is not an authentication-mode switch: normal routers retain their exact OAuth/Bearer and owner/resource checks.

- IdP checks exact worker/provider material, current approved signer/private capability, active setup User, public login Client and current confidential registration/root metadata locally.
- Device checks exact worker/consumer/endpoint material, current approved Device binding, current owner/policy agreement, administrator allowance and pending-cleanup fence. Completed readiness follows the current Device owner, not an immutable original-owner grant.
- Storage checks retained worker/registration receipts, protected service material and catalog roots, plus current Management policy for its configured endpoint. This needs the Node's configured Management/IdP services, not the remote Installation source. Existing-only reconstruction does not perform peer discovery/import or a full token/health qualification suite.

The native-root HTTPS listener probe sends **OPTIONS** to the existing status path and accepts 200 (CORS) or 405 (method router); it never invokes readiness-bearing GET or interprets readiness JSON. Native-root TLS, no redirects/proxy and existing connect/request bounds remain required. Slow/unavailable configured owner services can make GET time out or return concrete `503 unavailable` without shutting down the listener; the same running host can recover current readiness without reseeding. Public operation facts and a successful listener probe never override current status or grant API authority. See [receipt 42](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md).

**Retained worker schemas changed without migration:** IdP and Device `worker.json` are version **2**; Storage remains version **1**. IdP `credential` exists only for Create and is absent on Join. Device Create alone retains `create:{role,permissions}`; Join omits it and old top-level `role`/`permissions` are removed. Ordinary opens refuse older/contradictory state unchanged. Explicit stopped reset alone supports exact validated historical version-1 references and their attempted protected slots; it never converts them for serving or guesses missing inventories. Preserve `initialized.json`, admission, receipts and attempt fences. See [independent reset](operations.md#standalone-stopped-owner-reset).

### Public operator-login Client

Create/Join retain a public Client with token endpoint authentication `none`, password/refresh/token-exchange grants and `openid profile email` login scopes. IdP public facts supply `facts.login_client` after correlated all-owner completion in both layouts; CLI admission does not replace this readiness/login discovery. No manual registration or confidential secret is needed.

Use ordinary `POST /idp/oauth2/token` form fields `grant_type=password`, actual `client_id`, existing User `username`, securely handled `password`, allowed `scope`. Token exchange selects the permitted audience/action before owner calls; the Client audience allowlist is not a User permission grant. Independent Join does not copy source password credentials: source-password login at the target is not established by public import. See [login operations](operations.md#source-approval-and-ordinary-use).

### Production Node-join approval routes

Append `/independent/node-joins/...` to the selected source Device base in both
layouts. `IndependentJoinApi` uses distinct operation IDs from the separate domain
Node-join API. The app no longer chooses a legacy contract by deployment shape.

| Operation | Result |
| --- | --- |
| GET/PUT `/independent/node-joins/acceptance` | GET state; PUT 204, then read back |
| GET `/independent/node-joins/pending` | Exact pending offers |
| POST `/independent/node-joins/{id}/approve` | 202 accepted work, not target readiness |
| POST `/independent/node-joins/{id}/reject` | 204 acknowledgement |

Every route requires ordinary authorized User evidence with Management audience
`idp-management`, Device pairing-update and separate signer-enrollment permissions.
Target Setup codes and IdP-audience tokens cannot approve source work. Verify exact
operation, endpoint, member, Device and public signer before approval. Completed
source-offline reopen uses retained local evidence without reseeding.

## Generated clients

Rust route/schema definitions remain authoritative. `openapi/*.json` are deterministic derived contracts shared by both languages; never hand-edit contracts or generated output. See the [route and consumer inventory](openapi-inventory.md) for topology boundaries and the reconciled consumer audit.

```sh
pnpm install --frozen-lockfile --ignore-scripts
cargo run -p unified-server --example export-openapi -- openapi
pnpm --filter @aicacia/idp-client run build:esm
pnpm openapi
pnpm openapi:check
pnpm sdk:build
cargo check -p of-idp-client -p of-management-client -p of-device-client -p of-storage-client
node --test scripts/generate-openapi-client.test.mjs scripts/generated-*.test.mjs
cargo test -p unified-server --test composition
```

Export needs no listener, Installation, credentials or peer. Generation reads only committed local documents; `OPENAPI_SPEC_FILE` is an explicit local-file override for validation/failure tests. Java and OpenAPI Generator 7.25.0 (wrapper 2.41.0) are required. Initial tool/dependency acquisition may need network access; cached generation is offline. The generator stages and validates both languages, then transactionally installs the staged artifacts and checks the generated Rust crates as root workspace members with the root `Cargo.lock` (`cargo fetch`, then `cargo check`). Failed validation/generation/format/build rolls back both SDK families. No separate workspace or lockfile is created. Run generation serially.

### Developer and CI gate

Run `pnpm openapi:check` from the repository root to validate the current working tree without requiring a commit. CI runs `pnpm openapi:check --committed` in its independent **OpenAPI contracts and SDKs** job (45-minute limit, read-only repository permission, no deployment credentials).

The check exports all four owner documents twice into temporary directories and compares bytes with each other and `openapi/`; builds SDK prerequisites for a clean checkout; regenerates all four TypeScript/Rust SDK pairs twice and compares complete artifact trees; optionally (`--committed`) rejects tracked, staged or untracked artifact drift against HEAD; rebuilds ESM/declarations; loads every package export; runs the existing generation-failure and SDK tests; compiles real owner/Unified Rust consumers; and runs production owner registration, independent handoff, fresh-host publication, Setup security, generated transport/actual-owner and host-composed Node-join tests. A failure exits nonzero. Update Rust definitions/templates, export, regenerate, review both contract and SDK changes, then rerun the gate. Do not fix drift by hand-editing generated artifacts or updating only one language.

Prerequisites: Node from `.nvmrc` (26), pnpm 12.4.1, stable Rust with rustfmt, Java 21, and the pinned generator above. Install dependencies with the frozen lockfile; use `--offline` on pnpm install and `CARGO_NET_OFFLINE=true` after dependencies and the generator JAR are cached. No running deployment, provisioned Installation or peer is needed. Controlled loopback listeners are used by integration tests; this is not a network-isolation test. Initial package/crate/JAR acquisition needs network access. Each qualification subprocess is bounded to ten minutes; generator stages have three-minute bounds. Do not run generation/check commands concurrently.

When adding a route, register/document it through the existing owner/host router seam, retain actual-router SDK tests and update the inventory with any new outbound consumer. Every project-owned operation, including service-only handoffs, must use generated operations/builders. Direct current-state in-process owner calls, arbitrary external-provider/native fetch transport, framework HEAD/OPTIONS and non-HTTP message protocols are separate boundaries, not fallback HTTP implementations. Completeness tests derive owner operations from production registration; the Markdown inventory is not the sole oracle. Contract checks do not prove every authorization scenario: retain local/offline, exact Setup/Join authorization and transport tests. No full native GUI, cross-platform or production acceptance is implied.

Rust crates `of-idp-client`, `of-management-client`, `of-device-client`, and `of-storage-client` live under `crates/{owner}-client` as normal root workspace members and have no server dependencies. Their configurations accept existing reqwest 0.12 clients and request/response byte bounds, preserving transport-policy injection without handwritten operations. Default clients use native-root verified HTTPS, no redirect or proxy, 5-second connect/15-second request deadlines, and 1 MiB bounds. Response limits apply incrementally, before unbounded buffering, to successes and errors. Migrated consumers retain each caller's stricter limits, cached credentials and issuer/audience/type/scope/expiry/domain checks. Decoding does not authorize a call.

Exactly four canonical documents and SDK products exist: IdP, Management, Device, and Storage. Live owner `/openapi.json` discovery serves the corresponding checked-in canonical artifact, including on fresh unified and independent hosts; only the server URL is deployment-specific. Rust registrations remain authoritative for export, and the shared gate compares exports with these served artifacts. There is no separate `/idp/setup/openapi.json` endpoint. TypeScript uses the existing owner package roots (Storage also retains its generated-module export). Owner setup, independent Join/history and domain Node Join are tagged modules in those owner SDKs, not subpath products. All generated operations take complete verified **owner bases**, including the configured mount prefix exactly once. Setup belongs to IdP, Device, and Storage independently; Node Join stays in Device. Owner setup uses `/setup/owner/...` in the `owner_setup` module; independent source Join/history keeps `/independent/node-joins/...`. Old SDK products and route aliases are removed. Stable `https://localhost/{owner}` defaults are placeholders, not discovered identities.

Independent challenge/register/history handoffs are included, not hidden browser exclusions. Existing active-operation, Setup, recipient, challenge, bearer and permission checks are unchanged. WebSocket upgrade contracts describe only HTTP handshakes, not subsequent Storage/history messages or Iroh/SQL/filesystem streams. Built JavaScript exports and generated Rust operations are tested against actual owner/Setup listeners; transport tests cover TLS, redirects, deadlines and streaming bounds. These are not native GUI or production acceptance claims.

## Storage client consumption

Use explicit `idpBaseUrl` and `storageBaseUrl`, not legacy `baseUrl`/`audience` or origin fallback. Password-manager requires complete `PUBLIC_LIDP_BASE_URL` and `PUBLIC_STORAGE_BASE_URL`; `PUBLIC_STORAGE_AUDIENCE` is no longer consumed. Filesystem token exchange uses `resource=storage:filesystems`; catalog/socket paths append once.

Storage exports resolve through `esm`/`types`. Build explicitly before publication (no automatic `prepack`) and distribute complete compiled generated dependencies. Workspace source tests alone do not prove package exports. The shared check builds ESM/declarations, loads every public JavaScript export, and exercises actual owner listeners; historical export failures are superseded by ticket 02 qualification.

## Source anchors

- Shared setup/admission/independent contracts: `crates/api/src/`, including `independent_openapi.rs`.
- IdP and Management owner routers: `crates/idp-server/src/router/`, `crates/management-server/src/router/`.
- Unified setup worker/runtime: `crates/unified-server/src/setup_host.rs` and each owner server's `independent_create.rs`. Source approval belongs to `crates/device-server/src/node_join_source.rs`.
- Native lifecycle boundary: [NATIVE-SETUP.md](../apps/app/src-tauri/NATIVE-SETUP.md).
- Frontend setup transport: `apps/app/src/lib/common/state/setupApi.ts`, `independentSetupApi.ts`.


### Protected host opening and configured native UI origins

`api::SetupAdmission::open` returns `std::io::Result<SetupAdmission>` so a host can preserve operational error kinds (`WouldBlock`, `PermissionDenied`, and other I/O failures), distinguish missing retained files from malformed evidence, and offer a safe retry without replacing identity. Admission checks, protected code retrieval and HTTP responses retain their redacted `SetupAdmissionError` contract; no filesystem errors or codes are added to public status. Unified/independent host and reset consumers propagate the constructor's I/O errors directly.

Setup UI origin validation accepts exact HTTPS origins or the explicitly configured packaged-native origins `tauri://localhost` and `http://tauri.localhost`. Each router permits only its configured origin; this does not auto-allow both, accept `null`, or waive admission. Native UI origins do not relax HTTPS API target validation, per-owner setup-code scope or ordinary User authorization.
