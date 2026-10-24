# Ticket03 report — retained restart and offline-independent Join

## Status, ownership and authorized production exception

**Resolved for the documented same-machine Linux mock-provider topology.** Both
layouts pass retained restart and authorized Join/source-offline reopen. The final
post-integration full execution passes all seven scenarios; the same full suite
also passed earlier, with a separate repeated pair of Join executions between them.
This is actual bounded qualification, not a claim of universal network reliability.

- Worktree: `/home/nathan/code/rust/aicacia/of-system-tests-03`.
- Branch: `refactor/system-tests-03`, based on `a2b9bad75`.
- Read spec, ticket03, ticket02 report, system README and relevant owner contracts;
  loaded `tdd` and `ponytail`, claimed the ticket and used the already approved seams.
- Initial implementation/report commits: `ef8972314`, `66380a6b1`.
- Approved correction and completed independent fixtures: **`b6f9cfcbf`**.
- The initial report correctly stopped at Management's actual missing connection
  contract. The user subsequently **explicitly approved** standalone Management's
  IdP-owned Unix socket connection. The exception is recorded in the spec and ticket;
  no further production feature, security weakening or fallback was added.
- After `b6f9cfcbf`, ran:
  `GIT_EDITOR=true git -C /home/nathan/code/rust/aicacia/of-system-tests-03 merge --no-edit refactor/deployment-independent-system-tests`.
  Result: **Already up to date**, integration tip **`a2b9bad75`**. The final full
  validation below was run after this merge check and before this report.
- Only explicit owned paths committed in this worktree. Main's unrelated staged
  `.scratch/setup-simplification` work was never edited/stashed/staged/committed.
  No merge into integration/main and no push.

## Production correction

`crates/management-server/src/config/app_config.rs` adds:

```yaml
identity_socket: /private/node/idp.setup-host/identity.sock
identity_socket_deadline_ms: 10000
```

The path above is illustrative. Operators select the absolute socket published
by their own IdP owner. **Exactly one** of `identity_database` (existing TCP) or
`identity_socket` is required. Socket deadlines default to 10000 ms and must be
1..=300000 ms. Non-Unix socket selection is unsupported.

`provisioning.rs::identity_client` uses the existing `db::Client::connect_unix`,
bounds connection plus identity schema validation, and applies query deadlines.
It does not discover an alternate connection, retry another selector, open an owner
file, migrate schema, copy credentials or start a substitute SQL owner. TCP
connection behavior is retained. Socket access requires the trusted OS account's
private-directory access and grants SQL domain authority, not OAuth permissions.

`independent_reset.rs` shares selector validation. Stateless reset needs no owner
connection, removes no socket/database, and still refuses unexpected retained
consumer data. `docs/control-plane.md` and `docs/operations.md` document startup,
authority, explicit configuration, deadline and reset behavior. No production
server-to-server dependency or runtime test hook was introduced. A dev-only existing
`idp-db` server feature supports the actual Unix SQL listener in focused unit tests.

Focused tests exercise the public connection/reset seams: valid live identity
schema, no consumer file creation, absent selector, conflicting selectors, invalid
remote URL, missing socket, ordinary file preservation, relative socket, zero/too
large deadline, a stalled listener, wrong identity schema, socket reset without
connection/removal, and retained-evidence refusal.

## Shared scenarios and concrete deployments

`examples/system_test/provider.rs` supplies a durable wrapper around the existing
mock provider substitution: private per-process snapshots, mode-0600 atomic
replacement, file/parent sync, restore, secret replacement and deletion. The
launcher still calls actual production CLI entry points. This is plaintext
**test-only** storage under private roots, not a secure production provider.

`tests/system/create.rs` and `tests/system_create.rs` share assertion bodies across
actual processes rather than reimplementing owners/setup/replication:

- Create and retained restart check public discovery/readiness, fresh ordinary
  password login and User info, Device/Management health, an exact authorized
  Management permission decision, Storage creation/read and anonymous denial.
- Restart preserves commands, configuration, protected provider and domain state;
  it does not resubmit setup. It compares endpoint identity, reads a pre-restart
  resource and exercises another authorized operation.
- Independent remote-SQL Create/restart retains ticket02's actual SQL-owner process.
  The extra `independent_local_owner_create_and_restart` and **both independent
  Join Nodes** instead use each Node's own IdP-owned identity database/socket,
  separate Device owner and Storage state, plus its actual Management CLI process.
