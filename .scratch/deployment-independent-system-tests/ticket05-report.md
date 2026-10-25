# Ticket05 — final assertion audit and bounded basic qualification

## Result, scope and branch

**Resolved for the spec's bounded Linux same-machine mock-provider qualification.**
All nine actual-process scenarios passed serially, as did the external Unified and
IdP transport tests, focused CLI/provider/socket rerun, strict library Clippy and
final contract checks. This is not certification of every remaining private test
or of actual-process live post-Join reconciliation. Earlier failures below remain
part of the result, not erased by reruns.

- Worktree `/home/nathan/code/rust/aicacia/of-system-tests-05`, branch
  `refactor/system-tests-05`, based on integration **120f7953a**.
- Read the full spec, all tickets including 04a/b/c, coverage inventory, reports
  02/03/04, shared README, issue protocol and domain map/glossary. Loaded `tdd` and
  `ponytail`, claimed before implementation, used the already approved public
  generated-owner/native-client seams; no new seam interview or production API.
- Merge before final report: `GIT_EDITOR=true git -C
  ../of-system-tests-05 merge --no-edit refactor/deployment-independent-system-tests`
  returned **Already up to date**, integration tip **120f7953a**.
- Main checkout's eight unrelated staged AD setup-simplification entries remained
  untouched. No main/integration merge, push, branch creation or unrelated process
  signal. Only explicit owned paths are staged/committed.

## Implementation and audit

[coverage.md](coverage.md) is now the final authoritative disposition map, not a
blanket completion claim. Compared its index with actual baseline Rust test
attributes from `git ls-tree`/`git show 191962716`, including cfg/path-mounted and
subprocess-child entries: **155 declarations, no missing original scenario**.
Each identifier has a disposition and reason. Twelve bodies moved externally;
143 remain, including the one narrowed/renamed Join body. Platform/CLI cfg gates
explain test-runner count differences. External composition is additional coverage.

- Confirmed shared assertions in `tests/system/{create,authorization,recovery}.rs`
  consume generated HTTP models/operations and existing typed native/WebSocket
  clients. Both layouts call common scenario functions, not parallel suites.
  Reqwest here only configures transports/roots/URLs; no raw request fallback.
- Audited every remaining Unified raw-request file, explicitly distinguishing
  intentional wire/status/Origin/schema checks from **ordinary legacy HTTP still
  retained** inside private/mixed/deployment fixtures. No claim that all old raw
  calls are justified as malformed-wire checks. Normal shared operations are
  generated-client-first; migration of mixed legacy fixtures remains documented.
- Changed the external HTTP fixture's ordinary CLI health poll to generated IdP
  health, keeping the same deadline and all exact wire acceptance/denial checks.
- Deleted three genuinely unused Unified TLS fixture dev dependencies: `rcgen`,
  `rustls`, `tokio-rustls`. They moved with transport coverage; no remaining Unified
  Rust reference exists. Lock drops **exactly those three Unified edges**, no
  package/version additions or unrelated lock churn. Removed duplicate dev
  declarations of `of-idp-client`/`of-storage-client`; their production dependencies
  remain because verification uses them.
- Actual hook callers still exist: Device/identity/server runtime accessors serve
  private full-node/HTTPS/status-revocation tests; NodeJoin source serves retained
  offer checks; trace hook serves test-only setup/HTTPS diagnostics. No newly
  obsolete runtime field/helper/module found, none exported or blindly deleted.
- Preserved Unified supplied-router/listener, handle cleanup, native ALPN, private
  publication/recovery and independent remote-owner/reset/HTTPS coverage. Owner
  fixture relocation (permission/residency/cleanup) is explicitly deferred where
  injected authority/gates cross owners; not falsely called pure private units or
  completed owner-local placement. Do not introduce server dependencies to move it.

The effort reduced coupling, not merely filenames: 12 internal bodies moved to
public consumers (including 4 to the client owner), obsolete transport fixture
ownership removed, ticket04's 35 duplicate public Join lines replaced by shared
recovery, and ticket05 removes 3 unused direct edges/2 duplicate declarations.
No whole remaining mixed scenario has an assertion-equivalent replacement.

### Applicable live post-Join coverage

Spec-required applicable synchronization was **not dropped**. Initial Join history
and local User deletion do not prove live remote Device restriction enforcement.
The retained `full_node_tests` grant case mutates consumption/revocation **after**
Join and reconnects IdP history; restriction case privately grants reserved actions,
partitions and changes opposite Devices, reconnects Device history, denies retained
sockets, restores and checks unchanged bytes. Both passed focused execution here.
The partition/reopen/private-secret/JWKS case also stays enabled, not rerun here.

Exact post-Join forcing uses live `SecurityProtocol::synchronize`, runtime endpoint,
local member context and owner databases. No equivalent public process trigger or
reserved-policy administration exists. Retain these focused private/mixed tests;
do not read process keys/DBs, expose a production test hook, or count Join import
as their replacement. **Cross-layout actual-process live reconciliation remains
unqualified**, a precise seam limitation permitted by the retained-private scope.

