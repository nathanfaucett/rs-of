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
`independent_join_and_source_offline_reopen`.

**Ticket03 is blocked, not complete.** The independent Join execution currently
fails in the actual standalone Management CLI during local-owner source startup:
`standalone Management requires identity_database; local composition injects the identity client`.
The local-owner IdP publishes a Unix socket, while standalone Management requires
TCP identity SQL. Its local-owner connection and target Management launch remain
unqualified. Do not interpret this failing execution as independent Join coverage.
The full six-test invocation is expected to fail until that contract gap is fixed.
See `.scratch/deployment-independent-system-tests/ticket03-report.md`.

## Public seam and concrete deployments

`create::scenario` is the single assertion body. It checks generated-client
Installation discovery, ordinary Initial Administrator login and User info,
Device/Management database health, authorized Storage filesystem creation/read,
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
  `identity-database` executable is a fifth process. Its database is outside all
  consumer roots; IdP/Device/Management use its explicit loopback SQL connection.
  Device owns its separate local Device database. Storage receives no SQL access.
  IdP, Device, and Storage each admit the correlated Create tuple with their own
  protected host code. Only IdP receives the password. Completion requires each
  current public readiness response and matching public operation, issuer,
  service bases, and nonempty endpoint evidence, not just an accepted request.
  Management is not a setup worker: an ordinarily authorized administrator
  registers its Application and confidential service Client through generated
  IdP operations, then starts the ordinary Management CLI with that relationship.

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
recovery, synchronization, or full-suite certification. Unified approved Join and
source-offline reopen are qualified separately below; independent Join is blocked. Existing Unified tests
remain owned by ticket01; do not delete their Create coverage based only on this
narrow replacement. See the ticket02 report for exact validation and gaps.

## Authorized Join and offline independence

The shared Join scenario enables source acceptance through its generated Device
client, denies anonymous acceptance/approval, observes a pending non-ready target,
and approves the exact pending offer. Unified approval must return the same public
request observed in the pending list. The target discovers the same Installation,
uses an ordinarily issued source resource token, stops all source processes,
restarts its own retained processes, and reads its retained resource with the same
endpoint identity. A new resource operation and anonymous denial also run after
reopen. Source-issued tokens are held only in the test runner, not installed as
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
connection is given to the joining target. The currently failing source Management
launch prevents qualification of the remaining target Join and offline assertions.
No old recovery, private, rejected-Join, or layout-specific test has been deleted.
