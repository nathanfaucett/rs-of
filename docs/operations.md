# Node operations

Current owner/database boundaries and Device routes: [shared control-plane databases](control-plane.md).

These are current operational requirements for the [offline-first Node design](design.md), not a fully qualified deployment runbook. Ordinary unified CLI create/join is automatic apart from the human password, trusted peer choice and authorized approval; TLS terminates at the deployment proxy. Desktop/native and independent-owner HTTPS startup still require OS trust/elevation consent. Internal IDs, secrets, audiences, context roots and password files are not prerequisites.

## Qualification boundary

The approved qualification scope is reproducible bounded Ubuntu Linux x86_64 evidence. Multiple isolated Nodes on one host can qualify protocol/partition/reopen semantics without physical machines. Historical local qualification includes production authenticated join/completion/offline native reopen, session-provider setup/reset, completion-marker checks, four representative durable boundaries and focused DOM/client-SSR checks. The recorded durable regression took 350.97 seconds under a 360-second internal bound; earlier larger-fixture timeouts are not erased by that result.

**Verified GUI approval, profile-B completion and two-profile GUI offline restart remain deferred and not observed.** Offer display is not approval. Listener/status probes, receipts, markers and Residency acknowledgements never substitute for current trust/readiness or resource authorization. Historical lifecycle, rotation and retained-provider filesystem recovery results retain the narrower boundaries below; they are not current-tip reruns.

