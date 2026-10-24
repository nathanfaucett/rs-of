# Ticket03 report — partial implementation, blocked qualification

## Ownership, status and integration

- Worktree: `/home/nathan/code/rust/aicacia/of-system-tests-03`.
- Branch: `refactor/system-tests-03`; starting HEAD `a2b9bad75`.
- Implementation commit: `ef8972314` (`test(system): retain providers and qualify restart and Unified Join`).
- Loaded `tdd` and `ponytail`; read the spec, ticket03, ticket02 report and system README.
  The spec already approves the generated owner HTTP/native protocol seam.
- Claimed ticket03 before implementation. **Ticket remains claimed, not resolved.**
- After committing owned paths, ran
  `GIT_EDITOR=true git -C /home/nathan/code/rust/aicacia/of-system-tests-03 merge --no-edit refactor/deployment-independent-system-tests`.
  Result: **Already up to date**; current integration tip was `a2b9bad75`.
- No integration/main merge or push. No main-checkout files were edited, staged,
  stashed or committed, including `.scratch/setup-simplification`. Shared Cargo
  build artifacts were used as authorized. Sibling source trees were not edited.

## Changes and actual evidence

| File | Change / qualification |
| --- | --- |
| `examples/system_test/provider.rs` | Durable wrapper around the existing mock credential store substitution. Per-process private snapshot path, atomic mode-0600 replacement, file/parent sync, restore on launcher startup. Secret replacement and deletion included. |
| `examples/system-test-service.rs` | Installs that test-only provider, then calls unchanged actual production service CLI entry points. |
| `tests/system/create.rs` | Retains concrete launch commands, kills/reaps owned children, relaunches the same configurations/roots/providers, waits for the SQL owner before consumers, checks public readiness, endpoint identity, retained resource access and fresh ordinary authorized operations. Shared owner assertions remain outside the server implementation. |
| `tests/system/create.rs` | Shared Join assertion body and concrete Unified/local-owner independent setup branches. Authorized generated Device acceptance, anonymous denial, pending/non-ready target, exact authorized approval, correlated Unified issuer, resource operation, complete source process shutdown and retained target restart. Independent continuation is **not qualified** because its source Management startup fails first. |
| `tests/system_create.rs` | Explicit opt-in layout-named Create, retained restart and approved-Join/source-offline-reopen executions. The failing independent execution is not silently skipped or marked successful. |
| `tests/system/README.md` | Commands, durable mock-provider limits, credential-locality rules, native network dependency and explicit blocked independent status. |

Both layouts' retained-restart executions pass, including reading a resource
created before the restart, comparing the public endpoint identity, logging in
again with the Node's own retained password credential, and performing another
ordinary Storage operation with anonymous denial. No fresh setup is replayed.

Unified approved Join/source-offline reopen passed in isolation, including the
strengthened retained-resource and endpoint assertions. The same shared owner
assertions are used before and after target restart. Source-issued Storage tokens
are held by the consumer, not imported as target credentials. The runner stops
all source processes before restarting the target. Native issuer discovery uses
the existing public `idp_server::discover_installation_issuer` protocol client;
the actual setup/Join path is still executed by the actual production CLI.

No production code, features, contracts, generated output or dependencies changed.
No SQL reads/seeds, fixture completion markers, copied source credentials,
replacement routers, signer injection or shared source SQL for the joining target.
No old private/recovery/Join tests were deleted: ticket03 is not a complete replacement.

## Confirmed production connection gap

The independent Join execution selects local owners for its source and target,
not the remote-SQL attachment topology used by ticket02's fresh Create scenario.
Local source IdP/Device/Storage complete Create through their generated setup
clients. Ordinary source login, Application registration and confidential
Management service-Client registration also complete through generated IdP clients.
Then the **actual Management CLI** exits with:

```text
Error: Custom { kind: InvalidInput, error: "standalone Management requires identity_database; local composition injects the identity client" }
```

Final reproduction diagnostics: `/tmp/.tmpybovaI/management.log`.
The test retains the private root and fails on the observed child exit. There is
no fixture-invented failure or assertion pretending this is successful qualification.

The relevant production contracts are:

- `crates/idp-server/src/independent_identity.rs`: IdP's live locally owned identity
  database is exposed through `identity.sock` in its protected setup-host directory.
- `crates/idp-server/src/independent_create.rs`: operation-bound public owner facts
  publish the local socket; no configured TCP listener is published for that owner.
- `crates/device-server/src/independent_create.rs`: Device already discovers that
  local owner socket and uses `db::Client::connect_unix`.
- `crates/management-server/src/provisioning.rs::identity_client`: standalone
  Management rejects absent `identity_database`. Its existing unit test explicitly
  expects rejection rather than a local fallback.
- `crates/db/src/remote.rs::RemoteDatabaseConfig`: permits bare literal-loopback TCP
  HTTP or mTLS HTTPS, not a Unix socket.
- `crates/idp-server/src/identity_database_main.rs`: `identity-database` opens its own
  database file and exposes TCP. It has no existing socket-attachment/relay mode;
  opening IdP's live file again is not a supported connection.
- `crates/idp-server/src/independent_join_worker.rs`: remote SQL Join verifies
  approved existing shared rows; local-owner Join imports public histories. Pointing
  consumers at source SQL is attachment, not offline-independent history import.