### Narrow metadata refresh

The export check was red solely on pre-existing Management `info.description`:
committed “Management API server for LIDP” versus production “Management role,
permission, and assignment API”. Paths/schemas and the other three documents were
unchanged. Refreshed that one canonical metadata string and ran the existing
`scripts/generate-openapi-client.mjs management` workflow twice. Verified all **60
changed SDK files differ only by that generated description comment**, and second
generation is byte-identical. No client behavior/model/route, manifest dependency,
production owner code or public contract operation changed. The sole production
exception in the overall effort remains ticket03's approved Management socket fix.

## Validation commands and exact results

Commands ran from `/home/nathan/code/rust/aicacia/of` against this worktree, sharing
main's target cache. All agent-issued heavy Cargo invocations were serial. For
every Cargo row below, insert these common options after the subcommand:

```text
--manifest-path ../of-system-tests-05/Cargo.toml
--target-dir /home/nathan/code/rust/aicacia/of/target
-j8
```

Tool bounds below are per invocation/group, not relaxed internal deadlines. No
CRAP, coverage, feature powerset or broad expensive Unified library suite was run.

| Command suffix | Result and bound |
| --- | --- |
| `check -p unified-server -p management-server` | PASS default libraries, 14.02s; 600s bound. |
| `check -p unified-server --features cli --all-targets` | PASS, 23.53s; includes retained test compilation, no execution. |
| `check -p idp-unified --all-targets` | PASS, 8.19s; root default/CLI launcher/system targets. |
| `check -p management-server --no-default-features` | PASS, 8.95s. Above three plus prerequisite build shared one 600s bound. |
| `build -p idp-unified --example system-test-service -p idp-server --bin identity-database` | PASS, 2m37s, actual service binaries. |
| `test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture` | **9 passed, 0 failed, 0 ignored, 0 filtered**, 557.99s execution plus 2m01s build; **900s bound**. Includes both Create/use, both restart, extra independent local-owner restart, both authorized/interrupted/rejected/retried Join/learned history/source-offline reopen, both permission/resource/native-existing-socket denial. |
| `test -p unified-server --test http_host --test owner_routes --test setup_host --test bootstrap_config --test composition -- --test-threads=1` | **9 parent tests passed** (2 bootstrap, 2 composition, 2 HTTP parents, 1 routes, 2 setup), plus explicitly invoked isolated HTTP child **1 passed** (51.49s). Parent HTTP 53.49s. Direct child is ignored only to prevent unisolated execution; parent runs it. Shared 600s bound with transport below. |
| `test -p of-idp-client --test transport -- --test-threads=1` | **4 passed**, 0.35s; 50.09s build. Confirms TLS fixtures still owned/working where needed. |
| `clippy -p unified-server -p management-server --lib --no-deps -- -D warnings` | PASS initially 14.88s; **final after metadata 6.00s**. |
| `clippy -p unified-server --features cli --lib --no-deps -- -D warnings` | PASS initially 14.73s; **final 7.98s**. |
| `clippy -p idp-unified --test system_create --example system-test-service --no-deps -- -D warnings` | PASS initially 7.84s; **final 6.36s**. Final three inside 600s bound. |
| `test -p management-server --lib` | **FAIL, 15 passed/1 failed**, 35.32s; explicit-socket positive case exceeded its existing 1000ms connect/schema deadline. Default runner concurrency; no production edit. |
| `test -p management-server --lib -- --test-threads=1` | **16 passed**, 21.46s, 0.58s cached build. Cause of earlier deadline failure not established; serial pass is not proof of concurrent reliability. |
| `test -p idp-unified --example system-test-service` | **1 passed**, provider reopen/replacement/deletion, 0.01s (3.83s build). |
| `test -p unified-server --features cli --lib cli::tests -- --test-threads=1` | **5 passed**, 0.04s (1m36s build), 137 filtered intentionally. Serial socket/provider/CLI group bounded at 900s. |
| `test -p unified-server --lib generated_rust_and_built_javascript_use_actual_owner_routes -- --test-threads=1 --nocapture` | **1 passed**, 10.95s (1m38s build), 136 filtered; Rust and built JS against actual owner routers, not a substitute process. 600s bound with initial export check. |
| `test -p unified-server --lib two_completed_full_hosts_reconcile_consumption_and_revocation -- --test-threads=1 --nocapture` | **FAIL/abort** with default small thread stack during IdP Join import; SIGABRT/stack overflow after source fixture and authenticated Join (20.08s build). 600s tool bound, not a timeout/pass. |
| `RUST_MIN_STACK=67108864 cargo test` with common options, `-p unified-server --lib two_completed_full_hosts_ -- --test-threads=1 --skip two_completed_full_hosts_join_partition_reopen_and_learn_denial --nocapture` | **2 passed**, 236.86s, 135 filtered. Native live Device restriction and IdP grant reconciliation, unchanged assertions/deadlines; **900s bound**. Explicit stack is a qualification condition, not default-stack certification. |

Additional exact checks (paths below relative to the ticket05 worktree):