- Join enables source acceptance through generated Device operations, denies
  anonymous acceptance/approval, observes a pending non-ready target, and approves
  the exact offer. Authenticated transport endpoint equals requested endpoint;
  possession is not treated as Device approval or resource permission.
- Independent workers use their own protected host codes and correlated tuple.
  All three ready owners must publish matching exact operation nonce, purpose,
  Node, issuer, service bases and nonempty endpoint evidence. No target receives
  source SQL configuration. Existing native/WebSocket protocol clients perform
  public history import into independently retained target owner databases.
- Each side trusts the isolated source/target HTTPS CAs so the operation-bound
  history return callback is verified too; TLS validation is never disabled.
- An ordinary generated source User token exchange selects the target IdP audience.
  After approval/import, generated target IdP operations create its Management
  Application and confidential service Client. The target gets a **new local**
  service credential, not a copy of source credentials. Normal actor/action/target
  permission checks remain in force.
- After completed Join, common owner/resource assertions run. All source children
  are stopped/reaped, the target restarts from its own retained state, and retained
  resource reads, new resource operations, anonymous denial and exact authorized
  Management permission evaluation run again. The target endpoint stays unchanged
  and differs from the source endpoint.
- Local-owner restart waits for IdP's public ready status and live socket listener
  before launching Management. This is process ordering, not a production fallback.

Password/confidential-client verifiers intentionally do not replicate. Joined
resource operations use ordinarily issued source User tokens retained only by the
consumer; no source password verifier, private signer, provider entry or fixture
completion marker is copied to the target. Create/restart continues to test its
own local password credential. No shared assertion opens/seeds a domain database.

## Replacement boundaries

The coverage inventory and system README are updated. No entire old test body is
wholly replaced: API Join tests also exercise rejection/interruption/exact retry,
retirement and finalization; hosted/bootstrap tests include malformed wire,
retained-intent/TLS damage and publication fences; independent composition tests
include reset/recovery; full-node native tests include learned denial, sync and
credential separation. These tests and their still-used private helpers remain.
No obsolete public runtime accessor/helper was introduced or found for removal.

## Final validation

All Cargo commands used the worktree manifest and shared cache, sequentially:

```text
--manifest-path /home/nathan/code/rust/aicacia/of-system-tests-03/Cargo.toml
--target-dir /home/nathan/code/rust/aicacia/of/target
-j8
```

The command suffixes below follow `cargo <subcommand>` with those exact options.
No Cargo job was deliberately run concurrently with another Cargo job by this agent.

| Command | Result |
| --- | --- |
| `build -p idp-unified --example system-test-service -p idp-server --bin identity-database` | Passed, last rebuild 37.87s |
| `test -p management-server --lib` | **16 passed**, final post-merge 18.58s |
| `test -p idp-unified --example system-test-service` | **1 passed**, final post-merge 0.19s |
| `test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture` | **7 passed**, first full current-topology run 389.11s; **7 passed again post-merge, 425.05s** |
| `test -p idp-unified --test system_create join_and_source_offline_reopen -- --ignored --test-threads=1 --nocapture` | **2 passed**, 216.89s, between the full runs |
| `test -p idp-unified --test system_create independent_local_owner_create_and_restart -- --ignored --exact --nocapture` | **1 passed**, 54.29s |
| `test -p idp-unified --test system_create independent_join_and_source_offline_reopen -- --ignored --exact --nocapture` | Passed 128.34s after restart ordering fix; strengthened Management assertion execution passed 158.66s; then both final full runs pass |
| `test -p idp-unified --test system_create unified_join_and_source_offline_reopen -- --ignored --exact --nocapture` | Passed 100.02s; then paired repeat and both final full runs pass |
| `check -p management-server --no-default-features` | Passed, including final check |
| `check -p idp-unified --all-targets` | Passed, including final check |
| `check -p unified-server` | Passed default non-CLI library, including final check |
| `clippy -p management-server --all-targets --no-deps -- -D warnings` | Passed final check |
| `clippy -p idp-unified --test system_create --example system-test-service --no-deps -- -D warnings` | Passed final check |

Also passed:

- `cargo fmt --manifest-path /home/nathan/code/rust/aicacia/of-system-tests-03/Cargo.toml --package management-server --package idp-unified --check`.
- `python3 scripts/check_server_dependencies.py`, with Python's working directory
  explicitly set to this worktree: no direct server-to-server dependency edges.
- `git diff --check` before the implementation commit.

Final full-suite tool deadline was 600s; no final test was skipped or timed out.

## Earlier failures and native-discovery investigation

