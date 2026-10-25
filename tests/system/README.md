# Shared Node lifecycle qualification

Run from the repository root on Linux, with the sibling `ofdb`, `ofnet`, and
`offs` checkouts required by the workspace:

```sh
cargo build -p idp-unified --example system-test-service -p idp-server --bin identity-database
cargo test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture
```

Use the same Cargo target directory/profile for both commands. These expensive,
process-launching acceptance tests are explicitly opt-in, not silently skipped
qualification. Each test can also be selected by its exact name:
`unified_create_and_use`, `independent_create_and_use`,
`unified_retained_restart`, `independent_retained_restart`,
`unified_join_and_source_offline_reopen`, or
`independent_join_and_source_offline_reopen`, or
`independent_local_owner_create_and_restart`, `unified_permission_resource_denial`,
or `independent_permission_resource_denial`.

Ticket03's local-owner Management connection is explicitly selected through the
approved production `identity_socket` option. The nine-test suite covers targeted permission/resource denial, retained
restart and approved Join/source-offline reopen in both layouts, with a separate
local-owner Create/restart execution. See
`.scratch/deployment-independent-system-tests/ticket03-report.md` for exact
validation and earlier failures. Native networking prerequisites below are real;
no offline discovery reliability or external network availability is promised.

## Public seam and concrete deployments

`create::use_node` holds the common owner/resource assertions. It checks generated-client
Installation discovery, ordinary Initial Administrator login and User info,
Device/Management database health, an authorized exact Management permission decision,
authorized Storage filesystem creation/read,
and anonymous denial of that same resource. It never opens a domain database.
All HTTP operations use generated clients and models; there are no raw HTTP
fallbacks. The common independent setup contract is consumed through its
generated IdP client, including when addressing Device and Storage workers.
Ordinary owner operations use the respective generated owner clients.

- **Unified:** one child calls the production Unified CLI `run`, starts a fresh
  neutral loopback HTTP host, admits Create through its protected host code,
  polls public current readiness, and verifies the exact completed Create to
  obtain its public operator-login Client. TLS termination at a deployment proxy
  is outside this loopback CLI qualification.
- **Independent:** actual production CLI entry points run in separate OS
  processes for IdP, Device, Storage, and Management. The actual
  `identity-database` executable is a fifth process in the explicit remote-SQL Create/restart fixture. Its database is outside all
  consumer roots; IdP/Device/Management use its explicit loopback SQL connection.
  Device owns its separate local Device database. Storage receives no SQL access.
  IdP, Device, and Storage each admit the correlated Create tuple with their own
  protected host code. Only IdP receives the password. Completion requires each
  current public readiness response and matching public operation, issuer,
  service bases, and nonempty endpoint evidence, not just an accepted request.
  Management is not a setup worker: an ordinarily authorized administrator
  registers its Application and confidential service Client through generated
  IdP operations, then starts the ordinary Management CLI with that relationship.

- **Independent local owners:** the additional local Create/restart execution and
  both sides of independent Join run IdP, Device, Storage and Management as four
  actual CLI processes per Node. IdP owns its identity database and socket; Device
  owns a separate Device database and rediscovers its own IdP socket. Management's
  configuration selects its own Node's published socket explicitly and receives a
  separately registered local service Client. Restart waits for IdP's public ready
  status and socket listener before launching Management. Source and target have
  independent roots, private providers and owner databases; neither target
  consumer receives a source SQL connection.

Every address is explicit and uses a separately allocated ephemeral loopback
port in the independent layout. IdP uses no prefix, Device uses `/device`,
Storage uses `/storage`, and Management uses `/policy`; there is no common-origin
or common-prefix derivation. Independent setup supports only empty or owner
composition prefixes; Storage's resource router uses `/storage`. Independent
Create's Canonical Issuer is its exact IdP base under the supported contract;
Unified retains its separately generated logical Installation issuer.

## Safety and limits