- `pnpm --filter @aicacia/idp-client --filter @aicacia/management-client --filter
  @aicacia/device-client --filter @aicacia/storage-client install --offline
  --frozen-lockfile --ignore-scripts`: PASS, cached 302 packages, no downloads,
  757ms/200s bound. Install rewrote six **tracked** Storage bin shims; restored
  exactly those agent-generated changes before commit, no user work reverted.
- Same four filters, `run build:esm`: **all 4 passed**, initially and again after
  metadata refresh. No broad JS workspace build or SDK-language process matrix.
- `node --test scripts/owner-contracts.test.mjs
  scripts/generate-openapi-client.test.mjs scripts/generated-clients.test.mjs
  scripts/generated-device-client.test.mjs scripts/generated-independent-client.test.mjs
  scripts/generated-package-exports.test.mjs scripts/generated-setup-client.test.mjs`:
  **43 passed, 0 failed/skipped**, 118.79s, 200s bound. Includes generation-failure
  preservation, built package exports, owner prefix/security/setup contracts.
  After metadata, repeated same set **except unchanged generation-failure script**:
  **35 passed**, 5.13s, 600s shared bound with repeat generation/build.
- `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=8
  node scripts/generate-openapi-client.mjs management`: PASS twice (600s outer
  first /300s subprocess repeat bound). Existing workflow validates OpenAPI,
  regenerates both SDK families, formats, typechecks, fetches locked and checks
  generated Rust offline. Byte comparison proves repeat generation deterministic.
- Twice `cargo run` with common options `-p unified-server --example export-openapi
  -- <temporary>/first` and `.../second`, then byte comparisons of each canonical
  document: initial check **FAILED** on Management description (not silently
  ignored; the following actual-router test separately passed). Final after refresh:
  **all 4 documents match production and each other**, builds 4.99s/0.34s, 180s
  subprocess bounds. Temporary exports removed after comparison. This is relevant
  deterministic export/Management regeneration qualification, not a claim to run
  the entire broad `qualify-openapi-clients.mjs` orchestrator.
- `python3 scripts/check_server_dependencies.py` with cwd explicitly set to this
  worktree: PASS, repeated final, **no direct server-to-server edges**.
- `cargo fmt --manifest-path ../of-system-tests-05/Cargo.toml --package
  unified-server --package management-server --package idp-unified --package
  of-management-client --check`: PASS. Generation also checks its Rust formatting.
- `git diff --check`: PASS before final documentation/commit and repeated final.
- Audit-only first naive name search exceeded a 10s tool bound; reindexed source
  once and reran successfully. This was not test execution or validation success.

The nine-process and external passing runs preceded the metadata-only refresh;
no runtime/shared scenario implementation changed afterward. Final byte checks,
regeneration/typechecks/SDK tests and Clippy cover that comment-only refresh.
No need to relaunch 9 Nodes to retest generated comments.

Final post-merge/default checks were repeated after the metadata refresh and
report assembly: default Unified/Management libraries **PASS 3.21s**, Unified CLI
all-targets **PASS 4.72s**, root all-targets **PASS 2.54s**, same common Cargo options,
600s group bound. Relevant four-package fmt and `git diff --check` passed again.

## Limits and preserved failure evidence

All earlier reports retain their historical failures/timeouts: ticket02 full-cost
503; ticket03 provider/Management/discovery/HTTPS trust/startup transients; ticket04
private reopen failure and documented setup differences. This ticket does not
explain away those failures or claim indefinite flake-free operation.

Known ticket05 qualifications: initial socket positive test deadline failure under
concurrent runner, default-stack live import abort, explicit 64 MiB live-test stack
success, and a metadata export mismatch corrected without behavioral changes.
No currently failing requested **final** basic/shared/contract check remains, but
these earlier results constrain concurrent/default-stack claims. No assertion,
permission, response fence, timeout or production policy was weakened to get green.

Same-machine Linux only, sibling path checkouts, mock provider persisted as private
plaintext test snapshots, isolated HTTPS CAs and fake OS trust installers. Unified
process Join still needs real N0 DNS/discovery/relay networking. Independent
Management and remote SQL use supported loopback listeners; local-owner Management
uses the approved explicit IdP-owned socket. Small password costs, two service
workers and 64 MiB service stacks remain test conditions. Logs retain bounded 16 KiB
per owned child and secrets are not intentionally printed. Kill/reap is cleanup,
not graceful shutdown certification; aborted/forcibly killed runners do not prove
teardown. No unrelated process was killed.

Unqualified: real OS key store/trust installation, WAN/container/proxy/mTLS, other
platforms, load/performance, default-stack live reconciliation, arbitrary network
availability, the complete expensive retained Unified suite, cross-layout live
post-Join synchronization, exhaustive cross-layout source/target matrix and broad
regeneration of untouched SDK owners. Mixed owner fixture placement remains an
explicit migration opportunity, not hidden loss of assertions. These are documented
limits under the spec's private/layout exceptions, not unimplemented shared basic
scenario criteria. See [coverage map](coverage.md) and [shared README](../../tests/system/README.md).
