# Unified Node bootstrap

Ordinary bootstrap needs **no configuration file**:

```sh
unified-server create
unified-server serve
# On another machine:
unified-server join
unified-server serve
```

`create` defaults to administrator `operator` and Node name `Local Node`, and
prompts for a hidden initial password with confirmation. Partial create retry
prompts for the same password; completed create re-verifies the retained Node
without requiring the password again. `--administrator` and `--node` are optional
first-attempt overrides. `--password-file` is optional automation input, not a
requirement; it must be an operator-only nonsymlink regular file containing one
nonempty UTF-8 line. Passwords never enter argv, config or raw-secret manifests.
The obsolete `provision` command has been removed. For a different initial username,
use `unified-server create --administrator alice` and enter the password at the
hidden prompt. Optional automation instead uses
`unified-server create --administrator alice --password-file ./protected-initial-password`.
These are alternative first attempts; continuation omits name overrides.

The default Node root is `$XDG_DATA_HOME/of/unified`, or
`$HOME/.local/share/of/unified`. `--data-dir` selects an alternative Node root.
The host chooses an unused localhost port **once**, durably retains it in
`host.json`, and derives separate IdP/Management/Storage/endpoint directories,
public Application URIs, service audiences, Canonical Issuer and a distinct
local key namespace. A new Installation's Canonical Issuer is
`https://<retained-random-UUID>.installation.invalid/idp`: a unique retained logical
identifier, not the loopback address. The actual public HTTP API remains on the
saved local HTTPS listener. Two machines using the same localhost port therefore
have different Installation identities. Join retains the authenticated peer's
issuer, independent of its own API address. Restart/continuation uses exactly those retained settings;
port contention refuses rather than selecting a new identity. `host.json`
contains only public identity/settings, never passwords, relationship secrets,
private keys or tokens. Missing or malformed retained host identity requires
explicit recovery, not regeneration.

Bundled IdP, Management and Storage use current owner-local authorization calls.
No confidential relationship Clients, generated relationship secrets, or secret
environment variables are required. The public operator-login Client remains;
Node private signers and TLS material still require protected retained storage.

## Public operator login

Create **and** join automatically provision a separate public operator-login Client
with a retained ID and no Client secret. Completed retry/reopen verifies and reuses
it. CLI output prints `Operator login Client`, `Login scopes` and
`Token-exchange audiences`; `serve` also prints the local HTTPS API bases, issuer,
IdP token endpoint and service audiences. Use the actual printed login Client ID,
not a manual UUID or confidential service-client ID.