The test-only example installs the existing mock credential store (ignoring
provider-specific entry modifiers, as existing provider fixtures do), then calls
production service entry points. It does not construct replacement routers,
provision domain state, or add production test hooks. Each process has its own
mock provider persisted at a private fixture path. The test-only provider snapshots
mock entries on every secret update/deletion, atomically renames a mode-0600 file,
and syncs it and its parent before returning. It reloads only that process's own
entries at startup; source credentials are never copied to another Node. This is
plaintext test-only storage under a private temporary root, not a production secure
store. Retained restart reuses the actual command, configuration, state and provider;
it makes no fresh setup requests. It requires the same public endpoint identity,
reads a resource created before restart, and verifies fresh ordinary authorized use.
Workers use verified HTTPS and an explicit isolated CA bundle, also supplied as
native trust to child processes. Test-only `trust`/`pkexec` commands prevent real
OS trust installation. Management's production CLI exposes loopback HTTP; no
external TLS proxy is introduced or qualified here. The SQL listener is also
literal-loopback plaintext, as allowed by its production CLI.

Roots are private and ephemeral; configuration credential environment variables
are removed before explicit fixture settings are applied. There are bounded
connect/request/poll deadlines, two runtime workers per service, and explicit
64 MiB service stacks. Independent IdP uses the existing small test password
configuration (1 MiB, one iteration), not production password-cost qualification. Child stderr is drained while retaining at most 16 KiB per
service in the private root; stdout and stdin are disconnected. Tokens, password
responses, Client registration responses and setup capabilities are never
printed. Test failure retains the private root and reports only its path. Success
removes it. Owned children are killed and reaped before their roots are removed,
including unwinding. No unrelated process is signalled. A forcibly killed test
runner is not an orderly teardown test.

This is same-machine Linux mock-provider qualification, not real OS-provider or
trust installation, default-stack, proxy/mTLS, WAN/container,
general recovery, post-Join peer reconciliation, or full-suite certification. Unified approved Join and
source-offline reopen are qualified separately below in both layouts. Historical
mixed recovery/private tests remain because their corruption, rejection, retry,
publication and reset assertions are not wholly replaced. See the coverage map
and ticket03 report for exact replacement boundaries and validation.

## Authorized Join and offline independence

The shared Join scenario enables source acceptance through its generated Device
client, denies anonymous acceptance/approval, observes a pending non-ready target,
and approves the exact pending offer. Unified approval must return the same public
request observed in the pending list. The target discovers the same Installation,
uses an ordinarily issued source resource token, stops all source processes,
restarts its own retained processes, and reads its retained resource with the same
endpoint identity. A new resource operation, anonymous denial and authorized Management permission
evaluation also run after reopen. Source-issued tokens are held only in the test runner, not installed as
source credentials in the target.

Public security history deliberately excludes password and confidential-client
verifiers. Joined password login is not asserted: provisioning a fresh local password
would be a different operation, not a property of Join. Create/restart continues to
verify fresh password login and User info against each Node's own retained credentials.

Unified uses the production pinned-EndpointId native protocol, including the
existing IdP native issuer-discovery client as a bounded source readiness preflight.
The actual CLI uses N0 discovery; unlike the historical in-process tests' memory
lookup, this needs functioning DNS/iroh discovery/relay networking even on one
machine. Native preflight is bounded at 90 seconds; public HTTP admission is bounded
at 30 seconds, and pending/completion polling at 60/180 seconds. A successful
preflight is not Node authorization: ordinary Device approval is still required.
Network discovery failures are not bypassed with a router or injected lookup.

Independent Join selects local-owner source/target state, not the remote-SQL
attachment used by the Create fixture. Three workers have separate protected
capabilities, roots, explicit addresses and isolated HTTPS trust. No source SQL
connection is given to the joining target. Before history exchange, each side's
isolated native CA bundle trusts both Nodes so the operation-bound history callback
is also verified HTTPS. Generated IdP token exchange obtains a User token for the
target IdP audience; after approved history import the target registers its own
Management Application and confidential Client through generated operations.
This creates a target-local credential, not a copy of a source credential.
All three ready owners must publish the same exact Join operation, issuer,
service bases and nonempty endpoint evidence. The pending offer's authenticated
transport endpoint must match its requested endpoint, and the completed target
endpoint must differ from the source endpoint. Ticket04 replaces the public rejection/retry assertions from the old API Join
interruption case; its private host-purpose/peer, retained TLS/admission and
internal offer-lifetime assertions remain under
`production_api_join_interruption_preserves_private_host_intent`. Other private
recovery and layout-specific tests remain.