Windows/macOS, headless/container deployment, real WAN, lost-provider/crash recovery and exhaustive GUI/fault/feature matrices remain deferred qualification, not claimed support. Broad Cargo Hack/CRAP sweeps were outside that bounded machine-local qualification. Security, integrity, fail-closed startup and known local implementation defects remain in scope. This documentation update performs no code validation or native installation; supplied focused implementation results are recorded separately below, not independently rerun here. See [API contracts](api.md#setup-join-and-readiness) and [security boundaries](security.md).

Current scoped evidence through integration `a4caa2f71` supersedes intermediate setup/Join timeout, cleanup and generated-client blockers. Historical receipts retain their original failures and source/profile; this documentation does not rerun their tests:

- [Receipt 42 — final lifecycle review](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md): concrete review fixes and final focused qualification pass, including independent Create and historical-record reset, paired current/historical Join twice, actual HTTPS transfer and original-owner/replay checks, current readiness, delayed owner availability, reset crash windows, exact trust-removal fallback, owner regressions, strict affected-library Clippy and static boundaries. API library: 37 passed / three explicitly launched wrapper-child ignores; Management library: 38 passed / two explicit ignores. This is bounded same-host Ubuntu Linux x86_64 evidence with isolated native roots and mock/injected providers/trust commands, not real OS installation or production deployment certification.
- Receipt 42 proves an actual authenticated independent `GET /storage/databases` for a Management-approved endpoint absent from IdP signer trust, not just policy publication. The same OAuth token and counted single accepted TLS connection exercise policy allow/deny; retained Storage reopen preserves denial, and endpoint revocation denies on the same connection and after another retained reopen. Final-source operation children pass twice (49.37s and 50.04s). Valid OAuth evidence never substitutes for current endpoint policy.
- [Receipt 40 — signer-trust batching](../.scratch/ui-driven-node-bootstrap/evidence/40-signer-trust-batching.md): unchanged current/historical Join and their first authenticated Storage request pass in two consecutive paired runs under unchanged bounds. Receipt 42 repeats both paired fixtures successfully (129.32s and 131.30s suites). Earlier timeout receipts are historical, not a current independent Join blocker; finite repeats are not a universal latency SLA. No warming request, policy cache or deadline increase manufactured acceptance.
- [Receipt 41 — current Management contract/SDK and frontend](../.scratch/ui-driven-node-bootstrap/evidence/41-management-cleanup-client.md): actual 38-operation Management export, generated cleanup SDK and package build pass; 252 generator/SDK/API checks pass with no skips, 158 state/route tests and four setup DOM interactions pass. The generated `ownerCleanup` method preserves the exact tuple, optional/null pending evidence and 204/401/403/503 semantics. Frontend adoption is complete; these boundary-substitute tests are not real GUI/TLS/provider certification.
- [Receipt 43 — completed final native/shared check](../.scratch/ui-driven-node-bootstrap/evidence/43-final-native-shared.md): native `--tests` compilation, all 17 library tests plus the explicitly launched provider child, unified Create four tests plus its launched child, and combined HTTPS Create/Join/offline reopen pass (74.19s parent / 74.10s child). Rust is unchanged from `a4caa2f71`; no source fix was needed. This completes the final-source regression check after receipt 36, within the same Ubuntu/mock-provider/native-root boundary. See [native contract](../apps/app/src-tauri/NATIVE-SETUP.md).

Historical frontend blocker: receipt 41 recorded five missing built Storage SDK exports. OpenAPI ticket 02 supersedes that result with passing SDK ESM/declaration builds, actual-router built consumers and an app check with zero diagnostics after its localization/headless prerequisites. Password Manager localization/typecheck and broader workspace/native qualification limits remain recorded in ticket 02. The current shared contract/generation gate is `pnpm openapi:check`; see [API workflow](api.md#developer-and-ci-gate).

No Cargo, code tests, GUI, real provider/profile or trust-store operations run for this docs-only reconciliation. [Receipt 44](../.scratch/ui-driven-node-bootstrap/evidence/44-acceptance.md) records bounded spec acceptance and resolved tickets 02/03/04, with no implementation TODO; this documentation adds no execution result. Real OS install/elevation/provider persistence, rendered GUI, WAN/container and other-platform certification remain deferred, not implementation blockers inferred from the completed same-host seams.

## Background replication

An idle unified Node does not periodically query owner databases to refresh admission or scan Storage resources. Committed IdP/Management changes invalidate admission; actual peer-inventory and resource-catalog changes reconfigure workers. SQL/KV sync owns initial catch-up, change-driven reconciliation and failed-delivery retries. Filesystems use independent persistent `offs` sessions. Read-only commits and replayed state do not schedule another sync pass.

Authorization remains checked during exchanges. Revocation, policy-feed closure and peer removal fence existing sessions; pending local-copy cleanup still runs without peers. Unavailable peers can cause bounded-backoff delivery attempts, but not periodic resource-inventory scans. See [Storage replication lifecycle](../crates/storage-server/REPLICATION.md) for host integration and the standalone remote-policy feed limitation.

## Fresh API startup and app setup

Ordinary `unified-server`/`serve` on a genuinely fresh store starts a neutral **HTTP** setup host rather than requiring CLI provisioning. It retains listener/host prerequisites without choosing an Installation issuer or establishing domain state. The unified CLI never generates, installs or opens server certificates: portless or the deployment proxy owns public TLS. The listener is loopback-only; do not expose backend HTTP to untrusted networks. A separately deployed web app must use the correct public API base and the exact `idp.ui_public_uri` as its allowed browser origin. No-config startup derives public URLs from `PORTLESS_URL` when present, otherwise from the loopback HTTP listener. Only exact loopback origins may use HTTP; other setup UI origins require HTTPS. Desktop/native constructors retain their existing local HTTPS, CA installation and native-root verification.

Unified keeps the same HTTP route surface throughout setup and normal operation. All three owners expose version and complete OpenAPI documents before initialization without opening domain stores. Discovery, JWKS, health and operational routes remain registered; until their runtime exists they return `503` with `setup_required` (fresh awaiting-choice host) or `node_not_ready` (incomplete/recovery state), rather than disappearing as `404`. Unknown paths remain `404`, unsupported methods remain `405`, and runtime authentication is unchanged. `/idp/setup/status` remains the source of setup/readiness evidence; requests to public metadata never initialize a Node.

Unified CLI startup prints the local HTTP base and a **protected management retrieval path**, not the setup code. Authorized host management can retrieve the 64-character code from the second line of that dedicated private state file; never expose the file publicly, weaken its permissions or put the code in logs/URLs. The app's web setup page submits it in an explicit authorization header without cookies or persistent browser storage. Code acceptance does not sign in, approve membership or complete setup.

Fresh independent owner CLIs now work without relationship credentials: new default roots/ports are `idp-data`:3000, `management-data`:3001 and `storage-data`:3002, with existing configured/legacy roots and listeners preserved. Management/Storage keep those default ports on pre-domain retry, post-domain retry and completed reopen whenever retained setup-host state exists; domain creation must not switch the listener back to a legacy default. Explicit configured ports take precedence. Existing domains without setup-host state keep their existing configured/default listener and are not claimed as fresh setup. Each retains separate `<domain-root>.setup-host` metadata, TLS and code. Owner-relative setup paths use the actual configured prefix once. Optional `server.setup_ui_origin` is the exact HTTPS web-app origin; IdP otherwise uses `ui_public_uri`, while Management/Storage use the existing default app origin. Configure explicit `PUBLIC_IDP_BASE_URL`, `PUBLIC_MANAGEMENT_BASE_URL` and `PUBLIC_STORAGE_BASE_URL` for independent web routing; the app does not derive service bases or shared admission from an origin/path. Status `owners` distinguishes one unified target from separate singleton owners.

Startup installation is cancellation-owned, including termination/reaping of ordinary owned installer process groups; escaped/privileged descendants and real elevation behavior remain unqualified. Arbitrary verifier injection is available only under nondefault `api/test-support`, enabled through tests, not production dependencies.

The existing app submits unified and independent-owner Create/Join through verified HTTPS, observes host-owned work, recovers exact nonsecret intents, retries and explicitly cancels. A durable 202 is acceptance, not readiness; browser disconnect does not cancel work. Input codes/passwords remain transient; relationship secrets never pass through the browser. Poll current status and correlate all independent operation/issuer/base evidence before sign-in. See [literal contracts](api.md#setup-join-and-readiness) and [worker ownership](setup-workers.md).

After reload, enter the current target code(s) and recover the retained operation. Recovery alone resumes nothing. Recheck current scope/status, re-enter any required Create password and retry the exact tuple, or signal cancellation for the exact active intent. Do not replace its nonce, peer, issuer or identity. A complete-but-unretired owner remains unfinished until exact authorized finalization; retired owners use matching public evidence, not retired codes. Cancellation is per-bound-owner signaling, not rollback or finished cleanup. Missing/corrupt admission, TLS, worker evidence or attempted protected material refuses without regeneration.

A neutral awaiting-choice host still cannot be adopted by optional CLI Create/Join; those conflicting commands refuse. Preserve it instead of deleting files to bypass this limitation. The CLI examples below apply to their fresh/matching CLI intent, not that neutral host. Same-host HTTPS and frontend seam evidence does not qualify real OS installation, GUI or universal deployment behavior.

### Independent Create and Join workers

The three fresh owner CLIs dispatch to concrete Create workers without a second serving command. Submit the same [exact operation](api.md#independent-create-worker-contract) to each intended owner with its own host code; send the administrator password only to IdP. The browser/operator never assembles relationship secrets. Admission returns 202 independently of provisioning; poll all owners rather than interpreting acceptance or one ready owner as global completion. Closing the client does not cancel work. Authorized recovery/cancellation/retry preserves the exact operation and generated identities. After stopped restart, complete-but-unretired state needs exact authorized finalization; missing/corrupt attempted material refuses without repair or regeneration.

Independent Join uses the same owner-local codes/lifecycle, with `purpose:"join"`, empty administrator/issuer, no password, and the source's exact HTTPS Management base plus `#<pinned-endpoint-id>` as `peer`. Each owner authenticates discovery and retains the source Canonical Issuer. Recover the exact nonsecret tuple separately from each owner before retry/cancellation. Enable acceptance and approve/reject the exact offer on source Management using an ordinary authorized User bearer, never a target Setup code. Join imports approved public identity/policy through the respective owners, not private passwords or resource content. Completed reopen and exact unretired finalization work with the source offline. See the [request/status contracts](api.md#independent-join-worker-contract).

The [Create receipt](../.scratch/ui-driven-node-bootstrap/evidence/independent-create-worker.md) and [Join/security receipt](../.scratch/ui-driven-node-bootstrap/evidence/independent-join-security.md) record isolated native-root HTTPS, authenticated Management/Storage use, interruption/rejection/exact retry, cancellation, stopped reopen and refusal on lost evidence/material. The initialization fence must remain with `worker.json`: a lost initialized record refuses before domain creation, without replacing Client IDs/provider material. Do not delete setup files to force fresh setup. The app uses the generated independent SDK and correlates all-owner evidence before completion. Injected commands/mock providers do not qualify real OS installation, containers, WAN deployment or arbitrary crash checkpoints. No cross-owner store opens, copied endpoint private keys or per-process Device/Node enrollment are introduced.

Completed independent status is a current observation, not a durable ready latch. IdP checks current local signer/private material, active User and registrations; Management checks current Device/endpoint/owner/policy, allowance and cleanup fence; Storage checks local retained material/catalogs/receipts and current policy over its configured Management/IdP relationship. Failure returns nonsecret `503 unavailable`, `ready:false`; later valid observation recovers without setup replay. Normal API authentication remains unchanged. Completed reconstruction/reopen needs no Installation source peer, although current Storage observation/use still needs its configured owner services. The native-root HTTPS startup probe uses `OPTIONS` on the existing status path (200 from CORS or 405 from the method router), without executing readiness-bearing GET. Slow/unavailable configured owners can make GET time out or report 503 without cancelling the listener or removing normal authentication. See [exact readiness contract](api.md#independent-current-readiness-and-retained-schemas).

### App Device and source-approval owner routing

Device types and list/rename/revoke operations belong to the Management client. Source acceptance, pending offers, approval and rejection use the configured Management base plus `/node-joins/...` for unified sources or `/independent/node-joins/...` for independent sources, with its configured prefix applied once. Exchange the current User token at IdP for the `idp-management` audience, not the Management transport URL or an IdP-owner audience. Source approval remains ordinary User authorization, never target setup-code authorization. Desktop source controls use the same HTTPS routes, not native mutation commands.

The native reset peer-revocation consumer currently selects a composed peer by its exact HTTPS `/idp` base and verifies the retained Installation issuer before sending the User token. It revokes the signer at `/idp/replica-signers/{id}` using the IdP-owner audience, then separately exchanges for `idp-management` and revokes the Device at the same peer origin under `/management/devices/{id}`. Both successful responses are required before acknowledging the retained reset target. Partial success, redirects, unknown targets and lost acknowledgements retain the target for an authorized retry; signer revocation is not Device revocation. This composed-peer shortcut does not discover independent remote owner addresses.

The Management and IdP Device/identity clients are regenerated and built; Device methods/types are Management-owned, with no removed IdP compatibility routes. Receipt 41 exports and qualifies the current 38-operation Management contract, including generated owner-cleanup methods and pending-cleanup models. The independent SDK and bundled HTTPS frontend handoff are also delivered. OS CA trust installation and verified HTTPS remain prerequisites; certificate-warning bypass is never a setup step.

## Remote shared identity SQL

For a same-machine IdP-owned database, standalone Management may instead select an
explicit `identity_socket` and finite `identity_socket_deadline_ms`, mutually
exclusive with `identity_database`. It connects through the existing Unix SQL
client and validates the identity schema; there is no local file opening,
migration or fallback. See [local-owner startup, authority and reset rules](control-plane.md#default-same-machine-independent-setup-unix).

Standalone IdP and Management can attach to the same initialized identity database. Their identity repositories use the same `db::Client` query surface as an in-process Unified Host; only startup selects the connection. Unified composition still injects local clients without a SQL listener. Device state and user Storage replication do not use this identity connection.

Run the application-aware database owner, not the generic SQL CLI (which does not install the identity schema):

```sh
cargo run -p idp-server --bin identity-database -- --database ./identity.redb --listen 127.0.0.1:50051
```

The owner runs IdP and authorization-policy migrations before serving with the existing ofdb-sql server. Remote HTTP consumers only validate schema and conflict-operation readiness; they never migrate or fall back to a private identity file. Mixed historical credential schemas and servers without conflict inspection fail startup. Local database-owner and remote modes are mutually exclusive.

Add the same section to both standalone servers' YAML configurations:

```yaml
identity_database:
  endpoint: http://127.0.0.1:50051
  deadline_ms: 10000
```

Plaintext SQL is restricted to literal loopback IP addresses. For other deployments the owner requires `--certificate`, `--private-key`, and `--client-ca`, and consumers require an HTTPS endpoint plus `ca_certificate`, `client_certificate`, and `client_key` file paths. Use private listeners and identity-domain certificates. A trusted SQL client certificate grants the exposed SQL interface, not table-specific OAuth authorization. Never give Storage these identity SQL credentials.

Keep each consumer's `data_dir` separate from the database owner's directory. IdP still needs its Installation issuer, local signer binding and locally provisioned OS-keyring private material; sharing SQL does not provision or transfer a signer. Do not combine remote identity SQL with IdP's separate `management_data_dir` or `management_permissions` source. Configured serving with an existing local signer attaches to an initialized Installation. Fresh remote IdP and Device consumers also support the existing independent Create/Join workers; see [remote setup, attachment and reset](control-plane.md#fresh-create-join-attachment-and-reset). Remote Join attaches to the same Installation's initialized shared owners and revalidates exact source approval/current rows. Peer-history synchronization into a different database still requires a local owner and is not a query-only remote capability.

Stop the consumer before resetting its retained local setup state. Reset uses that runtime's local inventory and leaves the SQL owner's rows intact. IdP secret cleanup with a retained inventory currently needs the remote owner available to verify metadata; failure preserves reset evidence rather than deleting shared records. Timeouts are not evidence that a remote write or commit rolled back; do not blindly retry ambiguous writes.

Focused acceptance starts the SQL owner, IdP and Management in separate OS processes and exercises authenticated HTTP deny → Management assignment → IdP allow → consumer reset/restart → revocation → deny, plus SQL-owner outage. The fixture uses isolated mock keyring and OS-trust tools and 16 MiB runtime stacks. That identity-process fixture is not production OS-provider, mTLS-deployment or default-stack qualification. Remote Device process admission and remote Create/Join/reopen/reset have their own narrower [acceptance boundary](control-plane.md#fresh-create-join-attachment-and-reset).

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

The default Node root is `$XDG_DATA_HOME/of/unified`, or `$HOME/.local/share/of/unified`. `--data-dir` optionally chooses another root. The CLI uses localhost port `3000` by default, respects `PORT` (including portless's assigned backend port), and lets `serve --port` override it. Invalid/zero `PORT` values refuse startup. It durably retains public settings in `host.json`, then derives separate IdP/Management/Storage/endpoint roots, public Application URIs, audiences and a local key namespace. `PORTLESS_URL`, when present on fresh startup, supplies the public origin for those URLs; otherwise they use the local HTTP origin. On retained no-config hosts, `PORTLESS_URL` selects the validated runtime setup UI origin without rewriting retained Application URIs, audiences or Installation identity. Backend `PORT` can also change without changing that identity. `just unified` runs this server behind portless with backend port `3000` (or the supplied `PORT`). Native `BootstrapConfig::create` still selects and retains an ephemeral localhost HTTPS port. No manual root layout, internal UUIDs or service-secret environment variables are needed.

A new Installation's Canonical Issuer is `https://<retained-random-UUID>.installation.invalid/idp`, a unique retained logical identifier, **not** the local HTTP API address. The CLI serves backend HTTP; public HTTPS is supplied by the proxy. Native APIs continue to use their saved local HTTPS listener. Separate Installations using the same localhost port still have distinct issuers. Join retains the authenticated peer's issuer while using its own API address. Restart/continuation preserves the exact identity/settings; port contention refuses rather than selecting a replacement. Missing/malformed host identity requires explicit recovery. Earlier address-based retained host intents are not silently migrated or renamed.

Bundled owners use current local owner calls and provision no confidential relationship Clients or relationship secrets. The public operator-login Client and independent Node signer remain. Protected local TLS belongs to native/desktop HTTPS hosts, not unified CLI servers. The historically named `CredentialBootstrap` and `.credentials/credentials.json` retain exact host settings and TLS production evidence only; they perform no service-secret provider operations. `host.json` contains public identity/settings, never passwords, private keys or tokens. Ordinary create/join does not load `.env` or require `UNIFIED_*_CLIENT_SECRET` variables.

### Public operator login

Both create and join automatically provision a separate **public** operator-login Client with a retained ID; no manual Client registration or Client secret is needed. Completed retry/reopen verifies and reuses that registration rather than generating a replacement. Create/join print `Operator login Client`, `Login scopes` and `Token-exchange audiences`; `serve` also prints the local HTTP base, public IdP URI, issuer, IdP token endpoint and service audiences. Use the actual printed Client ID, not an invented UUID or one of the confidential service-client IDs.

Sign in through the ordinary `POST /idp/oauth2/token` route at the printed API address (use the proxy's HTTPS address for remote/browser clients), with an `application/x-www-form-urlencoded` password-grant request:

| Field        | Value                                                                                                                   |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `grant_type` | `password`                                                                                                              |
| `client_id`  | Actual `Operator login Client` ID printed by the CLI                                                                    |
| `username`   | Retained administrator username, e.g. `operator` or the chosen `alice`; on a joined Node, an existing Installation User |
| `password`   | That User's password, supplied through secure hidden input/client handling, not argv/logs                               |
| `scope`      | `openid` (optionally `profile email`, within the printed login scopes)                                                  |

The public Client uses token endpoint authentication `none`: do not supply a Client secret or confidential-client Basic authentication. Exchange the resulting User token through the same ordinary token endpoint for the required printed IdP/Management audience, with exact action scopes; the Client's audience allowlist is not the User's permission grant. Use the resulting authorized bearer for source acceptance/approval. Password authentication signs in an existing User; it is not Node enrollment, and join neither invents a new administrator password nor bypasses User authorization.

### Trusted peer and authorized approval

On the completed source, run `serve`, sign in with the automatically retained public login Client (or another existing ordinary authorized client/session) and enable acceptance with the authorized `PUT /management/node-joins/acceptance` route. `serve` prints the Node endpoint fingerprint and local HTTP base; remote/browser clients use the proxy's HTTPS origin. Transfer and verify the source fingerprint through a trusted channel; a discovered address alone is not trust evidence. Run `join` on the other machine and supply that fingerprint when prompted (or with `--peer`).

Before producing Node signer/service material, no-config join learns the source's Canonical Issuer over authenticated pairing. `of.equal-node-issuer-discovery.v1` binds purpose, fresh nonce and the pinned endpoint. The serving background task answers issuer probes while acceptance is enabled and current public Node state is ready, without pending-list polling or approval UI invocation. Discovery exchanges public identity, **not enrollment**. The peer/issuer are retained before Node production; unsigned HTTP/DNS metadata or an arbitrary discovered peer cannot substitute.

The production composed HTTPS router exposes:

| Route                                      | Operation                                                                           |
| ------------------------------------------ | ----------------------------------------------------------------------------------- |
| `GET /management/node-joins/acceptance`    | Read acceptance state                                                               |
| `PUT /management/node-joins/acceptance`    | Set acceptance with `{"accepting": true}` or `false`                                |
| `GET /management/node-joins/pending`       | List authenticated offers, including `offer_id`, authenticated endpoint and request |
| `POST /management/node-joins/{id}/approve` | Explicitly approve the exact pending offer                                          |
| `POST /management/node-joins/{id}/reject`  | Explicitly reject the exact pending offer                                           |

All routes require ordinary User authorization with Management audience (`idp-management`) and both exact Device pairing-update and signer-enrollment permissions. Inspect the exact offered Installation, Device, endpoint, owner and independent signer; these are distinct bindings. Target Setup codes and IdP-audience tokens cannot approve source work. App source controls work over HTTPS on web and desktop. Unified approval returns 200; independent approval returns host-owned 202, not completion. Independent acceptance PUT returns 204 and is read back; unified returns the state.

Approval checks Management Device/owner/policy and separate IdP signer trust, initializes only absent default policy, preserves restrictions/selections and rechecks authorization/enrollment before signed reply. Failed enrollment creates no policy; post-enrollment failure remains partial for exact retry, never automatic rollback/replacement or general resource access.

The source must run the updated `UnifiedRuntime`. Older sources without the purpose-bound issuer response cannot support no-config join; update them, or use an explicit composition that already supplies the trusted issuer. Never replace authenticated evidence with unsigned discovery. Signed approval/bootstrap races use bounded handshake/admission acknowledgment retry with exact anchor, private binding and learned-grant checks; bulk public state transfer does not use that short budget. No general allowlist bypass is introduced.

### Transport, OS trust and completion

**Unified CLI create/join/serve and neutral setup use HTTP and require no server certificates or trust installation.** Public TLS terminates at portless/proxy. CLI bootstrap remains Unix-only and loopback-only. Setup codes, exact browser-origin checks, retained intent checks, ordinary User authorization and current owner readiness are unchanged. Never forward the protected setup-code file or expose backend HTTP publicly.

**Desktop/native HTTPS is unchanged.** Native setup still generates/opens its retained localhost CA, requires OS trust installation and verifies native-root HTTPS; there is no insecure-certificate fallback.

Linux first tries `trust anchor`, then uses the matched platform anchor destination and refresh command: Debian-family `/usr/local/share/ca-certificates` with `update-ca-certificates`, or Fedora-family `/etc/pki/ca-trust/source/anchors` with `update-ca-trust extract`, via `pkexec`. macOS uses the login keychain. These code paths are not native installation qualification for every platform. The deployment must provide working secure-provider access, trust tools and elevation; a library or installed command alone does not prove that they work.

The local transport base is configured before runtime construction, separately from public proxy URLs. CLI completion checks current local owner readiness and closes its temporary runtime without starting a TLS listener. Native HTTPS completion additionally verifies the production listener using native roots, without redirects or proxies. It does not obtain service tokens or run a health/service-relationship integration suite as part of provisioning; those remain explicit integration qualification. Owners and the verification listener are closed before durable completion is published; create/join then exit. Their optional CLI `serve` entry point reopens existing state; ordinary fresh API startup uses the neutral setup host described above. Neither path turns missing retained credentials/trust or an incomplete intent into fresh setup.

### Retention, retry and explicit configuration

Retain the whole Node root, including `host.json`, the context/endpoint roots and endpoint-adjacent `.credentials`, `.tls` and `.provisioning` directories, plus the secure provider's retained state. Manifests contain public metadata, references, one-way verifiers and completion phases, not raw secrets. Protected files/directories use Unix owner-only checks; stable operator-controlled nonsymlink ancestry is required. These checks are not hostile concurrent pathname-race protection.

Cooperating CLI operations hold retained locks through setup/serving. Preserve `operation.lock` and its inode; never unlink it, remove intent/provider references or delete context data to clear contention or manufacture freshness. Missing attempted credentials, endpoint/signing material, partial TLS, malformed intent or unacknowledged Management/Storage initialization require explicit recovery rather than replacement. Bundled completed startup checks current local identity, public login registration and grants; a historical completion marker is not an authorization lease. It has no confidential relationship credentials to load or rotate. Independent HTTPS consumers retain their scoped Client credentials and owner checks. Missing/revoked registrations are never recreated by restart.

`--config config/unified/config.yaml` is an **optional explicit composition**, mutually exclusive with `--data-dir`. Bundled `service_clients` configuration is rejected; bundled owners require no relationship secrets. Independent owner commands retain their authenticated HTTPS Client configuration. Existing `UnifiedConfig` library compositions remain available, including explicit HTTP hosts; the unified CLI create/join/serve path uses backend HTTP, while native/desktop retains HTTPS. These unified commands do not supply reset/recovery or certificate renewal; independent owners have the separate stopped reset command below. See [default configuration reference](../config/README.md).

## Desktop boundary

Bundled desktop uses the shared HTTPS setup client and `ApiSetupHost` worker, one retained-port listener and one endpoint/router. Native owns OS trust/elevation consent, local host observation, protected `get_native_setup_code`, prerequisite retry, coordinated shutdown, explicit reset/reference recovery and Device marker completion. It does not independently Create/Join/cancel/recover/approve through native mutation commands. Device completion is bound to the verified public login Client and does not enable API authentication or establish Node readiness.

Only explicit bundled selection may retrieve/use the bundled capability or reset that local Node. Selecting remote bases must not retrieve local codes, substitute bundled services or mutate local state. Trust/provider/TLS startup failures leave an actionable UI for explicit retry/reset/reference recovery, not automatic regeneration. TLS clients are constructed after OS trust installation. Web/remote deployment installs required trust on all relevant hosts/clients; browsers cannot do it. No certificate-warning bypass, custom production CA client or plaintext fallback is allowed. See [native source contract](../apps/app/src-tauri/NATIVE-SETUP.md); receipt 43 completes final native/shared-host qualification on unchanged `a4caa2f71` Rust. This is not GUI/platform certification.

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

Independent transfer now has a scoped completion path: Management's service-only policy read returns exact `pendingCleanup` with a durable cleanup ID, empty resources and `adminAllowed:false`. The configured local Storage runtime retires old-owner SQL/KV/filesystem copies, rechecks that binding, sends `/replication/owner-cleanup` under `management.replication.admit`, and rereads current policy. Stale IDs/owner/endpoint changes deny; pending or failed retirement is never acknowledged as complete. An empty restricted policy after acknowledgement still needs explicit administrator allowance and new selections. Preserve cleanup facts/retirement fences for retry, including after stopped reopen; do not delete them or restore old generations. Remote-peer reads cannot drive local cleanup. See [literal transfer contract](api.md#owner-contracts); receipt 42 qualifies the actual HTTPS transfer, original-owner/replay, interruption and retained-state seams within the same-host mock-provider boundary.

## Secrets and local identity

Keep independent private signers, raw passwords and raw client secrets local. Synchronize only approved public/domain records. Protect CA/server/private-signing material; manifests retain IDs/references, not raw secrets. Never commit populated secrets or log tokens/credential bodies.

| Caller → receiver    | Required scopes                                                                         | Audience boundary                                     |
| -------------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Management → IdP     | `idp.token.validate`, `idp.user.lookup`                                                 | IdP service audience                                  |
| Storage → IdP        | `idp.token.validate`                                                                    | IdP service audience                                  |
| Storage → Management | `management.device.list`, `management.replication.read`, `management.replication.admit` | Management Storage audience                           |
| IdP → Management     | `management.permission.evaluate`                                                        | Management Application audience; registered evaluator |

Independent Storage uses the Management-approved endpoint ID, not a generated Storage transport key or per-service Device. Ordinary Create/Join derives that public binding through the existing owner exchange. Optional manual service configuration must provide `management_endpoint_id` (`STORAGE_MANAGEMENT_ENDPOINT_ID`) alongside its actual service bindings; it does not create or approve the named endpoint. The obsolete standalone Storage Iroh key/peer-refresh path is removed. Do not copy an endpoint private key or delete old identity/cleanup evidence as a startup workaround. Transfer cleanup uses the existing read/admit scopes, not another secret; see [the exact cleanup contract](api.md#owner-contracts).

All table rows describe independent HTTPS relationships established by the owner workers; consumer secrets remain provider-local and are handed directly to IdP under exact challenge authority, never through the browser. Unified/desktop register no confidential relationship Clients: IdP bearer/active-User validation and Management Device/endpoint/policy callbacks remain local. A remote Management client alone is not policy or resource permission. See [credential handoff](setup-workers.md#independent-credential-handoff).

IdP uses Client audience allowlists for issuance and inspected-token validation: Management→IdP additionally lists `idp-management` and the Management Storage audience. Independent Storage→IdP clients list `storage:databases` and `storage:filesystems`; bundled Management and Storage require no introspection Client. Only a separately admitted evaluator may assert a User for remote permission evaluation; bundled Management admits none. These are not User/resource grants or a central role. Never widen permissions to mask failed provider access, token validation or rotation.

## Persistence, reset and recovery

Persist whole context stores, endpoint/host identity, public references, protected TLS and provider state. `ofdb` owns SQL/KV sync/staging/batching/cleanup through public `ofdb-sql`/`ofdb-kv`; `offs` owns filesystem persistence/sync; `of` owns domain/protocol adapters over shared `ofnet` networking. Mesh messages do not imply transparent multihop resource streams. Do not run competing processes on the same roots or copy private signers to another Node.

Explicit native reset stops and joins the listener/worker before taking the stopped-host guard. It captures exact nonsecret key references and private manifest backups before deletion, validates original owner journals/inventories, and completes provider deletion before owner data removal. Failure retains data/journal/references; durable `data-pending` avoids repeating completed provider work. Reset may proceed offline but does not erase peers or remove OS CA anchors. Peer revocation requires separate ordinary authorized IdP signer and Management Device exchanges; both acknowledgements are required, with targets retained after partial failure.

Explicit reference recovery restores only missing exact backed-up manifests, never conflicts, endpoint keys, domain data, trust or admission. Lost/malformed provider inventories are not guessed. Old native `setup-ids.json`/`node-setup.json`/`services` state is refused intact, not migrated or treated as fresh. See [native reset contract](../apps/app/src-tauri/NATIVE-SETUP.md). Independent setup cancellation is not this native reset interface; no general remote reset endpoint is implied.

### Stopped independent-owner reset

**Destructive, local host-management only.** Stop the selected owner, including its setup-only listener, and use the same operator, configuration/environment and data-root selection as startup. These Unix CLI commands require explicit OS trust removal consent; execute only the one for the owner/root you intend to delete:

```sh
idp-server reset-independent --remove-os-trust
management-server reset-independent --remove-os-trust
storage-server reset-independent --remove-os-trust
```

The commands above use each owner's ordinary default/config/environment resolution; do not use defaults to reset a differently configured deployment. There is no browser/HTTP reset endpoint, new credential, automatic startup repair, remote membership revocation or peer data deletion. App/native profile reset is a separate interface.

Reset acquires the existing setup `operation.lock` nonblocking and holds it through deletion; live setup excludes reset even before domain creation. Missing original locks, admission, initialized worker evidence, mismatched owner/root or unknown/contradictory records refuse intact. Exact worker/intent/TLS evidence and matching PEM/DER CA fingerprint are captured before deletion.

**Schema notice:** ordinary IdP/Management opens require version-2 worker records and refuse older state without migration. IdP Join omits the Create credential; Management Join omits Create grant IDs, and Create uses `create:{role,permissions}`. Storage remains version 1. Explicit reset additionally validates exact historical version-1 IdP/Management records, including obsolete Create-only IDs/attempted slots; this is deletion support, not a serving compatibility path, migration or permission to invent missing references.

Provider deletion targets only exact captured slots under `independent-setup:<absolute setup-host directory>`: IdP consumer Client, Management consumer Client and endpoint, and both Storage consumer Clients. IdP signing/metadata material is deleted through its exact owner inventory (including that inventory's retained owner association on Join). No enumeration, prefix deletion, new keys or cross-owner store opens. Provider unavailability fails; an attempted slot already absent is idempotent only during this explicit reset, while unexpected material in an unattempted slot is a contradiction. Unrelated/source/foreign-namespace slots survive.

A private sibling `setup-reset.json` journal records **secrets → trust → data → cleanup**. Protected deletion must succeed first. Exact local OS CA removal must then succeed before local TLS/evidence is discarded: denial/unavailable tools retain the CA and journal. Domain data is removed only after those prerequisites; setup is atomically retired to the sibling `setup-reset` tree before final removal. Preserve the journal/retired tree across failure and rerun the same exact command/config; later data cleanup does not need already-deleted provider/domain metadata or regenerate material. Ordinary startup refuses either retained `setup-reset` or `setup-reset.json`, including both final cleanup windows, before creating fresh admission/TLS. Only exact explicit reset continuation can finish removal; never unlink live operation locks or delete fences to force a reset/retry.

Linux/macOS trust-removal paths identify the exact retained certificate/anchor, not a broad name or unrelated CA. Linux permits distro-only removal when the direct `trust` executable is absent; an existing tool's failed removal still requires successful native elevation. Exact fingerprint-derived anchor name and DER contents must agree, and refresh retries even after prior deletion. Receipt 42 qualifies these injected-command/refusal paths and current/historical-record Create/Join/reset; real consent/provider/platform behavior remains deferred. See [final reset review and qualification](../.scratch/ui-driven-node-bootstrap/evidence/42-finalreview.md).

### Historical stopped same-Node filesystem recovery (retained mock provider)

The recorded recovery fixture restored the entire configured IdP, Management, Storage and endpoint roots plus provisioning state to the same paths, preserving tree/bytes/modes and lock inode. Public `LocalProvisioner::open_runtime`, signer verification, endpoint identity, readiness and authenticated owner calls checked two restored reopens. Missing/corrupt roots or identity denied without repair. This was filesystem restore, not a provider export/import, recovery command or populated Storage-content backup.

That fixture retained the same private-key capability in the same mock provider and unchanged configured Client credentials. It does not qualify the new default root's additional `host.json`, credential/TLS state, lost providers, real OS identity changes, signer cloning, network-isolated execution, torn snapshots or crash-consistent backup. For current Nodes preserve all additional retained state described above, including IdP inventories/references/locks and the whole Storage tree; merely restoring `idp.redb` or directory names is insufficient.

No complete executable current-format backup/recovery procedure is verified here. Do not routinely roll back authorization history: even a consistent old snapshot may precede revocations, rotations or resource changes. Missing/corrupt/mismatched endpoint, signer, TLS, intent or required context state refuses existing-only startup without setup replay or identity regeneration. Preserve rejected state for diagnosis and explicit recovery.

Desktop reset affects only local setup/data/identity and may proceed offline; attempt authorized revocation/propagation when possible and preserve retry state on secure-store deletion failure. CLI create/join/serve is not reset tooling. Deselect removes a local copy, not the global resource; deletion synchronizes a Tombstone. Passthrough reads need an authorized reachable Full peer and cannot write.

## Further qualification

Deployment-specific native CA installation/elevation, real provider availability/persistence, process restart, container/proxy/Iroh connectivity, interrupted rotation and current-format backup/recovery need dedicated evidence. Existing historical results do not establish current-tip complete validation. Generic persistence/cleanup guarantees remain with their owners; consumer composition must not infer them from connection admission. Check disk space before image builds; do not prune Docker or delete caches/data without review.

Bundled startup refuses obsolete one-/two-/four-relationship references and Create/Join/native manifests unchanged. No truncation, migration, secret replacement or automatic cleanup is performed. Native explicit reset captures all historical Client IDs and key names, preserves original evidence across cleanup failure, and removes it only after successful cleanup.
