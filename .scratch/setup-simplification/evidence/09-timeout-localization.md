# Ticket 09: timeout localization

**Latest disposition:** [authorized focused execution](#authorized-focused-execution-2026-10-26) reproduced a traced approval timeout and observed a recovered readiness request timeout plus a distinct stopped-Database reopen failure. No deterministic root-cause regression or behavioral fix yet; ticket09 remains claimed/unresolved. The observation-only handoff below is historical; its NOT RUN labels describe that first pass, not the executed results appended here.

## Status and ownership

**Ready for coordinated focused testing, NOT localized or resolved.** Agent09 claimed [ticket09](../issues/09-diagnose-readiness-and-request-timeouts.md) on `implement/setup-followup-09` in `/home/nathan/code/rust/aicacia/of/.worktrees/setup-09`. Entry was clean at requested integration `8ee26fb1f8d789840dc246b94cfa22f6fdb7e6a5` (includes tickets08/10). No merge, sibling edit, provider/reset-fixture/SDK change, or behavioral fix belongs to this work.

Agent10 owns the Rust slot. **No build, test, executable discovery by execution, formatting/lint command, feature matrix, or coverage was run in this pass.** Commands below are a predeclared future plan, not results. Only source inspection and `git diff --check` have validated these changes. Do not treat ready-for-test as compile-verified. Commit hooks must not start Rust work during this handoff.

Read the setup spec (implementation/testing decisions and acceptance), ticket09, ticket07, [reliability](07-reliability-followup.md), and [validation](07-validation-followup.md) evidence. Loaded TDD and ponytail. Complexity medium, diagnostic risk low-to-medium (logging perturbs scheduling), localization priority medium; authorization/lifecycle risk remains high. Existing public HTTPS Join parents and independent-owner public HTTPS parents are the user-authorized seams; no new suite, fake, or helper framework.

## What source establishes — not a runtime diagnosis

- The separate admission/router ownership and queued-reply shutdown defects already have focused red/green evidence. Later 9/9 serial Join passes do not explain historical pending-offer or ordinary-authorized-request timeouts.
- The historical traced ordinary approval timeout was a **5-second client request** failure, not the parent watchdog. The historical pending-offer bound was 15 seconds. Source-observation hooks in `device-server/src/node_join_source.rs` and `idp-server/src/router/node_join_approval.rs` were `cfg(test)`: dependency builds in unified-server tests omit them. Their absence cannot establish that owner work did not start.
- Source approval goes through exact-offer lookup, per-offer operation mutex, current-offer checks, IdP enrollment, current verification, Management Device ownership, policy read/write, approved-peer refresh, another current-offer check, reauthorization/current verification and signed reply publication, then offer cleanup. The operation guard covers this entire sequence; waiting for it is distinct from time inside owner work. No stalled await has been identified dynamically.
- IdP enrollment reauthorizes and rechecks the approving Device before exact enrollment. The `transaction_begin/end` category brackets `enroll_node_join`, including Management Device enrollment and the IdP transaction, **not just SQL transaction acquisition**. Narrower instrumentation/regression is warranted only if this category is implicated.
- `tests/owner_composition_tests/create.rs::wait` serves Create, Join, operation and remote-setup callers. It observes IdP, Device, then Storage under **one shared 60-second deadline**. It does not grant a fresh minute per owner. Request-send errors/non-200 statuses were silently retried; successful 200 bodies were parsed and compared to the expected readiness boolean. The assertion follows each unsuccessful attempt, so an individual request/body wait can consume or cross the shared deadline. This is a source fact, **not proof** that this caused any of the four historical failures.
- The independent Management permission-request timeout is still a separate unresolved observation. These changes do not instrument all ordinary HTTP traffic or assign it the approval/readiness mechanism.

## Observation changes and safety contract

Only four existing source files change:

1. `crates/device-server/src/node_join_source.rs`: replace compiled-out prints with `log::debug!` on the explicit **`setup_timeout`** target. Categorical HTTP handler entry/end, offer lookup, operation-lock wait/acquire, enrollment begin/end, verification begin, policy begin, peer refresh begin/end and reply begin/end. Adjacent cumulative times delimit grouped verification/policy work; no new await or lock. Handler end includes success and `io::ErrorKind`, never its message.
2. `crates/idp-server/src/router/node_join_approval.rs`: same dependency-visible target for authorization, revalidation, enrollment transaction and verification, plus reply authorization/verification/publication. Transaction/publication ends include only success/error kind. Existing test-only activation hook stays unchanged.
3. `crates/unified-server/src/api_join_tests.rs`: existing ordinary authorized approval request gets start/end timing, numeric HTTP status or reqwest timeout/connect/request booleans **before the existing expect/assert**. With feature `cli` and `OF_SETUP_TIMEOUT_TIMING` present, its isolated child installs the existing optional env_logger with fixed `off,setup_timeout=debug`, not `RUST_LOG` or a broad HTTP filter. No new dependency. Without `cli`, client timing can print but the server logger is not installed; diagnostic commands explicitly enable `cli`.
4. `tests/owner_composition_tests/create.rs`: existing readiness helper optionally reports each request start, headers/status or categorical transport error, JSON/body completion or categorical timeout/decode error, and matching readiness. It records cumulative shared-wait, owner and request milliseconds, expected boolean and parsed readiness boolean only. Original parsing/assertions, sequential owner order, deadline and 50ms polling sleep remain intact. No response body/state string or error Debug/Display is logged.

`OF_SETUP_TIMEOUT_TIMING` is presence-based and is read only in tests; **unset it**, rather than set it to `0`, for control cells. Production logs require an installed logger allowing the dedicated debug target. Timers are local monotonic `Instant`s; elapsed time is relative to each seam, not a globally synchronized clock. Existing log timestamp ordering and adjacent phase deltas aid diagnosis, not proof of task scheduling/CPU attribution.

No added event contains credentials, codes, bearer headers, URLs, issuer/endpoint/offer/member/operation IDs, request/reply payloads, arbitrary strings from responses, or database values. Existing scenario assertions/other pre-existing diagnostics can still print URLs/retained status on failure: keep raw artifacts private and quote only sanitized categorical evidence. No blanket HTTP logging is enabled. Early bearer/source refusal can exit the HTTP handler after entry but before its end event; panics/cancellation also need not emit end. A missing end alone is not proof of a deadlock. Concurrent child stderr can interleave without correlation IDs; do not pair ambiguous events as one request.

**Unchanged:** exact intent and identity, authorized approval, pinned-peer trust, current readiness, provider ownership, reset behavior, SDK prerequisites, retries, sleeps, timeouts, and parent/child isolation. No real OS/profile/provider work is authorized.

## Fixed-input receipt at observation handoff

Resolved worktree sibling links point to the existing repositories, not new clones. All three were clean at this source-only snapshot; no claim that they will stay fixed during someone else's build:

| Repository      | HEAD                                       |
| --------------- | ------------------------------------------ |
| of (entry base) | `8ee26fb1f8d789840dc246b94cfa22f6fdb7e6a5` |
| ofdb            | `e3242170c94d5fc4bcc121db812310c8106c7ab6` |
| ofnet           | `dbea925a0a3f99053fffc02618baccc2a1cbea9f` |
| offs            | `df4c696a910296635473ec296d9235acbaa60d9b` |

SHA-256 of exact observation source bytes (before any future formatting/build):

| Input                                       | SHA-256                                                            |
| ------------------------------------------- | ------------------------------------------------------------------ |
| Cargo.lock (unchanged)                      | `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f` |
| device-server/src/node_join_source.rs       | `f4b9f34e7005e9e6d90e02c7cf52e55913c52de2eecb3c672068d7f797abb56f` |
| idp-server/src/router/node_join_approval.rs | `40c211b54e28f42681231dd7c921ff7081cfb43affa6b616e0ed6c467fa3d53a` |
| unified-server/src/api_join_tests.rs        | `8bc910247fb73ae0b3cff669fb4e6d91828dca58f553ac4df4a762ad995b1dbd` |
| tests/owner_composition_tests/create.rs     | `d0eeff2eee90cba8c433f686404f03e546358326bb61335ee2b06153dd4154a3` |

The four-source-file `git diff --binary` against entry HEAD hashes to `06011f895346b59528f218a269e5dc2e351aa30d838ef5ab55305ecc87ee6696`. Entry `git archive HEAD` hashes to `f2cb6112c23b18d09ef43fb062552587868d4e14a279dab0fd88b436c140e45c`. These fix source inputs, not compiler/artifact provenance. No new executable/hash exists yet. The observation commit is identifiable by this file's introducing commit; record its full HEAD in every run receipt.

Before and after each future build/cell, record main and sibling full HEAD, status, diff hash if dirty, actual `readlink -f` targets, Cargo.lock hash, rustc/Cargo versions, feature flags, selected parent names, cwd, relevant environment, and executable SHA-256. Obtain a coordinated no-mutation window for the sibling inputs; before/after receipts alone are not a hermetic build guarantee. Build from clean committed inputs; stop if any source/sibling/lock changes. A binary name/mtime or Cargo fingerprint alone is not proof of source identity. Do not reuse the historical executables as qualification of this code. Reuse the exact newly verified bytes across all cells; record stdout/stderr log hashes. Do not reset/stash/pin a sibling without its owner's authorization.

## Predeclared execution cells — all NOT RUN

First wait for explicit release of Agent10's Rust slot. No concurrent compilation/coverage/other qualification workload from this effort. Record `ps -eo pid,comm,pcpu,etime --sort=-pcpu` plus load/memory pressure before each cell; do not record process arguments or claim global host idleness.

Use the **same binaries, flags and selected scenarios** for control/diagnostic cells. No adaptive retry-until-green. At most three repetitions per cell, stop that cell on its first failure, preserve its full output and phase timeline before deciding another experiment. Each selected parent is retained, never invoke an ignored child directly. Serial cells use libtest threads=1; concurrent cells use threads=2 within the selected group only (no full owner suite). Run unified and owner groups separately, not overlapping. Concurrency and logging are independent factors:

| Cell | Scheduling                       | Categorical timing | Syscall tracing        | Purpose                                                                                    |
| ---- | -------------------------------- | ------------------ | ---------------------- | ------------------------------------------------------------------------------------------ |
| S0   | serial                           | unset              | none                   | untraced control; repeat finalization, interruption/exact retry, offline reopen            |
| S1   | serial                           | enabled            | none                   | phase localization with logging cost separated from S0                                     |
| C0   | concurrent (2)                   | unset              | none                   | selected-test concurrency sensitivity, without logging                                     |
| C1   | concurrent (2)                   | enabled            | none                   | categorical localization under selected concurrency                                        |
| T1   | serial, interruption parent only | enabled            | restricted futex/flock | separate traced replay; compare with the same parent's S1, never pool with untraced passes |

Selected unified group: `production_api_join_` selects the existing finalization, interruption/exact-retry and offline-reopen parents (three; verify with `--list` first). Selected owner group, exact full names:

- `owner_composition_tests::create::production_independent_create_and_offline_reopen`
- `owner_composition_tests::create::production_independent_create_unsupported_record_refusal_and_current_reset` (ticket10 repaired the formerly legacy-named scenario; no reset-fixture edits here)
- `owner_composition_tests::operation::production_independent_signer_free_operation_and_delayed_authority_reopen`
- `owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation`
- `owner_composition_tests::permissions::independent_permission_production_management_https`

The fifth parent preserves the historical separate permission-request representative; the new readiness helper does not necessarily observe its failing ordinary route. Small concurrent cells do **not** reproduce the original full matrix load and cannot disprove it. If any cell fails, first identify whether time is before handler entry, offer/operation lock, enrollment, verification/policy, refresh, reply, client response, or a particular readiness owner/status/body/error. Persist actual observed timings, exit status and unchanged bounds. No inference of common cause from similar timeouts.

### Gated build / diagnostic commands (cwd this worktree)

**Not executed.** After slot release, formatting and source receipt refresh come first. These commands deliberately use cli without Swagger UI and locked dependencies; record that feature choice rather than implying equivalence to the old feature matrix. If `--locked` fails, stop and report; do not refresh/revert the lock opportunistically.

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=1 RUST_MIN_STACK=67108864 cargo test --locked -p unified-server --features cli --lib --no-run --message-format=json > /tmp/setup09-unified-artifacts.json
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=1 RUST_MIN_STACK=67108864 cargo test --locked -p idp-unified --no-default-features --features cli --test owner_boundaries --no-run --message-format=json > /tmp/setup09-owner-artifacts.json
```

Resolve executables from actual Cargo artifacts, not guessed historical suffixes. This prints paths and SHA-256 without executing tests:

```sh
python3 - <<'PY'
import hashlib, json
from pathlib import Path
for receipt, name in [("/tmp/setup09-unified-artifacts.json", "unified_server"), ("/tmp/setup09-owner-artifacts.json", "owner_boundaries")]:
    artifacts = [json.loads(line) for line in Path(receipt).read_text().splitlines() if line.startswith("{")]
    paths = {a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("target", {}).get("name") == name and a.get("profile", {}).get("test") and a.get("executable")}
    assert len(paths) == 1, (receipt, paths)
    path = Path(paths.pop())
    print(name, path, hashlib.sha256(path.read_bytes()).hexdigest())
PY
```

For an S1 smoke **after** confirming source/artifact provenance, the following selects only the three existing unified parents and is bounded at 1000s. Capture to a private file and hash it; use outer timeout/process-group cleanup as well if supervising concurrent children. This is one suite invocation, not the planned repetitions or a passing result:

```sh
OF_SETUP_TIMEOUT_TIMING=1 RUST_MIN_STACK=67108864 python3 - <<'PY'
import json, subprocess
from pathlib import Path
artifacts = [json.loads(line) for line in Path("/tmp/setup09-unified-artifacts.json").read_text().splitlines() if line.startswith("{")]
paths = {a["executable"] for a in artifacts if a.get("reason") == "compiler-artifact" and a.get("target", {}).get("name") == "unified_server" and a.get("profile", {}).get("test") and a.get("executable")}
assert len(paths) == 1, paths
with open("/tmp/setup09-S1-unified-1.log", "xb") as output:
    result = subprocess.run([paths.pop(), "production_api_join_", "--nocapture", "--test-threads=1"], stdout=output, stderr=subprocess.STDOUT, timeout=1000)
raise SystemExit(result.returncode)
PY
sha256sum /tmp/setup09-S1-unified-1.log
```

For owner execution use the resolved `owner_boundaries` executable with the five full names above as libtest filters plus `--exact --nocapture --test-threads=1` (2 in C cells); allow 1000s for the whole group, never change individual parent bounds. For S0/C0 remove `OF_SETUP_TIMEOUT_TIMING` from the inherited environment; for C1 change only test threads to 2. For T1 use only the exact interruption parent, `--exact --nocapture --test-threads=1`, prefixed by `strace -f -ttt -e trace=futex,flock -o /tmp/setup09-T1-syscalls-1.log`. No network read/write/send/recv, TLS payload, broad HTTP trace, or `RUST_LOG=debug`. Preserve 300s parent/5s request and all scenario bounds; strace overhead is the point of a separate cell, not a reason to enlarge them.

## TDD gate and remaining work

This is instrumentation, not a green implementation claim. No regression was written or run without localization, and no known failing behavior was patched. After an actual failure identifies a boundary/mechanism, add the **smallest deterministic public-seam failing regression**, demonstrate red on this observed implementation, then make the root-cause fix and show green. Do not test log strings/private call counts or use delays as a substitute for causality. Reuse existing scenarios to qualify any later fix.

Ticket09 remains **claimed/unresolved**, every acceptance box remains unchecked, ticket07 is untouched. Repeated finalization/interruption/offline results for this observation source are **not yet available**. No common cause, baseline equivalence, environment-only attribution, final readiness qualification, matrix, CRAP score, real platform/provider acceptance, or historical timeout elimination is claimed.

## Authorized focused execution 2026-10-26

### Integration, resource bounds and fixed inputs

After the user released the exclusive Rust slot, fast-forwarded `df45b41ec44c0eb90b4319f54fa1ac69b57937dd` to requested integration **`e6c72aae991f833670cb4820471ab48ac73b6bfb`** before formatting/builds. Reinvoked TDD and ponytail. No competing qualification workload was launched. All builds/lints used `CARGO_BUILD_JOBS=2`, `RUST_MIN_STACK=67108864`, shared `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target`, and this worktree as cwd. Each build/tool scenario invocation was bounded at **600000ms**; the disposable process-group supervisor additionally bounded each selected scenario group at 570s, with TERM/KILL/reap on exhaustion. None of those bounds fired; existing scenario parent/request/readiness bounds were unchanged.

Declared **one repetition per cell** before builds, within the earlier maximum-three plan and the user's one-focused-block budget. No failed cell was rerun to green. Cells/groups ran sequentially, not concurrently with compilation, Clippy, or one another; only C0/C1 selected libtest parents ran concurrently (`--test-threads=2`). Once libtest was launched, its selected queued parents ran to completion even if another parent failed: C1 owner's 4 passes/1 failure is one invocation, not a stop/retry loop. Parents continued to launch their own isolated ignored children; no ignored child was invoked directly.

Actual toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`. Main Cargo.lock hash stayed `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`. All four repositories were clean, with matching HEAD/status at every cell boundary and matching archive hashes before builds and after all cells/linting **before the final lint-only edit**:

| Input repository | Full HEAD                                  | SHA-256 of `git archive HEAD`                                      |
| ---------------- | ------------------------------------------ | ------------------------------------------------------------------ |
| of               | `e6c72aae991f833670cb4820471ab48ac73b6bfb` | `a4a357d69590e1506eb05f6248ec0d7c0813ab4937613ee8848015f4c9c96662` |
| ofdb             | `e3242170c94d5fc4bcc121db812310c8106c7ab6` | `b9c32a875b4c67552ecd9e30461cb381be1418e322664fe8bcb6a402724d3a43` |
| ofnet            | `dbea925a0a3f99053fffc02618baccc2a1cbea9f` | `bfba0566e795912b15cb555a3f9f724244c950bee11f18a5ea9ca73f031fc582` |
| offs             | `df4c696a910296635473ec296d9235acbaa60d9b` | `7fe57bb321e805a52c991a299e83aa38e23ac77a637be8d83a100c937a2c97f1` |

Sibling symlinks resolved to the same actual repositories recorded above. None was modified. These receipts establish observed unchanged inputs, not an OS-enforced hermetic snapshot. Per-cell private JSON receipts include UTC, full command argv, permitted environment, before/after source checks, binary hashes, exit status, wall time, process-name-only snapshots, load and memory PSI. Before-cell load averages (1 minute) ranged 2.24–2.70; memory PSI avg10 was 0.00 throughout. Background editor/system processes existed. This is **not global idleness or a resource-cause attribution**.

### Actual builds and artifact identity

Executed both previously proposed `cargo test --locked ... --no-run --message-format=json` commands, replacing jobs=1 with the authorized jobs=2. Both passed within 600000ms:

- Unified: `cargo test --locked -p unified-server --features cli --lib --no-run --message-format=json`, **6m18s**. No Swagger UI feature.
- Owner: `cargo test --locked -p idp-unified --no-default-features --features cli --test owner_boundaries --no-run --message-format=json`, **5m59s**. No Swagger UI feature.

Selected the executable from each actual Cargo JSON `compiler-artifact`, checked `build-finished.success`, and verified `--list` before executing. The unified filter selects exactly the three predeclared Join parents; the owner command uses the five full names listed above plus `--exact`. Actual artifacts:

| Executable under shared `target/debug/deps/` | SHA-256 used in **every execution cell**                           |
| -------------------------------------------- | ------------------------------------------------------------------ |
| `unified_server-f70d87b770f6bdb5`            | `b4dce12f21f4527d0ebbfa9976c523c09316f04213b2f185069979219297da00` |
| `owner_boundaries-82e9ad7da03710d6`          | `c1e151e65e6d2f95ed0a7dacb80bcdb002f37de6357c1690af55cadcff0a144c` |

Both hashes were asserted before/after every relevant cell; no binary was rebuilt between cells. A later **compile-only** rebuild for the lint-only edit replaced the owner executable at the same filename; its new hash is recorded below. Do not mistake that mutable filename for the original cell bytes.

### Actual results — separate cells, not pooled qualification

All times below are supervisor wall seconds. A group contains the three unified or five owner parents already declared, not the full suite/matrix.

| Cell                                                              | Unified result / seconds       | Independent-owner result / seconds         |
| ----------------------------------------------------------------- | ------------------------------ | ------------------------------------------ |
| S0, serial, timing unset, untraced                                | **3 passed**, 257.332          | **5 passed**, 381.919                      |
| S1, serial, timing enabled, untraced                              | **3 passed**, 258.742          | **5 passed**, 378.006                      |
| C0, threads=2, timing unset, untraced                             | **3 passed**, 175.943          | **5 passed**, 213.725                      |
| C1, threads=2, timing enabled, untraced                           | **3 passed**, 177.145          | **4 passed / 1 failed**, 191.065, exit 101 |
| T1, serial interruption only, timing enabled, futex/flock tracing | **1 failed**, 42.963, exit 101 | not part of this cell                      |

Finalization and source-offline reopen each passed in S0/S1/C0/C1; interruption/exact retry also passed in those four untraced cells, then failed at approval in T1. Thus untraced Join was **12/12**, but traced interruption was **0/1**. Do not turn four different scheduling/logging cells into three idle repetitions or a reliability guarantee. Independent permission HTTPS parent passed in all four owner cells. No 60-second shared-readiness assertion or 15-second pending-offer failure occurred in this block.

### Traced ordinary approval: reproduced and partially localized

T1 actual command: same environment/cwd and verified unified binary, prefixed by `strace -f -ttt -e trace=futex,flock -o /tmp/setup09-T1-unified-1.strace`, exact parent `local_provisioning_tests::https_bootstrap_tests::production_api_join_interruption_preserves_private_host_intent --exact --nocapture --test-threads=1`. Trace excludes HTTP/TLS payload reads/writes and broad network logging. No retry of this failed cell.

The child panicked at `crates/unified-server/src/api_join_tests.rs:457` (`ordinary authorized source approval`), then the parent failed its child-success assertion. Sanitized categorical evidence:

```text
seam=approval_client phase=start elapsed_ms=0
seam=approval phase=op_lock_wait elapsed_ms=0
seam=approval phase=op_lock_acquired elapsed_ms=0
seam=enrollment phase=authorization_end elapsed_ms=626
seam=enrollment phase=revalidation_begin elapsed_ms=731
seam=enrollment phase=transaction_begin elapsed_ms=1464
seam=enrollment phase=transaction_end elapsed_ms=2502 successful=true error_kind=None
seam=enrollment phase=verification_end elapsed_ms=3451
seam=approval phase=policy_begin elapsed_ms=3764
seam=approval phase=peer_refresh_begin elapsed_ms=3835
seam=approval phase=peer_refresh_end elapsed_ms=4505
seam=approval phase=reply_begin elapsed_ms=4505
seam=reply phase=authorization_begin elapsed_ms=0
seam=approval_client phase=end elapsed_ms=5001 timeout=true connect=false request=true
seam=reply phase=verification_begin elapsed_ms=646
seam=reply phase=verification_end elapsed_ms=933
seam=reply phase=publish_begin elapsed_ms=933
seam=reply phase=publish_end elapsed_ms=933 successful=true error_kind=None
seam=approval phase=reply_end elapsed_ms=5439
seam=approval_http phase=end elapsed_ms=5439 successful=true error_kind=None
```

The successful handler completion was observed **after the client timed out**, not inferred from a later passing replay. This run does not stall before the handler, on the offer operation mutex, or in reply publication. The client deadline is crossed during the grouped reply authorization interval after approximately 4.5s of preceding owner work. Enrollment, verification and peer refresh also account for material elapsed time. There is no one five-second blocked await in these category boundaries.

For comparison, S1 interruption's cumulative server phases were enrollment end **2564ms**, policy begin **2769ms**, peer refresh begin/end **2816/3313ms**, reply end **3995ms**; client got HTTP 200 in **4078ms**. Other S1 approvals completed at the client in **3774/3898ms**; C1 approvals returned 200 in **4288/4347/4240ms** (do not map the first two overlapping children to a particular parent). T1's larger elapsed work is measured under syscall tracing; one paired observation cannot distinguish tracer cost from every source of scheduling/owner latency or attribute the historical untraced failures.

**Localization boundary, not a behavioral root cause:** cumulative owner-side request processing overran the unchanged 5s client bound under this traced execution. The per-query cost, executor scheduling, backend workload and trace perturbation were not separately measured; do not infer an Iroh hang, operation-lock contention, authorization bug, DB bug, common timeout cause, or a warranted deadline change. Existing reauthorization/current trust checks must not be removed to fit a budget. No focused deterministic causal regression has yet been established, so **no behavioral fix was attempted**. A future fix still requires such a regression, not an arbitrary injected sleep or an assertion on these timing/log values.

### Independent readiness and the separate stopped-owner failure

S1 readiness emitted only 200 statuses in complete parsed header lines, plus 14 startup connection-error lines; no body errors or request timeouts. Maximum complete shared-wait match was **24357ms**, maximum owner interval **12781ms**, maximum request-to-headers **4582ms**. These are client observations; 200 does not alone assert readiness—the original JSON readiness comparison still determines success.

C1 captured this complete additional line:

```text
seam=independent_readiness owner=idp phase=request_error elapsed_ms=12656 request_ms=5001 timeout=true connect=false request=true
```

The existing helper recovered through its original polling behavior; no new retry was added. Complete C1 match lines reached at most **25792ms** shared / **17612ms** one owner, with maximum complete request-to-headers **4904ms**. No body error or terminal readiness assertion failed. Multiple isolated children write to inherited stderr, and some C1 lines interleave **within** formatted output, so these extrema/counts are over intact lines only. This IdP status timeout cannot safely be assigned to one of the two overlapping Create parents. Its exact internal readiness check is not instrumented; client-side timing alone does not identify the server-side cause. The original four matrix 60-second failures remain unlocalized.

The **terminal** C1 owner failure instead was ordinary Create's child at `tests/owner_composition_tests/create.rs:1080`, after approximately **52.97s**:

```text
existing-only stopped owner: QueryError { kind: Storage, message: "Database already open. Cannot acquire lock." }
```

That is `seed_endpoint_negatives` reopening the isolated Device Database after `shutdown.cancel()` and successful awaits of all three independent owner servers (`create.rs:949–956`), **not** a timeout of an HTTPS approval or readiness assertion. The unsupported-record/current-reset Create parent, operation parent, permission parent and remote-setup parent passed in that same invocation. The isolated root was removed by its existing parent; no live production data/provider was touched.

Source follow-up (no speculative patch): Device `independent_create::run` awaits `IndependentDeviceRuntime::shutdown`, then closes its endpoint. Runtime shutdown joins its offer task and Iroh protocol; the control also owns normal routers and a readiness callback capturing the Device store (`independent_create.rs::observe`). `api::IndependentCreateHost::serve` delegates to generic `serve_setup`, whose Axum graceful-shutdown task is awaited but has no shared-router destruction fence equivalent to the earlier unified-host fix. These are concrete ownership paths worth instrumenting, **not proof of which reference held the failed DB lock**. The actual lock holder/destruction order was not traced in C1. The appropriate next regression seam is immediate existing-only Database reopen after independent owner stop, with deterministic ownership/drop evidence before choosing any fence/cleanup fix. Do not delete a lock, force unlock, sleep/retry reopen, rewrite provider/reset fixtures, or claim the previous unified fix diagnoses this independent failure automatically.

### Formatting, scoped Clippy and final lint-only cleanup

`cargo fmt --all -- --check` passed immediately after integration, before builds. Strict production-library Clippy **passed** in 21.16s:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 cargo clippy --locked -p device-server -p idp-server -p unified-server --features unified-server/cli --lib --no-deps -- -D warnings
```

Strict test-target Clippy did not pass:

- `cargo clippy --locked -p unified-server --features cli --lib --tests --no-deps -- -D warnings`: **failed**, 14 lib-test diagnostics and one `full_node` integration-target diagnostic. Locations are existing `config.rs`, `endpoint.rs`, `hosted_setup_integration_tests.rs`, `local_provisioning_tests.rs`, `runtime.rs`, `tests/support/identity.rs`, and `tests/support/full_node.rs`; no diagnostic in changed `api_join_tests.rs`. Includes unused support functions, items-after-test-module, unnecessary `to_vec`, collapsible conditions, field-reassign-with-default, single-element loop and needless borrow. No unrelated lint cleanup.
- `cargo clippy --locked -p idp-unified --no-default-features --features cli --test owner_boundaries --no-deps -- -D warnings`: initially **failed with seven** diagnostics, including a collapsible response/status conditional in the touched readiness helper. Collapsed that one conditional to an equivalent let-chain and ran `cargo fmt --all` plus `cargo fmt --all -- --check` and `git diff --check`, all passed. This is **lint-only**, not a timeout/ownership fix; no await, deadline, retry, sleep, ordering or assertion changed.
- Reran the same owner Clippy after that focused cleanup: **six remain**, all outside the changed helper: `review_transfer_restart_tests.rs:112`, `create.rs:1563`, `operation.rs:491`, `remote_setup.rs:368` (collapsible conditions), `tests/support/identity.rs:59` (needless borrow), `node_bootstrap_interruption_tests.rs:804` (drop_non_drop). They were not modified or suppressed.

All execution cells precede this lint-only edit and tested the immutable e6c72aae9 source, not the final edited source. Final `create.rs` SHA-256 is `b5acc714244da2af92c702959ea33171ecd2428ab5c500172888d635c0f798a2`. A final same-flags owner **no-run compilation passed in 26.70s**; it replaced `owner_boundaries-82e9ad7da03710d6` with SHA-256 `f7fccfcbe8993bf700af88cf22408dffc44fc94774424397131902deed9a1c2c`. No test was executed from that rebuilt owner binary and no failure was retried to claim green. The unified source/artifact was not changed. There is no final-code complete-system qualification claim.

### Durable artifact receipts and disposition

Raw logs are private disposable local artifacts, not committed transcripts. Their existing assertions can contain URLs/IDs; only categorical sanitized excerpts are copied here. Added timing was inspected: static phase/owner labels, durations, booleans, numeric status and error-kind enums only. No passwords, setup codes, bearer tokens, request/reply bodies, issuer/endpoint/offer IDs or arbitrary response strings were added to instrumentation. All 32 owned capture files were restricted to mode 0600 at finalization; no real OS/profile/provider changes, package/SDK changes, matrix or coverage was run.

| Private local log                                 | SHA-256                                                            |
| ------------------------------------------------- | ------------------------------------------------------------------ |
| `/tmp/setup09-S0-unified-1.log`                   | `1ba6621e0650fbec001b4f1342ce1921db85a417b40ff53e9cd7afe9e855356b` |
| `/tmp/setup09-S0-owner-1.log`                     | `4d4ffc31b62b794adfa1e9495c7d263ddab63258694273b8f37f92a9ac421e90` |
| `/tmp/setup09-S1-unified-1.log`                   | `81a498de955fd7cbbbf30d10031c9231ded0d6d1ed3ca649dc7b6caa6e2c1384` |
| `/tmp/setup09-S1-owner-1.log`                     | `2045bac5562eaa71339755a860adad380694cc9dc9f39a20a101d629e36a24ab` |
| `/tmp/setup09-C0-unified-1.log`                   | `487a0f5a32ce094275e744f09eaa4044597b545fda0fee561b8ff7ef11d55dee` |
| `/tmp/setup09-C0-owner-1.log`                     | `c8677727502922ba3d3153357bb4bebc53f4f57f66f766af3974c43070063f7a` |
| `/tmp/setup09-C1-unified-1.log`                   | `8671ac4f78191b37b3bc9fd81878aa8972d388a2abcf3392b7bfc7aa1c1b4e3b` |
| `/tmp/setup09-C1-owner-1.log`                     | `5e5ec152365504437bc8905560251358c7184621208724f4b3f8cbfee657eea7` |
| `/tmp/setup09-T1-unified-1.log`                   | `edc25d5fc1e92061ce74736142564f29429247a4c62426d9f7f5816a03b284ee` |
| `/tmp/setup09-T1-unified-1.strace` (506096 bytes) | `5483afd20a0a3304acd815718fc1cdfeb99b4c3604b8fd090e765261f469e977` |
| `/tmp/setup09-clippy-unified.log`                 | `9c239dd9bc220a1377405877f85bf2b30a5cbd009f8a083c548c9fbb5da6ba3c` |
| `/tmp/setup09-clippy-production.log`              | `216403eb6d50b07155d88349de7cd16d1096b05601e4ca34bbcc9984253db20c` |
| `/tmp/setup09-clippy-owner-after.log`             | `c1cbc403e9f013f1c7d92eea25f91bea4a8e43d83b4b1bd6b8c070ee77fc8737` |

Private `/tmp/setup09-artifact-manifest.json` SHA-256 **`a353cd3cc986b0a9255fc70080d44c70ba8d4485c1de0f70f7dfcb0c5dc06302`** records all capture sizes/hashes, including Cargo JSON/build logs, initial/final source receipts, per-cell JSON, lint logs and disposable 5081-byte supervisor (`/tmp/setup09-run-cell.py`, SHA-256 `4c0fe0ce37d8e8a2734fb3b0b5c9067f95f9e72b803d4931ba22e103a6ab6a36`). No new test suite/harness was added to the repository. This supervisor only selects the existing tests, enforces the outer process-group bound, and captures provenance; it does not inject behavior or retries.

**Ticket09 stays claimed/open in the work sense (unresolved).** All acceptance boxes remain unchecked; ticket07 is unchanged. Traced approval overrun is now reproduced and narrowed to cumulative owner processing, but there is no deterministic implementation root-cause regression/fix. Historical terminal readiness/permission/pending-offer failures are not eliminated or assigned a common cause; the recovered IdP status timeout and distinct stopped-Database failure remain follow-up evidence. Passing controls, one traced failure and one concurrent lock failure are not enough to assert tracing-only, concurrency-only, baseline-equivalent or environment-only causation. Slot is released back after the evidence commit and final integration check; no further qualification workload is left running by this agent.