## Ticket04: denial, learned history and interrupted Join

The new `*_permission_resource_denial` entries share `authorization.rs`. Generated
owner clients verify the exact actor, caller audit, action and specific Device
target, denied actor/Application substitution, wrong audience, an ungranted
restriction action, read-only delete denial, and a database-kind token denied on a
filesystem. An authorized read after denial verifies that the resource is still
present and unchanged. Existing `tokio-tungstenite` and `storage-model` protocol
clients write/read bytes over the actual Storage WebSocket. After generated IdP
User deletion, a subsequent write on that **same connection** must fail or close;
reconnect and catalog access must return 401 while another owner's health remains
available. Independent WebSockets verify the fixture CA explicitly, not an
accept-all connector. These cases delete their disposable administrator last.

The existing two `*_join_and_source_offline_reopen` entries now also exercise
`recovery.rs`:

- Source refresh consumption/revocation through generated OAuth clients is learned
  during real native/WebSocket Join history import. Both must return
  `invalid_grant` on the target. An unconsumed imported grant succeeds, rotates and
  then rejects replay, distinguishing denial from missing history or an outage.
  Denial survives target reopen with every source process stopped.
- Repeating the exact active admission preserves the pending request. Protected
  cancellation refuses absent capabilities. The scenario then stops/reaps target
  processes, restarts their retained commands, compares the recovered exact public
  intent, checks non-readiness and observes source offer release. A cancellation
  response alone is **not** asserted to mean cleanup completion or rollback.
- Exact retry retains the public Node, signer and endpoint request. Anonymous
  Device rejection is denied; ordinary authorized rejection cannot complete the
  target. The rejected offer is no longer approvable. A further exact retry keeps
  the request and completes only after ordinary approval.
- Independent setup has three protected worker operations. When Device rejection
  leaves IdP/Storage waiting, explicitly cancel those workers and reopen the
  retained processes before retry. Independent stale offers return documented
  409; Unified returns 404. These are lifecycle/contract differences, not weakened
  membership assertions.

Only these existing behavioral families are qualified. Post-Join live peer
reconciliation still requires the in-process owner `SecurityProtocol`, local
member identity and owner databases; no public process trigger exists. Its old
consumption/revocation and learned Device restriction tests remain, including
existing-connection enforcement. This suite does not read their keys/databases or
add a production/test-control endpoint to force reconciliation. See
`.scratch/deployment-independent-system-tests/ticket04-report.md` for the concrete
inventory, replacements and exact validation history; ticket01's coverage entries
are not edited by ticket04.

## Final audit and retained live reconciliation

See `.scratch/deployment-independent-system-tests/coverage.md` for every original
scenario disposition and the raw-HTTP/hook audit, and `ticket05-report.md` in that
directory for exact final validation and earlier failures. Ticket05 ran **all nine**
actual-process entries serially; this does not expand their documented topology.

Live post-Join consumed/revoked grant reconciliation and learned **Device**
restriction denial on retained sockets remain enabled in Unified's private mixed
`full_node_tests`. They need live owner databases/member context and the native
`SecurityProtocol::synchronize`; the process interface has no exact trigger.
Initial Join history and local User deletion are **not** substitutes. The two
retained tests can be run without the third full-node scenario:

```sh
RUST_MIN_STACK=67108864 cargo test -p unified-server --lib two_completed_full_hosts_ -- --test-threads=1 --skip two_completed_full_hosts_join_partition_reopen_and_learn_denial --nocapture
```

Use the same shared target and `-j8` convention as the process suite. A default
small test-thread stack aborted during native import in ticket05; both focused
live cases subsequently passed with the explicit 64 MiB stack. This is not
actual-process cross-layout live reconciliation or default-stack qualification.