Earlier failures are evidence, not relabelled as passing:

1. Pre-approval retained restart correctly failed with the in-memory provider.
   Durable provider support fixed that. Provider deletion's first unit run exposed
   mock search retaining absent specifiers; snapshot persistence now excludes only
   `NoEntry` and propagates other provider errors. Its final unit test passes.
2. Pre-approval standalone Management really exited without TCP identity selection.
   That is the precise production gap now fixed under the explicit user exception.
3. First socket positive test was red at compilation: selectors were absent and the
   test-only SQL server feature was not enabled. The implemented selector and
   dev-only test-owner feature made it pass. All 16 Management tests now pass.
4. One early local-owner source Create run stopped `unfinished`, IdP stage
   `installation`, error kind `Other`, after 180.37s (`/tmp/.tmpGwU5qM`). Its exact
   cause was not established; later phase diagnostics were added. Do not infer
   that the socket correction explains it or claim indefinite flake-free operation.
   Current local Create passes individually and in both final full runs.
5. An attempted extra native operator Client registration was denied. It was
   unnecessary: the ordinary existing operator token exchange already supports the
   target audience. No Application permission was widened to accommodate it.
6. Join history initially stopped before import (225.09s, target
   `/tmp/.tmpR46KBq`): the source lacked trust for target HTTPS return callbacks.
   Supplying both isolated Node CAs fixed the fixture trust configuration; no
   history/challenge/approval verification was removed.
7. After that, Join reached target Management and offline restart, but the restarted
   Management exited on transport connection (134.65s, `/tmp/.tmpFlrQ6Y`). Waiting
   for the restored IdP socket before starting its consumer fixed startup ordering.
8. Exact generated permission echo initially distinguished omitted `id` from
   explicit null. The typed request now explicitly sends the expected null field;
   the exact response assertion remains. No permission expectation was relaxed.
9. Strict Clippy first rejected three test `.err().expect()` calls; they were
   corrected to `expect_err`, then both strict Clippy commands passed.
10. Prior six-test full runs were **4 passed, 2 failed** (252.78s and 284.76s),
    before the approved correction. They include the then-real Management blocker
    and production native discovery/admission failures. Their failures are not
    cancelled by later passes. Initial 200s tool-deadline build/test interruptions
    were also not passing validation or orderly teardown certification.

Unified's real CLI uses N0 pinned-EndpointId discovery, not the historical private
memory lookup. Logs from earlier failures show `dns.iroh.link` publication and
relay DNS/probe failures; one previous full run exhausted the existing 90s native
preflight. The fixture uses the existing IdP native issuer-discovery client with
that bounded non-mutating readiness preflight; it still runs actual CLI discovery,
Join, approval and authorization. It does not inject an address lookup, alter
production presets, disable trust, or retry an admitted mutation to hide a failure.

During this continuation, OS resolution for `dns.iroh.link` and the relay (with and
without trailing dot) succeeded; direct HTTPS to `https://dns.iroh.link/` returned
200; a direct UDP A query through the configured system resolver `127.0.0.53:53`
succeeded. These probes establish availability at that moment only. They do not
explain all previous intermittent failures or prove discovery reliability offline,
under failing DNS, or across arbitrary networks. **Production N0 discovery/relay
network availability remains a genuine prerequisite**, even for this same-machine
Unified fixture. Later isolated, paired, and two full runs passed without a
network/security bypass. The exact green final commands are reported above, not
an unconditional guarantee that an unreliable environment is green.

## Qualification limits / remaining gaps

No known unimplemented ticket03 acceptance remains in the tested topology. Both
actual layouts reopen retained Nodes and complete authorized Join/source-offline
reopen with independently owned target state. Remaining qualifications are limits,
not silently weakened assertions: external N0 networking availability/reliability,
the unexplained earlier local-Create transient, real OS provider/trust installation,
default stacks, production password costs, proxy/mTLS, WAN/container, other OSes,
gracious shutdown, recovery/synchronization suites and broad platform/load coverage.

Processes retain two workers and 64 MiB stacks, 3s connect/30s HTTP limits, 60s
pending/reopen polls, 180s setup completion and 90s native readiness. Logs are drained
with 16 KiB retained per child incarnation; secrets/responses are not printed.
Owned children are killed/reaped before root cleanup; failed roots remain private.
A forcibly terminated runner is not graceful teardown qualification.

No CRAP, coverage or feature-powerset commands were run. No full-workspace,
OS-provider, performance or always-offline discovery certification is claimed.
