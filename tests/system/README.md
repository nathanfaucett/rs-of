# Shared Node lifecycle and filesystem qualification

Run on Linux from the repository root, with the sibling `ofdb`, `ofnet`, and
`offs` checkouts required by the workspace:

```sh
cargo build -j 8 -p idp-unified --example system-test-service -p idp-server --bin identity-database
cargo test -j 8 -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture
```

Use the same Cargo target directory/profile for both commands. All nine expensive,
process-launching scenarios are opt-in and must run serially. They are not silent
skips counted as qualification. The ordinary `create::root_is_private` regression
runs without `--ignored`.

## One suite, two production layouts

The `*_create_and_use`, `*_retained_restart`,
`*_join_and_source_offline_reopen`, and `*_permission_resource_denial` pairs run
the same scenario bodies for Unified and multi-process Nodes. The ninth scenario,
`independent_local_owner_create_and_restart`, additionally checks local independent
owners instead of the explicit remote-SQL Create fixture.

- **Unified:** one child calls the production Unified CLI `run`, using its actual
  composed HTTPS endpoint and exact `/idp`, `/device`, `/storage`, `/management`
  prefixes. The fixture prepares only local TLS/admission prerequisites, then
  supplies ordinary configuration to the CLI. One CA protects all three owners;
  three separate capabilities admit their respective operations. The canonical
  issuer is the exact IdP base. No old aggregate host setup API is used.
- **Multi-process:** production IdP, Device, Storage, and Management CLI entry
  points run in separate children. IdP has its own origin with no prefix, Device
  uses `/device`, Storage `/storage`, and Management `/policy`, on separately
  allocated loopback ports. Create/restart also covers an explicit remote
  `identity-database` process. The additional local-owner fixture and both sides
  of Join use independent owner databases and the published local IdP socket,
  not a shared source SQL connection.

`create::owner_create` admits the same correlated Create operation to all three
owners, sending the password only to IdP and exactly one capability per request.
It checks exact retries, conflicting retries (409), current owner-specific
readiness, public exact operations, matching endpoint evidence and the published
login Client. Management is not a fourth setup owner: Unified composes it from
owner configuration; the existing independent fixture registers its local service
relationship through generated IdP operations and starts its configured production
CLI. Both layouts must pass the same ordinary authorized Management decision.
There is no extra operator interaction or deployment setup broker.

`Node::restart` reuses retained commands, configuration, roots, and private
providers without new setup requests. It checks all three owners current
readiness and preservation of each exact operation and endpoint. Independent
Management startup waits for its configured IdP socket as well as public readiness.

## Filesystem seam

`create::use_node` drives generated discovery, login/User info, Device health,
Management authorization, and Storage resource creation/read/anonymous denial.
`filesystem.rs` extends that same body in both layouts:

- Generated Storage clients create and read a filesystem in the authorized namespace.
- Typed `storage-model` requests and responses write and read exact file bytes over
  the production Storage WebSocket. TLS verifies only the isolated fixture CAs.
- Generated Residency operations switch an empty disposable resource to Passthrough
  and then Full; actual protocol writes must fail and then succeed respectively.
  The resource is empty before Passthrough, so restoring Full needs no remote
  content provider. Repopulating evicted content is not a single-node assertion.
- Protocol deletion makes a file unreadable and absent from directory listings.
- Generated resource deletion makes a populated disposable filesystem return 404
  and disappear from the catalog. Its ID remains in the runner for absence checks
  after retained restart, without querying any database or journal.
- Before creating any replacement content after restart, the retained resource
  must return the original bytes and keep its deleted file absent.

The same verified connection builder is reused by the retained-socket denial
scenario. HTTP operations use generated clients/models only; there are no raw HTTP
fallbacks. No `offs` source or tests are changed, and no dependency is added.

## Join, denial, and retained recovery

Both layouts use generated owner-local Join admission and generated Device owner
acceptance/pending/approval/rejection. Target listener availability is checked
before admission; setup admission is not readiness. The scenario checks exact
pending retries, capability-protected cancellation, interrupted process reopen,
retained exact intent, source-offer release, rejected stale-offer denial (409 on
the current owner contract), and eventual exact approval. Deleted host-broker
routes and their historical 404 responses are not compatibility surfaces.

Completed owners publish the same Join operation and a target endpoint distinct
from the source. Imported consumed/revoked grants remain denied, while an
unconsumed grant succeeds, rotates and rejects replay. The source is then stopped
and the target reopens from its own retained state; filesystem bytes/tombstones,
Management authorization and learned-grant denial are checked again.
Source-issued tokens stay in the runner, not the target credential store.
Joined password login is not asserted because Join does not import password
verifiers or provision a new local password.

`authorization.rs` retains actor/Application substitution, wrong audience,
ungranted Device action, read-only delete, wrong resource-kind denial, and local
User-deletion enforcement on an already-open Storage connection and reconnect.
These cases delete their disposable administrator last.

## Safety and limits

Roots are private isolated temporary directories. The launcher uses the existing
mock credential provider with a per-process durable mode-0600 test snapshot,
never a real secure store. Mock `trust`/`pkexec` prevent OS trust installation.
Credential environment variables are removed before explicit fixture settings.
Service processes use two runtime workers and 64 MiB stacks. The small IdP
password cost is not production password-cost qualification.

HTTP connect/request limits and explicit startup/readiness/recovery/protocol
limits bound waits. Owned children are killed and reaped before successful root
cleanup. Failure preserves private diagnostics, not tokens or capabilities in the
console. Do not publish the private fixture/provider files. No unrelated process
is signalled.

This qualifies same-machine Linux production entry points with mock providers,
not WAN/default-stack/real OS trust or remote content fetching. It does not claim
cross-node filesystem synchronization or internal synchronization/admission/
retirement coverage: the unchanged `offs` in-process suite owns those internals.
No public process trigger forces post-Join security reconciliation. The focused
Unified live test is separate from this production-process qualification.

See `.scratch/unified-owner-lifecycle/ticket03-validation.md` for exact commands,
red/green evidence, passed scenarios, and broader validation limits. In particular,
ticket02 timeout results are not retrospectively claimed as runtime green.