Sign in through ordinary `POST /idp/oauth2/token` at the printed local HTTPS API,
using `application/x-www-form-urlencoded`: `grant_type=password`, the printed
`client_id`, the User's `username` (e.g. `operator` or chosen `alice`), securely
handled `password`, and `scope=openid` (optionally `profile email`). The public
Client's token endpoint authentication is `none`; no Client secret or Basic header
is needed. Use ordinary token exchange for the required printed audience/action
scopes before source acceptance/approval. The audience allowlist does not grant
User permission. Join uses an existing Installation User/password, not a newly
invented administrator password. Keep passwords out of argv/config/logs; the
bootstrap password file is optional delivery, not a sign-in prerequisite. See
[login operations](../../docs/operations.md#public-operator-login).

## Join identity and approval

`join` prompts for the trusted peer endpoint fingerprint, or accepts `--peer` for
automation. Select/verify that fingerprint through a trusted channel: discovering
an address does not establish which Installation should be trusted. This trust
choice and the initial human password cannot safely be invented by bootstrap.

Before producing Node signer/service material, no-config join learns the peer's
Canonical Issuer over the existing authenticated pairing transport. The
`of.equal-node-issuer-discovery.v1` request/response binds purpose, fresh nonce and
the pinned endpoint. The serving background task answers issuer probes while
authenticated acceptance is enabled and current public Node state is ready; no
pending-list polling or approval UI invocation is needed for discovery.
This reveals public Installation identity; it does **not** enroll a Node. The
peer/issuer are retained before actual Node production. No unsigned HTTP/DNS
metadata or arbitrary discovered peer is treated as trust evidence.

The source must run the updated `UnifiedRuntime` with authenticated acceptance
enabled. The production composed HTTPS router exposes these owner IdP routes:

| Route                               | Operation                                       |
| ----------------------------------- | ----------------------------------------------- |
| `GET /idp/node-joins/acceptance`    | Read authenticated acceptance state             |
| `PUT /idp/node-joins/acceptance`    | Set acceptance with `{"accepting": true/false}` |
| `GET /idp/node-joins/pending`       | List authenticated pending Node offers          |
| `POST /idp/node-joins/{id}/approve` | Explicitly approve an exact offer               |
| `POST /idp/node-joins/{id}/reject`  | Explicitly reject an exact offer                |

All routes require an ordinary bearer session and reuse IdP's existing
`authorize_node_join_approver` permission checks. There is no new password/API-key
auth mechanism and no automatic enrollment. Discovery is public identity exchange,
not Node approval; actual Node offers stay pending until explicitly approved.

Approval first validates and commits the exact IdP Device/member/signer binding,
then verifies current enrollment ownership through IdP owner APIs. Only after
successful enrollment does the host initialize the default empty Management
policy, preserving existing restrictions/selections. The host refreshes admission
and the IdP rechecks authorization and exact enrollment before publishing the
signed pairing reply. Rejected IdP enrollment does not create Management policy.
No operator repository call is required and the joining host does not manufacture
policy. Management/reply failure after successful IdP enrollment remains a partial
operation for exact retry, not automatic rollback or identity replacement.

Transport admission is refreshed from learned trusted signer membership after
IdP import. Signed-approval/bootstrap races use bounded handshake/admission
acknowledgment retry, rechecking the exact anchor, private binding and learned
context grants. Bulk public state transfer is not given the short handshake
budget. No temporary general allowlist bypass or resource permission is added.

## Mandatory TLS and completion

Both create and join require OS CA installation and protected retained localhost
CA/server material. Only `127.0.0.1` and `::1` are supported for localhost TLS.
Node bootstrap is explicitly Unix-only; unsupported platforms/IPs refuse before
creating retained material.

The secure base is configured before production runtime construction. Completion
checks current owner-local readiness and serves the **production HTTPS router**
for native-root listener verification. Full integration qualification separately
exercises all three health routes and current local Management→IdP identity
lookup. Bundled Management and Storage bearer validation and endpoint identity
lookup use local IdP owner callbacks; IdP permissions and Storage replication
use local Management authorization. These local consumers require no confidential Clients. Owners and listener are closed before completion is
published. Verification uses native roots only, no redirects or proxies. There is
no plaintext/browser-only fallback or skip-install flag.

Linux uses `trust anchor`, or the matched platform destination/refresh command:
Debian-family `/usr/local/share/ca-certificates` with `update-ca-certificates`, or
Fedora-family `/etc/pki/ca-trust/source/anchors` with `update-ca-trust extract`.
macOS uses the login keychain. Real OS provider availability, elevation and trust
verification must work in the deployment; no unattended-keyring or universal
crash-safety claim is made.

## Retention and explicit compositions

Beside the endpoint root, retain `.credentials`, `.tls` and `.provisioning`
directories. The historically named `.credentials/credentials.json` retains exact host settings
and TLS-attempt evidence only, not service Client inventories or secrets. TLS
production history is durable before protected material is created, and writes
require exact readback. Missing attempted signing material,
endpoint/signing material, partial TLS, malformed intent, or unacknowledged
Storage initialization require recovery rather than replacement. Manifests
retain only public metadata, references, one-way verifiers and completion phases.
Cooperating operations hold retained locks through setup/serving. Ancestry must be
stable, operator-controlled and nonsymlink; these checks are not hostile concurrent
pathname-race protection.

`--config config/unified/example.yaml` remains an optional explicit composition.
Bundled `service_clients` configuration and relationship-secret environment
variables are no longer supported; an obsolete `service_clients` section is
rejected. Independent Management/Storage/IdP server commands retain their own
explicit authenticated HTTPS relationship configuration.

Old one-, two- and four-relationship credential references, Create/Join intents and native setup
manifests are not migrated or truncated: ordinary open/retry refuses them unchanged.
Keep those records and attempted-production evidence for explicit recovery/reset;
never delete them to make a host appear fresh. Native reset accepts the original
one-/two-/four-relationship references solely for exact cleanup, retains every original
secret name across provider
failure, and removes owner state only after verified secret deletion. Unified
credential records likewise remain intact for independently verified cleanup;
this change adds no automatic cleanup or replacement path. IdP, Management, Storage
and endpoint roots remain independent; their count is unrelated to Client count.

## Regression scope and qualification limits

The HTTPS regression harness supplies `RUST_MIN_STACK=16777216` to its child test
process. It uses mock keyrings, local in-memory Iroh lookup, test-only install
commands and isolated native CA-file trust, never real OS-store mutation. Tests
exercise authenticated issuer discovery, actual source approval/default policy,
production join, source shutdown, offline reopen, authenticated Storage and
incomplete/refusal boundaries. Default config, secure in-memory password input,
CLI parsing and purpose/nonce/peer validation also have tests. The regression now
runs the actual `serve_https` task and exercises acceptance, pending, approval and
rejection through HTTPS rather than direct runtime method calls. Discovery must
complete before any pending-list request. An authorized malformed-signer offer
must fail enrollment without policy or transport admission; an explicit HTTP
approval must then complete a legitimate join. A same-port/two-Installation test
checks distinct retained issuers.

Supplied focused validation reports 12 new unit tests passed and the production
final HTTPS regression passed in 296.18 seconds under the unchanged 300-second child
bound and narrow 5-second HTTP timeout. It covers no-config source/create,
retained public login/password authentication, authorized acceptance/rejection,
stale/malformed offer denial and policy preservation, real join, offline reopen,
authenticated Storage and completed retry. Earlier baseline stack overflow failed;
later admission and rejected-enrollment timeout failures were fixed before the pass.
Desktop localhost validation passed 17 tests in 5.33 seconds, and three issuer/discovery tests passed. The deterministic reply/invalidation race test and library Clippy with and without `cli` passed; strict all-targets Clippy encountered existing fixture diagnostics. `ofnet`
generation and pairing-reply-closure tests also passed. Frontend Vitest was blocked
by an absent dependency and Svelte checking reported five unrelated Storage client
export errors; neither is a frontend/GUI pass.

No native OS CA installation was run. The HTTPS harness's injected commands and isolated native CA-file
roots modify no real OS trust store and do not qualify native installation.
[Node operations](../../docs/operations.md#qualification-boundary) records the
bounded Ubuntu scope and deferred GUI/platform limits.

An approval UI is optional client work, not a serving/API blocker: the production
HTTPS owner routes above are available to existing authenticated clients. Desktop
can continue using the corresponding runtime methods. The expanded owner seam is
in `idp-server/src/router/node_join_approval.rs` and `idp-server/src/runtime.rs`;
all permission checks remain with the IdP. Earlier address-based default host
intents are not silently migrated or renamed to a new Installation; incompatible
state is preserved for explicit recovery.
An older source without the purpose-bound issuer response cannot support no-config
join: it must be updated, or an explicit composition must already supply the
trusted issuer. Do not substitute unauthenticated metadata for that evidence.
See [Node operations](../../docs/operations.md) for the stable runbook and Desktop
boundary. Desktop fresh create automatically retains a unique logical issuer;
join takes a pinned peer and Node name and uses the shared authenticated issuer
discovery helper. Manual issuer UI/input is removed. Exact retry reuses retained
intent without silently replacing existing identity. Native reset/recovery remains
its own host boundary; shared discovery does not imply identical CLI interfaces or
GUI acceptance. Deployment packaging, certificate renewal, CLI reset/recovery
tooling, real OS-provider qualification and Windows ACL support remain separate.