Accordingly, there is no currently supported complete actual-process local-owner
Management topology for this fixture. Adding local-owner socket support or another
owner listener/adapter is production behavior, not a test-provider substitution.
Under the explicit no-new-production-features constraint, no such connection was
invented and no TCP-to-Unix fixture proxy was introduced.

**Independent target Join, correlated completion, target Management relationship
launch and source-offline reopen remain unvalidated/incomplete.** The drafted local
target setup/approval branch must be completed/qualified after the production
connection contract is resolved; it must not be described as passing coverage.

## Credential separation and native discovery findings

An earlier Unified Join test wrongly attempted password login on the joining Node.
It reached ready, then returned HTTP 400 on password login. Public security history
intentionally excludes `local_credential_secrets`; see
`docs/public-persistence.md#local-credential-separation`. The correct approved-Join
resource operation consumes an ordinarily issued source Storage token, as prior
source-offline tests do. No password verifier was copied or seeded. Fresh Create
and retained restart still test password login and User info normally.

Unified's actual CLI uses N0 EndpointId discovery. Historical in-process tests use
an internal memory address lookup that this process fixture deliberately does not
inject. An early full run failed Join admission with discovery errors in the child
logs, including DNS failures publishing to `dns.iroh.link` and probing iroh relays.
The fixture now checks native source discovery readiness through the existing native
protocol client before submitting Join. This is a bounded non-mutating preflight,
not approval or signer trust. It does not remove the networking dependency or
make the test consistently pass in this environment.

## Validation commands and results

Every Cargo command used this prefix, from the worktree manifest, sequentially:

```sh
cargo <subcommand> --manifest-path /home/nathan/code/rust/aicacia/of-system-tests-03/Cargo.toml --target-dir /home/nathan/code/rust/aicacia/of/target -j8
```

- `test -p idp-unified --example system-test-service`: **1 passed**, 0.01s.
  Its first red run exposed mock search retaining deleted specifiers (`NoEntry`);
  snapshot persistence now omits absent entries and propagates other provider errors.
- `build -p idp-unified --example system-test-service -p idp-server --bin identity-database`:
  **passed**, final rebuild 13.88s.
- `test -p idp-unified --test system_create unified_retained_restart -- --ignored --exact --nocapture`:
  initial red run failed retained readiness with the in-memory provider (77.63s).
  After durable provider support it passed (25.66s); strengthened final full runs also pass.
- `test -p idp-unified --test system_create independent_retained_restart -- --ignored --exact --nocapture`:
  passed (50.14s). An earlier consumer exit was corrected by waiting for the restarted
  SQL owner's actual listener, not sleeping a fixed 500ms. Strengthened full runs pass.
- `test -p idp-unified --test system_create unified_join_and_source_offline_reopen -- --ignored --exact --nocapture`:
  **1 passed**, 153.39s on the final isolated execution, with native preflight,
  anonymous source-administration denials, exact approval, retained resource and endpoint checks.
- `test -p idp-unified --test system_create independent_join_and_source_offline_reopen -- --ignored --exact --nocapture`:
  **failed**, 24.09s, actual local-owner source Management child exit above.
- **Final post-merge full run:**
  `test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture`:
  **4 passed, 2 failed**, 284.76s, bounded by a 450s command deadline. Both Create
  and both retained restart executions pass. Independent Join fails on Management
  exit. Unified Join fails at the **90-second native Trusted Peer discovery
  preflight timeout**, before target admission; private source diagnostics:
  `/tmp/.tmpKNDAr3`. No flaky rerun was used to relabel this full run green.
- Earlier full run: **4 passed, 2 failed**, 252.78s: same independent blocker plus
  Unified Join admission failure before native preflight was added.
- `check -p idp-unified --all-targets`: **passed**, 15.32s.
- `check -p unified-server`: **passed**, 14.22s.
- `clippy -p idp-unified --test system_create --example system-test-service --no-deps -- -D warnings`:
  **passed**, including after the final native preflight change.
- `cargo fmt --manifest-path /home/nathan/code/rust/aicacia/of-system-tests-03/Cargo.toml --package idp-unified --check`:
  **passed**.
- `python3 scripts/check_server_dependencies.py`, with Python's working directory
  explicitly set to this worktree: **passed**, no direct server-to-server edges.
- `git diff --check`: **passed** before implementation commit.

The first test build/run and the first combined launcher build/test exceeded their
200s tool deadlines. Later commands used larger bounded deadlines. Those aborted
commands are not passing validation; forced runner termination is not orderly
teardown certification. Normal completed runs kill/reap only owned children.

## Limits and follow-up decision

Runtime clients retain 3s connect/30s HTTP deadlines; native source preflight is
90s, setup completion 180s, pending offers 60s, retained readiness 60s. Private logs
retain at most 16 KiB per child incarnation, are drained continuously, and failure
roots remain private. Provider files are test plaintext under private roots, not
OS-provider security qualification. Graceful shutdown, real trust installation,
WAN/container, production-cost passwords, full-suite and platform certification
are not claimed. No CRAP, coverage or powerset commands were run.

Before ticket03 can resolve, authorize/design the supported standalone Management
connection to a local-owner Node without source SQL sharing, then complete the
target Management launch/relationship path, run the independent authorized Join
and source-offline assertions, and obtain reliable Unified native-discovery
qualification in an environment with working production discovery networking.
Only then decide which old tests are directly replaced; private recovery and
layout-specific tests must remain unless their assertions are actually replaced.
