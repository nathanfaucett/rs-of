# Node operations

These are current operational requirements for the [offline-first Node design](design.md), not a fully qualified deployment runbook. Ordinary CLI create/join is automatic apart from the human password, trusted peer choice, authorized approval and OS trust/elevation consent. Internal IDs, secrets, audiences, context roots and password files are not prerequisites.

## Qualification boundary

The approved qualification scope is reproducible bounded Ubuntu Linux x86_64 evidence. Multiple isolated Nodes on one host can qualify protocol/partition/reopen semantics without physical machines. Historical local qualification includes production authenticated join/completion/offline native reopen, session-provider setup/reset, completion-marker checks, four representative durable boundaries and focused DOM/client-SSR checks. The recorded durable regression took 350.97 seconds under a 360-second internal bound; earlier larger-fixture timeouts are not erased by that result.

**Verified GUI approval, profile-B completion and two-profile GUI offline restart remain deferred and not observed.** Offer display is not approval. Listener/status probes, receipts, markers and Residency acknowledgements never substitute for current trust/readiness or resource authorization. Historical lifecycle, rotation and retained-provider filesystem recovery results retain the narrower boundaries below; they are not current-tip reruns.

Windows/macOS, headless/container deployment, real WAN, lost-provider/crash recovery and exhaustive GUI/fault/feature matrices remain deferred qualification, not claimed support. Broad Cargo Hack/CRAP sweeps were outside that bounded machine-local qualification. Security, integrity, fail-closed startup and known local implementation defects remain in scope. This documentation update performs no code validation or native installation; supplied focused implementation results are recorded separately below, not independently rerun here. See [API contracts](api.md#setup-join-and-readiness) and [security boundaries](security.md).

Current focused validation reported by the coordinating parent/implementation agents: **12 new unit tests passed**; the final production HTTPS regression passed in **296.18 seconds** under its unchanged **300-second child bound** and narrow **5-second HTTP timeout**, covering no-config source/create, public login/password authentication, authorized acceptance/rejection, stale and malformed offer denial, policy preservation, real join, source-offline reopen, authenticated Storage and completed retry. Earlier baseline stack overflow failed; subsequent admission and rejected-enrollment timeout failures were fixed before that pass. Desktop localhost validation passed **17 tests in 5.33 seconds**, with three additional issuer/discovery tests passing; the deterministic reply/acceptance-invalidation race regression and library Clippy with and without `cli` also passed. Strict all-targets Clippy encountered pre-existing fixture diagnostics; the `ofnet` generation and pairing-reply-closure tests also passed. Frontend Vitest was blocked by the absent dependency; Svelte checking reported **five unrelated Storage client export errors**, not a frontend pass. The HTTPS harness uses mock keyrings, injected install commands and isolated native CA-file roots: **no real OS trust store was modified**. These focused results establish neither GUI acceptance nor universal platform/provider/deployment qualification.

## Fresh API startup — partial app setup integration

Ordinary `unified-server`/`serve` on a genuinely fresh store now starts a neutral HTTPS setup host rather than requiring CLI provisioning. It retains listener/host prerequisites without choosing an Installation issuer or establishing domain state. Mandatory OS CA installation and native-root verification remain in place. A separately deployed web app must use the correct API base and the explicit config's exact HTTPS `idp.ui_public_uri` as its allowed browser origin; no-config currently derives that origin from the local listener.

Startup prints the HTTPS base and a **protected management retrieval path**, not the setup code. Authorized host management can retrieve the 64-character code from the second line of that dedicated private state file; never expose the file publicly, weaken its permissions or put the code in logs/URLs. The app's web setup page submits it in an explicit authorization header without cookies or persistent browser storage. Code acceptance does not sign in, approve membership or complete setup.

Fresh independent owner CLIs now work without relationship credentials: new default roots/ports are `idp-data`:3000, `management-data`:3001 and `storage-data`:3002, with existing configured/legacy roots and listeners preserved. Each retains separate `<domain-root>.setup-host` metadata, TLS and code. Owner-relative setup paths use the actual configured prefix once. Optional `server.setup_ui_origin` is the exact HTTPS web-app origin; IdP otherwise uses `ui_public_uri`, while Management/Storage use the existing default app origin. Configure explicit `PUBLIC_LIDP_BASE_URL`, `PUBLIC_LIDP_MANAGEMENT_BASE_URL` and `PUBLIC_STORAGE_BASE_URL` for independent web routing; the app does not derive service bases or shared admission from an origin/path. Status `owners` distinguishes one unified target from separate singleton owners.

Startup installation is cancellation-owned, including termination/reaping of ordinary owned installer process groups; escaped/privileged descendants and real elevation behavior remain unqualified. Arbitrary verifier injection is available only under nondefault `api/test-support`, enabled through tests, not production dependencies.

This implementation is partial: unified web Create controls are wired, but the complete Create/Join feature is not delivered. Unified Create now returns promptly after durable authorized admission and runs as a host-owned operation; observe completion through status instead of keeping the Create request open. Closing the submitting client does not cancel setup; cancellation requires an explicit code-authorized request. Its focused HTTPS completion/authentication/retry/retirement/offline-reopen regression passes. The web app accepts protected administrator/Node input and the host code, submits unified Create, and offers status observation, exact in-session retry and explicit cancellation. It discovers the verified public login Client for ordinary sign-in. Codes/passwords are never persisted in browser storage. After reloading during incomplete unified Create, enter the host code and choose **Recover retained Create**. Authorized recovery retrieves only the exact nonsecret intent and checks current status; it does not resume work by itself. Re-enter the code and original password for an exact retry, or the code for cancellation of active work. No new nonce is substituted. Complete owner provisioning with unretired admission stays unfinished after restart and can finalize via the same authorized retry. Missing/corrupt state still refuses rather than being regenerated. [Recovery qualification](../.scratch/ui-driven-node-bootstrap/evidence/11-create-recovery-finalization.md) covers early cancellation/restart and a representative stopped completion-before-retirement checkpoint. The implemented unified setup contract is now generated separately through the IdP client owner workflow and consumed by the app's checked setup helper; [generation qualification](../.scratch/ui-driven-node-bootstrap/evidence/12-setup-generation.md) covers generation, package build and focused consumption. The app also exposes unified Join using the pinned source endpoint, transient target setup code and existing source User-authorized approval routes. Protected recovery restores the exact Join intent, including its authenticated issuer, for retry/cancellation. This flow is not accepted: the latest bounded opt-level-0 production test gets past the former stack overflow but still times out before readiness after source approval; see [current blocker](../.scratch/ui-driven-node-bootstrap/evidence/13-unified-api-join.md). Independent Create and Join workers now support the API-only lifecycle described below; app mutation wiring, generated independent setup clients and native-path reconciliation remain separate work. Do not advertise complete web setup. The optional independently configured IdP permission client now uses Management's production evaluator over verified HTTPS, and independent Create now establishes its protected service relationship; independently configured deployments still need valid retained credentials. The earlier [focused qualification](../.scratch/ui-driven-node-bootstrap/evidence/14-independent-permission-evaluation.md) is not independent bootstrap acceptance. The API/helper/render checks are not a rendered browser or real OS trust/provider certification. See [backend evidence](../.scratch/ui-driven-node-bootstrap/evidence/08-host-owned-create.md) and [app/OpenAPI evidence](../.scratch/ui-driven-node-bootstrap/evidence/09-unified-web-create.md). A neutral host's retained TLS/admission cannot yet be adopted by CLI Create/Join; those conflicting commands refuse. Preserve the state rather than deleting/resetting it to bypass the gap. Missing/corrupt admission/TLS is not automatically recreated; explicit recovery remains required. The existing completed-Node CLI flows below still apply to their original fresh/matching retained intents, not a neutral awaiting-choice host.

Focused evidence lives in the [initial receipt](../.scratch/ui-driven-node-bootstrap/evidence/01-setup-admission.md), [independent-owner startup and cancellation receipt](../.scratch/ui-driven-node-bootstrap/evidence/02-independent-startup.md), and [37 passing client/loader/page tests plus 13 helper tests](../.scratch/ui-driven-node-bootstrap/evidence/03-web-owner-admission.md). Injected trust tests are not actual OS installation; no real CA store/provider was modified for this slice.

### Independent Create and Join workers

The three fresh owner CLIs dispatch to concrete Create workers without a second serving command. Submit the same [exact operation](api.md#independent-create-worker-contract) to each intended owner with its own host code; send the administrator password only to IdP. The browser/operator never assembles relationship secrets. Admission returns 202 independently of provisioning; poll all owners rather than interpreting acceptance or one ready owner as global completion. Closing the client does not cancel work. Authorized recovery/cancellation/retry preserves the exact operation and generated identities. After stopped restart, complete-but-unretired state needs exact authorized finalization; missing/corrupt attempted material refuses without repair or regeneration.

Independent Join uses the same owner-local codes/lifecycle, with `purpose:"join"`, empty administrator/issuer, no password, and the source's exact HTTPS Management base plus `#<pinned-endpoint-id>` as `peer`. Each owner authenticates discovery and retains the source Canonical Issuer. Recover the exact nonsecret tuple separately from each owner before retry/cancellation. Enable acceptance and approve/reject the exact offer on source Management using an ordinary authorized User bearer, never a target Setup code. Join imports approved public identity/policy through the respective owners, not private passwords or resource content. Completed reopen and exact unretired finalization work with the source offline. See the [request/status contracts](api.md#independent-join-worker-contract).

The [Create receipt](../.scratch/ui-driven-node-bootstrap/evidence/independent-create-worker.md) and [Join/security receipt](../.scratch/ui-driven-node-bootstrap/evidence/independent-join-security.md) record isolated native-root HTTPS, authenticated Management/Storage use, interruption/rejection/exact retry, cancellation, stopped reopen and refusal on lost evidence/material. The initialization fence must remain with `worker.json`: a lost initialized record refuses before domain creation, without replacing Client IDs/provider material. Do not delete setup files to force fresh setup. Existing-app independent mutation wiring and generated clients remain separate integration work. Injected commands/mock providers do not qualify real OS installation, containers, WAN deployment or arbitrary crash checkpoints. No cross-owner store opens, copied endpoint private keys or per-process Device/Node enrollment are introduced.

### App Device and source-approval owner routing

Device types and list/rename/revoke operations belong to the Management client. Source acceptance, pending offers, approval and rejection use the configured Management base plus `/node-joins/...`, with its configured prefix applied once. Exchange the current User token at IdP for the `idp-management` audience, not the Management transport URL or an IdP-owner audience. Source approval remains ordinary User authorization, never target setup-code authorization. Native source commands consume the same Management-audience bearer.

The native reset peer-revocation consumer currently selects a composed peer by its exact HTTPS `/idp` base and verifies the retained Installation issuer before sending the User token. It revokes the signer at `/idp/replica-signers/{id}` using the IdP-owner audience, then separately exchanges for `idp-management` and revokes the Device at the same peer origin under `/management/devices/{id}`. Both successful responses are required before acknowledging the retained reset target. Partial success, redirects, unknown targets and lost acknowledgements retain the target for an authorized retry; signer revocation is not Device revocation. This composed-peer shortcut does not discover independent remote owner addresses.

The parent must regenerate the Management client from its real owner schema and rebuild its package exports before the moved Device client methods/types are usable. No compatibility calls to removed IdP Device routes are retained. This consumer reconciliation does not implement independent-owner Create/Join, relationship-secret delivery or native runtime construction. OS CA trust installation and verified HTTPS remain required; certificate-warning bypass is not a setup step.

## Automatic unified CLI bootstrap

With a built `unified-server` binary (Cargo package `unified-server`, feature `cli`):

```sh
unified-server create
unified-server serve
# On the joining machine, after enabling acceptance on the source:
unified-server join
unified-server serve
```

`create` defaults to administrator `operator` and Node name `Local Node`. It prompts for a hidden initial administrator password with confirmation. `--administrator` and `--node` are optional first-attempt overrides. Partial create retry asks for the same password; completed create re-verifies the retained Node without asking again. Retry the same `create` or `join` command with retained names/settings, not a new identity declaration. `provision` and `--fresh` are obsolete CLI syntax.

For noninteractive password delivery only, `create --password-file` accepts an operator-only (`0600` or stricter), nonsymlink regular file containing one nonempty UTF-8 line; a final LF/CRLF is removed. This is optional automation, not a bootstrap prerequisite. Never put passwords in argv, shell history, config, logs or raw-secret manifests. Noninteractive join can use `join --peer` with the verified endpoint fingerprint. There is no password-grant join.

Optional first-attempt username and automation examples (password-file input is never required for interactive setup):

```sh
# Choose the initial administrator username; enter its password at the hidden prompt:
unified-server create --administrator alice
# Alternatively, automate password delivery from an already protected file:
unified-server create --administrator alice --password-file ./protected-initial-password
```

These are alternative first attempts, not commands to run sequentially against the same Node. Continuation uses retained names; omit username/Node overrides on retry.

### Derived and retained settings

The default Node root is `$XDG_DATA_HOME/of/unified`, or `$HOME/.local/share/of/unified`. `--data-dir` optionally chooses another root. The host selects an unused localhost port once and durably retains public settings in `host.json`, then derives separate IdP/Management/Storage/endpoint roots, public Application URIs, audiences and a local key namespace. No manual root layout, internal UUIDs or service-secret environment variables are needed.

A new Installation's Canonical Issuer is `https://<retained-random-UUID>.installation.invalid/idp`, a unique retained logical identifier, **not** the local HTTP API address. The actual APIs use the saved local HTTPS listener. Separate Installations using the same localhost port still have distinct issuers. Join retains the authenticated peer's issuer while using its own API address. Restart/continuation preserves the exact identity/settings; port contention refuses rather than selecting a replacement. Missing/malformed host identity requires explicit recovery. Earlier address-based retained host intents are not silently migrated or renamed.

Bundled owners use current local owner calls and provision no confidential relationship Clients or relationship secrets. The public operator-login Client, independent Node signer and protected TLS remain. The historically named `CredentialBootstrap` and `.credentials/credentials.json` retain exact host settings and TLS production evidence only; they perform no service-secret provider operations. `host.json` contains public identity/settings, never passwords, private keys or tokens. Ordinary create/join does not load `.env` or require `UNIFIED_*_CLIENT_SECRET` variables.

### Public operator login

Both create and join automatically provision a separate **public** operator-login Client with a retained ID; no manual Client registration or Client secret is needed. Completed retry/reopen verifies and reuses that registration rather than generating a replacement. Create/join print `Operator login Client`, `Login scopes` and `Token-exchange audiences`; `serve` also prints the HTTPS API bases, issuer, IdP token endpoint and service audiences. Use the actual printed Client ID, not an invented UUID or one of the confidential service-client IDs.

Sign in through the ordinary `POST /idp/oauth2/token` route at the printed local HTTPS API address, with an `application/x-www-form-urlencoded` password-grant request:

| Field        | Value                                                                                                                   |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `grant_type` | `password`                                                                                                              |
| `client_id`  | Actual `Operator login Client` ID printed by the CLI                                                                    |
| `username`   | Retained administrator username, e.g. `operator` or the chosen `alice`; on a joined Node, an existing Installation User |
| `password`   | That User's password, supplied through secure hidden input/client handling, not argv/logs                               |
| `scope`      | `openid` (optionally `profile email`, within the printed login scopes)                                                  |

The public Client uses token endpoint authentication `none`: do not supply a Client secret or confidential-client Basic authentication. Exchange the resulting User token through the same ordinary token endpoint for the required printed IdP/Management audience, with exact action scopes; the Client's audience allowlist is not the User's permission grant. Use the resulting authorized bearer for source acceptance/approval. Password authentication signs in an existing User; it is not Node enrollment, and join neither invents a new administrator password nor bypasses User authorization.

### Trusted peer and authorized approval

On the completed source, run `serve`, sign in with the automatically retained public login Client (or another existing ordinary authorized client/session) and enable acceptance with the authorized `PUT /idp/node-joins/acceptance` route. `serve` prints the Node endpoint fingerprint and local HTTPS base. Transfer and verify the source fingerprint through a trusted channel; a discovered address alone is not trust evidence. Run `join` on the other machine and supply that fingerprint when prompted (or with `--peer`).

Before producing Node signer/service material, no-config join learns the source's Canonical Issuer over authenticated pairing. `of.equal-node-issuer-discovery.v1` binds purpose, fresh nonce and the pinned endpoint. The serving background task answers issuer probes while acceptance is enabled and current public Node state is ready, without pending-list polling or approval UI invocation. Discovery exchanges public identity, **not enrollment**. The peer/issuer are retained before Node production; unsigned HTTP/DNS metadata or an arbitrary discovered peer cannot substitute.

The production composed HTTPS router exposes:

| Route                               | Operation                                                                           |
| ----------------------------------- | ----------------------------------------------------------------------------------- |
| `GET /idp/node-joins/acceptance`    | Read acceptance state                                                               |
| `PUT /idp/node-joins/acceptance`    | Set acceptance with `{"accepting": true}` or `false`                                |
| `GET /idp/node-joins/pending`       | List authenticated offers, including `offer_id`, authenticated endpoint and request |
| `POST /idp/node-joins/{id}/approve` | Explicitly approve the exact pending offer                                          |
| `POST /idp/node-joins/{id}/reject`  | Explicitly reject the exact pending offer                                           |

All routes require an ordinary bearer session and IdP's existing `authorize_node_join_approver` checks; acceptance is not an unauthenticated toggle. Inspect the exact offered Installation, Device/member, endpoint and public signer before authorized approval. No operator repository calls, new API-key/password mechanism or automatic enrollment are required. An approval UI is optional client work, not a serving/API blocker; Desktop can use the corresponding runtime methods.

Approval validates and commits the exact IdP Device/member/signer binding and verifies enrollment/ownership through owner APIs. Only then does the host initialize an absent default empty Management policy, preserving existing restrictions/selections, refresh admission and ask IdP to recheck authorization/enrollment before publishing the signed reply. Rejected enrollment creates no Management policy. A Management/reply failure after enrollment is a partial operation for exact retry, not automatic rollback or identity replacement. The joining host does not manufacture source policy or receive general resource access.

The source must run the updated `UnifiedRuntime`. Older sources without the purpose-bound issuer response cannot support no-config join; update them, or use an explicit composition that already supplies the trusted issuer. Never replace authenticated evidence with unsigned discovery. Signed approval/bootstrap races use bounded handshake/admission acknowledgment retry with exact anchor, private binding and learned-grant checks; bulk public state transfer does not use that short budget. No general allowlist bypass is introduced.

### Mandatory TLS, OS trust and completion

**Both create and join, including continuation, install the retained localhost CA into OS trust.** Approve the required OS elevation/trust action. Failure to install or verify trust stops setup: there is no skip-install flag, plaintext, browser-only or insecure-certificate fallback. The CLI is Unix-only and accepts localhost TLS listeners only at `127.0.0.1` or `::1`; unsupported platforms/IPs refuse before material production.

Linux first tries `trust anchor`, then uses the matched platform anchor destination and refresh command: Debian-family `/usr/local/share/ca-certificates` with `update-ca-certificates`, or Fedora-family `/etc/pki/ca-trust/source/anchors` with `update-ca-trust extract`, via `pkexec`. macOS uses the login keychain. These code paths are not native installation qualification for every platform. The deployment must provide working secure-provider access, trust tools and elevation; a library or installed command alone does not prove that they work.

The secure API base is configured before runtime construction. CLI completion checks current local owner readiness and verifies the actual production HTTPS listener using native roots, without redirects or proxies. It does not obtain service tokens or run a health/service-relationship integration suite as part of provisioning; those remain explicit integration qualification. Owners and the verification listener are closed before durable completion is published; create/join then exit. Their optional CLI `serve` entry point reopens existing state; ordinary fresh API startup uses the neutral setup host described above. Neither path turns missing retained credentials/trust or an incomplete intent into fresh setup.

### Retention, retry and explicit configuration

Retain the whole Node root, including `host.json`, the context/endpoint roots and endpoint-adjacent `.credentials`, `.tls` and `.provisioning` directories, plus the secure provider's retained state. Manifests contain public metadata, references, one-way verifiers and completion phases, not raw secrets. Protected files/directories use Unix owner-only checks; stable operator-controlled nonsymlink ancestry is required. These checks are not hostile concurrent pathname-race protection.

Cooperating CLI operations hold retained locks through setup/serving. Preserve `operation.lock` and its inode; never unlink it, remove intent/provider references or delete context data to clear contention or manufacture freshness. Missing attempted credentials, endpoint/signing material, partial TLS, malformed intent or unacknowledged Management/Storage initialization require explicit recovery rather than replacement. Bundled completed startup checks current local identity, public login registration and grants; a historical completion marker is not an authorization lease. It has no confidential relationship credentials to load or rotate. Independent HTTPS consumers retain their scoped Client credentials and owner checks. Missing/revoked registrations are never recreated by restart.

`--config config/unified/example.yaml` is an **optional explicit composition**, mutually exclusive with `--data-dir`. Bundled `service_clients` configuration is rejected; bundled owners require no relationship secrets. Independent owner commands retain their authenticated HTTPS Client configuration. Existing `UnifiedConfig` library compositions remain available, including explicit HTTP hosts; the CLI create/join/serve path requires HTTPS. CLI reset/recovery and certificate renewal tooling are not supplied by these commands. See [unified bootstrap reference](../config/unified/README.md).

## Desktop boundary

Desktop's restricted native flow covers create/join, approval, Device completion and local reset. Fresh create automatically derives a unique retained logical issuer from its Installation ID, independent of the HTTPS listener. Join takes the pinned peer and Node name, uses the shared `discover_installation_issuer` authenticated purpose/nonce-bound discovery helper, then retains the issuer/peer/name for exact retry. The manual issuer UI/input is removed. Existing retained setup intent is reused, not silently assigned a new issuer. Desktop generates/retains relationship secrets through the same `RawKeyringRepo` provider pattern and composes the three contexts on one retained-port HTTPS listener. Shared discovery/provider behavior does not make its native lifecycle/reset interface identical to the CLI.

Desktop certificate preparation requires OS CA installation and native-root listener verification; stale trust markers do not waive installation. Denied elevation/provider/TLS verification stops the flow without browser-only/plaintext fallback. Runtime readiness and retained Installation/Device completion are separate from setup status. Errors close retained owners; reset is local and preserves retry state on secure-store deletion failure. These are source boundaries, not fresh GUI/native-platform passes.

## Client configuration and distribution

Application clients still need the verified public service addresses of their selected Node. For password-manager, set complete service bases `PUBLIC_LIDP_BASE_URL` (including `/idp`) and `PUBLIC_STORAGE_BASE_URL` (including `/storage`). These are application deployment settings, not internal bootstrap prerequisites. `PUBLIC_STORAGE_AUDIENCE`/origin fallback is removed; filesystem token exchange uses `resource=storage:filesystems`.

Build the Storage package explicitly before publishing its public `esm`/`types` exports; no automatic `prepack` hook exists. Ship complete build output, including generated dependencies. Historical isolated package-build and unaliased-consumer checks do not establish current deployment acceptance. A workspace source alias does not replace the separate distribution build.

## Container deployment recipe (not image-qualified)

The root `idp-unified` binary forwards to `unified_server::run`; its historical name does not mean IdP-only behavior. The Dockerfile builds `unified-server` with `cli`, using GNU/Linux stages and D-Bus Secret Service libraries. Stage revision-matched `of`, `ofdb`, `offs` and `ofnet` source trees as siblings. The prospective build, from that staged parent, is:

```sh
df -h .
docker build -f of/Dockerfile -t of-node:local .
```

No image build/pull/run is claimed here. `of/Dockerfile.dockerignore` takes precedence over a context-root ignore and excludes conventional local state/secrets/build artifacts; it is not a secret scanner. Keep populated secrets and operator state outside the source/build context. The static recipe check is `python3 -B of/scripts/test_container.py`; a static check is not image/runtime qualification.

The image runs as UID/GID `65532:65532` and currently defaults to `-c /app/config.yaml serve`, an explicit-config deployment, not the ordinary no-config default. Changing packaging is outside this documentation update. Override image arguments for `create`, `join` or no-config `serve` only with the same retained root/provider identity. Persist the entire selected Node root and the secure provider's state; do not clone signers or share roots between Nodes.

The current runtime image contains CA certificates and D-Bus libraries, **not a complete qualified elevation/trust-install environment**. D-Bus libraries do not start/unlock Secret Service. Arrange a protected persistent provider for the same operator identity and working trust installation/elevation before bootstrap. Do not assume non-root container setup, host trust propagation, unattended keyring access or logout/restart persistence works. A volume or environment file is not a substitute for the secure provider. Avoid privileged containers, widened socket permissions or mounting whole host runtime directories to mask failures.

The CLI rejects non-loopback listeners. Bridge port publication does not expose its loopback API. Any deployment proxy must reach the Node's loopback HTTPS listener (for example, in a deliberately shared network namespace), verify its CA, and preserve `/idp`, `/management` and `/storage` exactly once. Do not use an HTTP upstream or disable certificate verification. Native-root completion probes do not use proxies, so proxy trust alone cannot satisfy mandatory local OS installation. Container/host networking, Iroh reachability, proxy TLS, dependency linkage, Rust image availability and offline process/container restart require separate qualification; this is not an unattended deployment runbook.

## Qualified local lifecycle and credential rotation boundaries

Historical focused shared-runtime checks used real isolated context stores and qualified readiness-gated background/router startup, release on startup error, consuming shutdown with a surviving router handle, offline membership restoration/learned allowlist revocation and exact retained-host reopen. They do not qualify universal cancellation/crash cleanup or all detached owner mesh-task joins; the owner mesh API does not expose a join handle for every task.

Historical Desktop regressions qualified ordinary-error teardown with actual runtime/TLS owners: listener-close failure did not skip runtime shutdown, setup/restored-listener verification errors closed owners, initiating errors were preserved and returned window-close cleanup errors requested exit code 1. An injected exit callback is not a real Wry process-exit/restart test. Concurrent setup/close, hung shutdown and crash/cancellation remain unqualified.

The historical rotation regression qualified authorized **same-ID Storage→IdP secret replacement** through the unified HTTP fixture, old-secret rejection, completed-host reopen/retry with the replacement, a consuming Storage `IdpClient` call, unchanged journal/provider inventory and registration-deletion denial without recreation. Its provider was a narrow mock. Bundled Management and Storage now call IdP locally and no longer provision those service credentials; their obsolete bundled rotation fixtures have been removed. That historical result does not establish automatic CLI/native HTTPS rotation, unattended OS-provider behavior, independent relationship rollout, peer propagation or interrupted multi-consumer cutover.

For a same-ID replacement, an authorized User bearer for the IdP service audience needs the exact Installation infrastructure-client read/update permissions. Read `GET /idp/oauth2/register/{client_id}`, retain all metadata, then `PUT` the full registration with only the securely supplied `client_secret` replaced. This is not PATCH: omitted arrays can clear scopes/audiences. Omitting the secret retains the verifier; it does not generate a secret. Protect the successful response, which contains the supplied secret. Securely update the consuming provider entry (or explicit composition's injection), then stop/reopen with the same Client ID/public relationship. Do not delete/edit intent to bypass mismatches. Registration changes and consumer updates are not an atomic multi-consumer cutover.

Replacement blocks old-secret issuance but does not itself invalidate issued bearers. Authorized `DELETE /idp/oauth2/register/{client_id}` revokes the registration locally and requires exact infrastructure-client delete permission. Deleting an independent relationship denies its subsequent authenticated use. It is not a safe replacement rollout; partitioned peers may not yet know the revocation.

## Trust, partitions and revocation

At startup, `of` restores persisted locally trusted approved/non-revoked Device endpoint membership into the shared `iroh-chain` allowlist and updates it as changes are learned. `EndpointIdStore` is in-memory, not a persistent trust store. Completed local identity/policy/signing capability does not require central contact, the joining source to remain online or a 30-second freshness lease. Missing, invalid or locally revoked evidence denies the affected operation; remote disconnection alone does not invalidate completed trust.

Pairing and connection admission are not resource authorization. Both peers independently check current ownership, namespace, selection and restrictions at execution boundaries, including existing connections. Learned revocation blocks subsequent operations; already-authorized work may finish. Previously copied bytes cannot be recalled. Transfer/deselection gates affected local use until managed-copy cleanup/reset; an unaware offline Node can temporarily act under its earlier view. OAuth consumption is atomic locally, with no global single-use guarantee during partitions.

## Secrets and local identity

Keep independent private signers, raw passwords and raw client secrets local. Synchronize only approved public/domain records. Protect CA/server/private-signing material; manifests retain IDs/references, not raw secrets. Never commit populated secrets or log tokens/credential bodies.

| Caller → receiver    | Required scopes                                               | Audience boundary                                     |
| -------------------- | ------------------------------------------------------------- | ----------------------------------------------------- |
| Management → IdP     | `idp.token.validate`, `idp.device.lookup`                     | IdP service audience                                  |
| Storage → IdP        | `idp.token.validate`, `idp.device.lookup`, `idp.device.list`  | IdP service audience                                  |
| Storage → Management | `management.replication.read`, `management.replication.admit` | Management Storage audience                           |
| IdP → Management     | `management.permission.evaluate`                              | Management Application audience; registered evaluator |

All table rows describe independent HTTPS client contracts. Unified and desktop bootstrap register no confidential relationship Clients: Management and Storage bearer validation and endpoint discovery use current local IdP callbacks; IdP permission evaluation and Storage replication use current local Management authorization. No bundled OAuth self-calls or relationship-secret inventory is required; independent setup remains incomplete. A configured remote Management client alone does not supply the current Device policy needed for Storage replication.

IdP uses Client audience allowlists for issuance and inspected-token validation: Management→IdP additionally lists `idp-management` and the Management Storage audience. Independent Storage→IdP clients list `storage:databases` and `storage:filesystems`; bundled Management and Storage require no introspection Client. Only a separately admitted evaluator may assert a User for remote permission evaluation; bundled Management admits none. These are not User/resource grants or a central role. Never widen permissions to mask failed provider access, token validation or rotation.

## Persistence, reset and recovery

Persist whole context stores, endpoint/host identity, public references, protected TLS and provider state. `ofdb` owns SQL/KV sync/staging/batching/cleanup through public `ofdb-sql`/`ofdb-kv`; `offs` owns filesystem persistence/sync; `of` owns domain/protocol adapters over shared `ofnet` networking. Mesh messages do not imply transparent multihop resource streams. Do not run competing processes on the same roots or copy private signers to another Node.

### Historical stopped same-Node filesystem recovery (retained mock provider)

The recorded recovery fixture restored the entire configured IdP, Management, Storage and endpoint roots plus provisioning state to the same paths, preserving tree/bytes/modes and lock inode. Public `LocalProvisioner::open_runtime`, signer verification, endpoint identity, readiness and authenticated owner calls checked two restored reopens. Missing/corrupt roots or identity denied without repair. This was filesystem restore, not a provider export/import, recovery command or populated Storage-content backup.

That fixture retained the same private-key capability in the same mock provider and unchanged configured Client credentials. It does not qualify the new default root's additional `host.json`, credential/TLS state, lost providers, real OS identity changes, signer cloning, network-isolated execution, torn snapshots or crash-consistent backup. For current Nodes preserve all additional retained state described above, including IdP inventories/references/locks and the whole Storage tree; merely restoring `idp.redb` or directory names is insufficient.

No complete executable current-format backup/recovery procedure is verified here. Do not routinely roll back authorization history: even a consistent old snapshot may precede revocations, rotations or resource changes. Missing/corrupt/mismatched endpoint, signer, TLS, intent or required context state refuses existing-only startup without setup replay or identity regeneration. Preserve rejected state for diagnosis and explicit recovery.

Desktop reset affects only local setup/data/identity and may proceed offline; attempt authorized revocation/propagation when possible and preserve retry state on secure-store deletion failure. CLI create/join/serve is not reset tooling. Deselect removes a local copy, not the global resource; deletion synchronizes a Tombstone. Passthrough reads need an authorized reachable Full peer and cannot write.

## Further qualification

Deployment-specific native CA installation/elevation, real provider availability/persistence, process restart, container/proxy/Iroh connectivity, interrupted rotation and current-format backup/recovery need dedicated evidence. Existing historical results do not establish current-tip complete validation. Generic persistence/cleanup guarantees remain with their owners; consumer composition must not infer them from connection admission. Check disk space before image builds; do not prune Docker or delete caches/data without review.

Bundled startup refuses obsolete one-/two-/four-relationship references and Create/Join/native manifests unchanged. No truncation, migration, secret replacement or automatic cleanup is performed. Native explicit reset captures all historical Client IDs and key names, preserves original evidence across cleanup failure, and removes it only after successful cleanup.
