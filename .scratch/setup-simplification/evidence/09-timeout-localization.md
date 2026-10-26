# Ticket 09: timeout localization

**Latest disposition:** [Owner-source resumption](#owner-source-resumption-2026-10-26) identifies a concrete Axum connection notification-before-service-destruction gap and traces it to the actual Device native resource, but does not identify the holder in historical C1. Source/artifact inspection only; no executable run, new diagnostic/test or behavioral edit. Ticket09 is open/released, unresolved. [P3 clock correction](#p3-clock-correction-and-ownership-follow-up) and [earlier focused execution](#authorized-focused-execution-2026-10-26) remain historical results, not new reproductions. Earlier NOT RUN labels describe the initial observation pass only.

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

`OF_SETUP_TIMEOUT_TIMING` is presence-based and is read only in tests; **unset it**, rather than set it to `0`, for control cells. Production logs require an installed logger allowing the dedicated debug target. Timers are local monotonic `Instant`s; elapsed time is relative to each seam, not a globally synchronized clock. Adjacent phase deltas aid diagnosis, not proof of task scheduling/CPU attribution. The initial `format_timestamp_millis` setting did **not** emit a common timestamp because env_logger's humantime feature is disabled; the P3 follow-up below corrects this. Do not infer absolute timestamps in the earlier logger captures.

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

## P3 clock correction and ownership follow-up

### Scope and verified clock correction

Continued under the user's exclusive Rust slot at `c5779ad446e42424942d7d3fef4b05409fc4ba99`; integration was still `e6c72aae991f833670cb4820471ab48ac73b6bfb`. No sibling edits, provider/reset-fixture/SDK changes, matrix or coverage. All Rust commands retained shared target, jobs=2, stack=67108864 and a 600000ms tool bound.

**P3 confirmed:** `env_logger` is declared with `default-features=false` and no humantime feature. Consequently the earlier `.format_timestamp_millis()` setting emitted lines such as `[DEBUG setup_timeout] ...`, without timestamps. Earlier per-seam `elapsed_ms` observations and syscall trace timestamps remain valid; a common logger clock must not be retroactively inferred from them.

`api_join_tests.rs` now uses one local `timing_logger()` builder with the existing fixed `off,setup_timeout=debug` filter and an explicit std formatter:

```text
unix_ms=<SystemTime since UNIX_EPOCH in milliseconds> <categorical message>
```

It uses `std::time::SystemTime`, `std::time::UNIX_EPOCH` and `std::io::Write`; no dependency/feature was added and no broad logger was enabled. Pre-epoch clock failure returns an IO formatting error rather than manufacturing an epoch. Existing monotonic per-seam elapsed fields remain the duration source. Unix time is a **wall clock**, may jump, and has millisecond granularity: it is not an ordering/causality guarantee, a request identifier, or a solution for overlapping children/interleaved stderr. Only messages routed through this logger gain the prefix; existing client/readiness `eprintln!` events still have their original per-seam elapsed times. The new specific owner-reopen event below also has an explicit Unix-ms prefix for syscall comparison.

One focused output-seam check, `categorical_timing_logger_emits_common_clock_without_humantime`, builds this actual logger locally with a captured writer, without initializing/mutating the global logger. It emits one static categorical record, requires an integer `unix_ms=` prefix and **exact** categorical suffix/newline, and checks that the timestamp lies between the surrounding std clock observations. No credentials, headers, bodies, IDs or arbitrary error messages are introduced.

TDD: first extracted the existing builder unchanged and added the focused check. Running it **failed** at the required common-clock prefix/suffix, not compilation (red build 49.19s). Replaced the ineffective timestamp setting with the explicit formatter; the **same check passed** (green build 45.18s). Final formatted-source invocation also passed, **1 passed / 0 failed**, 1.02s Cargo time, test time <0.01s. Command, with the shared environment described above:

```sh
cargo test --locked -p unified-server --features cli --lib categorical_timing_logger_emits_common_clock_without_humantime -- --nocapture --test-threads=1
```

No HTTPS timing-cell family was rerun to qualify the clock correction. The check exercises the actual logger/output path with humantime still disabled.

### Ownership investigation: what actually keeps a Device Database open

Read-only trace through current source, rather than assuming the outer Arc or the earlier unified fix explains C1:

1. `device-server::database_client::DeviceStore` contains both an embedded `db::Client` and `Option<Arc<db::Database>>`. Independent Device `run` retains a store; `observe` captures another store in its current-readiness callback; normal/Join-history routers also retain query clients and, for history, an owner Arc.
2. The callback is held by `api::CreateShared.readiness`; normal routes are held by `CreateShared.normal`. Issued setup/connection routers retain `IndependentCreateControl(Arc<CreateShared>)`. These are concrete application ownership paths. The callback takes the control as an argument rather than capturing it, so its shown construction does not establish a self-cycle.
3. `ofdb_sql::Client` embeds a **cloned Database value**, not necessarily the same outer `Arc<Database>`. The clone's file backend shares `NativeResource.inner: Arc<Inner<...>>`; its backend holds `RedbKernel → RedbDatabase → Arc<redb::Database>`. Therefore an outer `Weak<Database>` reaching zero would **not prove** the native file handle was released. Observing only that Weak would produce misleading evidence.
4. Read installed locked `redb 4.3.0` source as well: its `Database` documentation explicitly states that live write transactions defer close and keep the database file locked after the Database is dropped. Its open maps conflicting storage locks to `DatabaseAlreadyOpen`; this is a real locking failure, not an inferred missing setup secret. On this Linux build native range locks (`F_OFD_SETLK`) and the legacy flock can both matter, so a flock-only capture is insufficient.
5. Independent Device shutdown joins its offer task/protocol and endpoint; generic `api::serve_setup_inner` joins the Axum listener task. It does not have the unified worker's shared-destruction fence. This remains a candidate for retained connection/control state, **not a demonstrated identification of the owner in the C1 failure**. No source evidence establishes an ofdb leak or broken close implementation; no sibling change is justified.

A deliberate externally retained test-only router or client could trivially keep the Database locked, but would not identify the reference that survived the real awaited shutdown. No such manufactured causal regression or speculative fence/unlock patch was added.

### One bounded native-lock diagnostic — not a passing replay

Added only a categorical diagnostic at the existing immediate Device reopen in `create.rs::seed_endpoint_negatives`, behind the existing presence-based `OF_SETUP_TIMEOUT_TIMING` switch:

```text
unix_ms=<clock> seam=independent_owner owner=device phase=post_shutdown_reopen successful=<bool> error_kind=<query error kind or None>
```

The original `Database::open` and subsequent `expect("existing-only stopped owner")` are unchanged semantically. The event reports the actual open result before that assertion; it contains no path, resource ID or arbitrary error text. This helper is reached after the existing caller's shutdown cancellation and successful awaits of owner server tasks. No new retry, sleep, readiness shortcut, timeout or provider hook.

Built `owner_boundaries` with the same locked cli/no-Swagger flags as before; **no-run build passed in 1m21s**. Ran **one** diagnostic invocation of the two existing exact Create parents with `--test-threads=2 --nocapture`, timing enabled, prefixed by:

```sh
strace -f -ttt -yy -e trace=fcntl,flock,close -o /tmp/setup09-review-owner-locks.strace
```

Used the actual Cargo-reported `owner_boundaries-82e9ad7da03710d6` artifact, SHA-256 **`ade59bf7ae52bddbe9baed7d64140f4e92b7140f5d671ccf2ac36562cb3f8265`**, verified unchanged after execution. No reads/writes/send/recv payload tracing, no broad HTTP logs. Raw descriptor paths necessarily contain isolated fixture names/IDs and remain private; they are not copied into categorical evidence here. The supervisor retained 570s process-group safety cleanup, but **the existing 180s parent watchdogs fired first**. Actual outcome: **0 passed / 2 failed**, exit 101, **180.467s** supervisor time; not an outer-tool timeout and not another Database-lock reproduction. No rerun to green and no parent deadline inflation.

Before those eventual watchdog failures, both targeted post-shutdown opens succeeded:

```text
unix_ms=1793023745093 seam=independent_owner owner=device phase=post_shutdown_reopen successful=true error_kind=None
unix_ms=1793023746839 seam=independent_owner owner=device phase=post_shutdown_reopen successful=true error_kind=None
```

Grouping the raw trace by exact private Device database file (anonymous A/B here, not a mapping of concurrent parent identity) shows, near these boundaries:

| File group | Prior descriptor unlock (Unix ms) | New exclusive-lock acquisition (Unix ms) | Post-open categorical event (Unix ms) |
| ---------- | --------------------------------: | ---------------------------------------: | ------------------------------------: |
| A          |                     1793023745087 |                            1793023745092 |                         1793023745093 |
| B          |                     1793023746833 |                            1793023746837 |                         1793023746839 |

The prior descriptor close calls are also present near the unlocks; A's close entry is split by strace's unfinished/resumed scheduling and is not treated as an independently timestamped completed close. All observed `flock`/`F_OFD_SETLK` acquisition calls for these two Device files succeeded; no failed native Device lock call was captured. This gives actual native release/acquire evidence **for this run**, not proof of what held the file in the earlier C1 failure. Other phases continued until the parent bounds expired, so do not label these parents passing or call the lock issue fixed. Trace perturbation is a limit, not a reason to change bounds or an established sole cause of the watchdog failures. At diagnostic start loadavg1 was 3.36; memory PSI avg10 some/full 0.27/0.24, recorded without any environment-only attribution.

### Validation, fixed inputs and next blocker

`cargo fmt --all`, final `cargo fmt --all -- --check`, and `git diff --check` pass. Strict scoped test Clippy was run again, serially (no competing Rust qualification):

- Unified cli `--lib --tests --no-deps -- -D warnings`: still fails on the same **14 lib-test plus one full_node-target** diagnostics documented above; none is in the changed formatter/test.
- Root cli/no-default-features `--test owner_boundaries --no-deps -- -D warnings`: still fails on the same **six** diagnostics outside the changed helper/probe. The later `create.rs::wait_public` location is now line 1573 because of the new event. No unrelated cleanup/suppression.

No production lifecycle implementation was changed. Formatter and probe source were byte-stable during the owner build/diagnostic and final validation; initial/final checks matched `c5779ad44` plus binary diff SHA-256 **`fc37717e2176153b0311b6149244bd94298c0f12b8980fa59e472a9fe2ecb0b9`**. Source byte hashes:

- `crates/unified-server/src/api_join_tests.rs`: `306f75ca6a5c87ecf0b6315d57d4ca77d08fc6bfabc47e2db83abe09adf2a866`.
- `tests/owner_composition_tests/create.rs`: `ff8c0be73b10b509a1f88d0e84b9605c6a501c24a57e2c7f9263a51a7bfd08f4`.
- Final formatter-test executable `unified_server-f70d87b770f6bdb5`: `747cb5c5daa7f1195b3e2af1505f07a2fbbe7f5b8757fb030f61df6990c95b72` (new bytes, not the earlier timing-cell artifact).

Cargo.lock remained `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; siblings remained clean at the exact prior `ofdb=e3242170c94d5fc4bcc121db812310c8106c7ab6`, `ofnet=dbea925a0a3f99053fffc02618baccc2a1cbea9f`, `offs=df4c696a910296635473ec296d9235acbaa60d9b` revisions. No sibling source, package feature/dependency, actual profile, OS trust or provider was modified.

Private mode-0600 receipts:

| Artifact                                                 | SHA-256                                                            |
| -------------------------------------------------------- | ------------------------------------------------------------------ |
| `/tmp/setup09-clock-red.log`                             | `dc5736c1787f30f7ed85db2258c513dce3389110bdb69532e51b117e91b0e1e0` |
| `/tmp/setup09-clock-green.log`                           | `dc04d90d3747b587c0204101f166e99e7f7817d07e3c09d3f95b9e8e124b28c4` |
| `/tmp/setup09-clock-final.log`                           | `69be587a42520180fbf241e6ff5f9abc77f7db694bfb10681fcb605336655c78` |
| `/tmp/setup09-review-owner-locks.log`                    | `9d0bae786700325b419c0a0a8d610b7d468d43091b44752e8a9178126cb67261` |
| `/tmp/setup09-review-owner-locks.strace` (3068960 bytes) | `eb919184a3103ea9b56275ab9ac1aaaa39fcf4cd288b4c516c6144ff5bd73618` |
| `/tmp/setup09-review-owner-locks.json`                   | `86bd101a46d38c4f2cf1d3f6a556a1760a8b2d92b048757350f67b5cc8f4c3bc` |

`/tmp/setup09-review-artifact-manifest.json` SHA-256 **`1ccb2602ecb13da8e4da7a6d426a95ab850178926c6c297103f0ca0c647c523d`** records all 11 review capture files, including build/Cargo artifact JSON, source-input receipt and both Clippy logs. No matrix/coverage or retry-to-green was run.

**P3 is corrected and focused red/green verified. Ticket09 remains unresolved. Precise next blocker:** capture a failing immediate post-shutdown Device reopen together with the lifetime of the surviving **inner NativeResource/embedded-client handle or redb write transaction**, then construct the corresponding deterministic gated shutdown/reopen regression. Current application-level router/control ownership and native syscall evidence do not distinguish those holders in the failed historical run. There is no evidence yet assigning the defect to ofdb, and no safe basis for changing independent lifecycle ownership or forcing a Database close. The earlier approval overrun and terminal readiness/permission history also remain separate unresolved boundaries. This block leaves only the timestamp correction, focused formatter check, categorical reopen probe and evidence; no speculative Database fix.

## Owner-source resumption, 2026-10-26

### Scope, provenance and what is new

Claimed ticket09 before investigation in `.worktrees/setup-09-owner`, branch `implement/setup-followup-09-owner`. Clean entry HEAD, integration tip and merge-base were all **`1cae4e81bed55bff4acc93eb4561250dad51d6da`**, confirming the requested base. Read AGENTS, issue-tracker instructions, spec, ticket09 and this evidence; loaded TDD and ponytail. Complexity/risk/priority: medium ownership analysis, low documentation risk, high risk for any premature lifecycle change; prioritize identifying the actual native holder over another passing replay.

**Source and existing-artifact inspection only.** User reports about 6.6GiB available RAM with swap full; parent owns resource preflight. No Cargo, test executable (including `--list`), formatter, Clippy, matrix, CRAP, debugger, tracer or build ran. No resource-based causal attribution. Writes are restricted to this ticket and evidence in this worktree; no map/spec/ticket11, sibling, provider/profile, bounds/retries/sleeps or production behavior changes.

Read-only `ofdb` snapshot remained clean at `e3242170c94d5fc4bcc121db812310c8106c7ab6`. Main Cargo.lock SHA-256 remained `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`. Locked installed sources inspected: Axum **0.8.9**, Hyper **1.11.1**, hyper-util **0.1.20**, Tokio **1.53.1**, redb **4.3.0**. Registry-relative pointers below refer to those exact installed versions, not latest versions.

Existing `/tmp/setup09-C1-owner-1.log` was inspected without executing anything: SHA-256 still **`5e5ec152365504437bc8905560251358c7184621208724f4b3f8cbfee657eea7`**, one `Database already open. Cannot acquire lock.` line, **zero** later `post_shutdown_reopen` probe lines. This confirms the old artifact lacks the later probe; it is not a new failure or holder capture. The previous successful traced reopens and transaction-retention documentation are prior findings, not new runtime evidence.

**New source result:** the generic listener's missing destruction fence is now narrowed to a specific notification-before-destruction window in its locked Axum/Hyper dependencies, with a complete real router → native resource chain. This proves a reachable ownership-order gap in the source, **not that this window was occupied by the surviving holder in historical C1**, nor a deterministic root cause of the observed failure.

### Actual shutdown and retained-resource chain

All main-project pointers below are relative to this worktree at the confirmed base; no Rust file changed in this pass.

| Boundary                       | Exact source and ownership consequence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Actual failure seam            | `tests/owner_composition_tests/create.rs:943–956` waits for current readiness, awaits `normal_use`, cancels shutdown, successfully awaits all three owner server handles, then calls `seed_endpoint_negatives`. That helper's first Database open is `:1079`; diagnostic is `:1080–1090`, assertion `:1091`. The later `review_transfer_restart_tests::qualify` has **not started**. The helper has not yet created its own Device client/transaction.                                                                                                                                                 |
| What the parent actually joins | `create.rs:111–175` creates the three `serve_independent_create` tasks; Device wrapper is `:151–162`. `device-server/src/independent_create.rs:159–172` joins both its serving future and spawned worker on normal completion, whichever wins the select. It does not intentionally detach that worker on this successful path.                                                                                                                                                                                                                                                                        |
| Worker-owned values            | `independent_create.rs:180–215` opens and retains the real `DeviceStore`, identity and runtime, installs readiness, and publishes the runtime router on completed reopen. On cancellation, `:236–242` awaits runtime shutdown and endpoint close. `independent_join.rs:43–57` joins the offer task and protocol; the runtime's remaining router/source fields are then dropped. Merely adding another await of this already-joined worker does not address a detached HTTP connection.                                                                                                                 |
| Listener-owned values          | `api/src/independent_create.rs:164–203` installs cloned `IndependentCreateControl` in actual setup/fallback routes, then passes the host as `serve_setup`'s guard. `api/src/setup_server.rs:115–121,147–164` joins the task that awaits Axum graceful shutdown (with the existing 10s error path). A successful listener join is not a join of every detached Axum connection task's destructor.                                                                                                                                                                                                       |
| Connection → control           | Axum `src/serve/mod.rs:371–389` creates the connection's Tower/Hyper service and spawns a detached task. Axum `src/routing/into_make_service.rs:35–36` clones the real router. hyper-util `src/service/glue.rs:20–40` stores that service and clones it for requests. Thus even an idle keep-alive connection can retain the setup router/control after its last request body is consumed; a live SQL operation is not required for this branch.                                                                                                                                                       |
| Control → Device owners        | `api/src/independent_create.rs:60–73` stores `normal` and `readiness` in `Arc<CreateShared>`. Device `independent_create.rs:316–352` readiness captures the real store, cloning it per check. `:386–427` builds normal routes using its embedded client and owner. `independent_join.rs:98–143` puts Device clients in the plane/source and owner in history routes; `router/state.rs:16–23` retains selection policies/control plane. `api/src/independent_create.rs:624–637` dispatches through a cloned normal router. These are actual installed routes, not an artificially retained test handle. |
| Device store → native lock     | `device-server/src/database_client.rs:50–70` holds both `Client` and optional outer `Arc<Database>`. Sibling `ofdb/crates/sql/src/client.rs:38–52`, `database.rs:71–95,394–417` show each embedded client owns a cloned Database whose file backend is `NativeResource`. `ofdb/crates/btree-redb/src/resource.rs:14–25` shares `Arc<Inner>` containing the backend. `ofdb/crates/btree-redb/src/database.rs:14–26` ultimately shares `Arc<redb::Database>`. A control/router survivor therefore keeps the actual native backend alive, not just an unrelated application Arc.                          |

### Exact notification-before-destruction window

1. Axum `src/serve/mod.rs:389–414`: detached task constructs `let mut conn = pin!(builder.serve_connection_with_upgrades(io, hyper_service))`, polls it until completion, then explicitly executes **`drop(close_rx)` while `conn` is still in scope**. Its pinned connection is destroyed on leaving that scope, after the notification.
2. This is not necessarily a connection object whose service was already emptied by polling. Hyper `src/server/conn/http1.rs:507–570` has `UpgradeableConnection.inner: Option<Connection<...>>`; on **ordinary shutdown** its poll returns `Ready(Ok(()))` without taking `inner`. Taking `inner` is the separate upgrade branch. Hyper `src/proto/h1/dispatch.rs:48–51,144–164` retains `Server.service` and returns `Dispatched::Shutdown` without taking the service. hyper-util `src/server/conn/auto/mod.rs:652–708` forwards the H1 completion without replacing the connection state. H2 also has a stored service (`hyper/src/proto/h2/server.rs:83–95`); no assumption about which HTTP version historical C1 used is needed or established here.
3. Tokio `src/sync/watch.rs:1013–1021`: dropping the last receiver immediately notifies waiters of `Sender::closed()`. Axum's listener (`src/serve/mod.rs:296–303`) awaits precisely **`close_tx.closed()`**, not each connection JoinHandle or completion of its destructors. On a multithreaded runtime the waiter may proceed while the notifying connection thread is still before/during `conn` destruction.
4. Therefore the listener guard/worker may be gone and their outer joins successful while a detached connection's real service still holds `CreateShared` and the Device native resource. Dropping that service can itself trigger the final backend close; notification still precedes completion of that close. The small window is consistent with intermittent immediate-reopen failure and successful slower traced reopens, **but consistency is not reproduction or proof of historical causation**.

The previous evidence's generic missing-fence candidate is not being counted as a new discovery. The additional finding here is the exact dependency-level ordering, the surviving service on ordinary H1 shutdown, and the real Device native-resource path that can cross that boundary. No inference that all approval/readiness timeouts share this cause.

### Transaction alternative and limits

`ofdb/crates/sql/src/database.rs:433–444` wraps an actual engine transaction in `resource.transaction(...)`; `btree-redb/src/resource.rs:280–291,422–444` shows that wrapper **also retains the NativeResource owner**, plus an Arc slot containing the transaction. The registry in `State.transactions` (`:27–33`) is weak, not an owning cycle. `btree-redb/src/database.rs:35–43` begins the redb write transaction. A live transaction can thus keep the native resource alive independently of the outer Database Arc; an empty/consumed transaction slot must not be reported as a live redb writer. redb's documented deferred close is still a distinct lower-level possibility (`redb/src/db.rs:744–748,2012–2024`), not demonstrated in C1.

The shown embedded query/transaction paths do not offload into a newly spawned database worker. The normal joined Device worker and offer task cannot be called the missing native holder simply because they once owned clients. The Join-history stream does use `WebSocketUpgrade::on_upgrade` (`device-service/src/join_history.rs:194–231`), which has separate lifetime implications, but this local Create/reopen boundary does not call that history-stream route; do not manufacture a WebSocket session to explain this failure. Source inspection is not a general proof against every possible cycle, cancellation path or transaction survivor.

### Minimal next capture — NOT AUTHORIZED / NOT RUN

No additional source diagnostic was added: an outer-Arc Weak/drop print cannot distinguish embedded-client/native-transaction holders, while existing syscall probes already locate native release without identifying owners. A new passing replay or a deliberately retained router/client would not close the evidence gap. No new test seam is proposed; the existing public independent Create parent and immediate existing-only reopen remain the seam for a later causal regression.

After **explicit parent resource/slot authorization**, the smallest useful next step is one failure-triggered, private ownership capture of the **existing** Create flow, not another broad cell matrix:

1. Verify fixed current main/sibling/lock inputs and actual executable provenance first. Inspect debugger symbols/line tables before choosing probes; prior binary filenames are mutable and the C1 bytes are not current-source qualification. If compilation is required, request a separately authorized bounded serial build; do not run Cargo merely to find an executable. Do not run any build and test concurrently.
2. Prefer debugger observations without sibling edits. Record the **real** Device store's `NativeResource.inner` and underlying redb object at `independent_create.rs:192–208`, and track their destruction via available typed drop symbols. Observe, without retaining/upgrading an Arc, the real Axum connection service/control at `axum/src/serve/mod.rs:414`, listener completion at `api/src/setup_server.rs:153–164`, and existing reopen result at `create.rs:1080`. These are probe locations, not an unverified runnable debugger script. If debug information cannot expose live ownership/drop state, stop: syscall-only evidence or thread backtraces alone cannot distinguish suspended async owners. Native-resource instrumentation in a sibling requires separate ownership/write authorization, not an opportunistic edit here.
3. On an **actual failing** open, preserve the fixture before ordinary parent cleanup: capture the original and attempted descriptors' device/inode and `/proc/<child>/fdinfo`/native lock records, together with the observed native-resource/control lifetime. Retain raw paths/pointers privately; publish categorical summaries only. Inspect live `State.transactions` weak-slot liveness and whether a slot still contains a native transaction, without upgrading or otherwise extending it. Distinguish (a) connection service still owns control/native backend with no live native transaction, (b) transaction/native writer survives, and (c) different process/open instance holds the same inode. A syscall conflict or positive native Arc count alone does not identify which owner is responsible.
4. Keep existing 180s parent/10s listener/request/readiness bounds; preserve child isolation/providers and process-group cleanup. A debugger must not leave a stopped child past its existing watchdog. Predeclare **one invocation** and no retry-to-green. The historical failing scheduling cell selected five parents with threads=2; a smaller Create-only attempt changes scheduling and is a diagnostic reduction, not an equivalent reproduction. Record debugger/tracer perturbation separately; if reopen succeeds or the watchdog fires, report that outcome and stop rather than changing bounds.
5. Only if the failure capture ties the conflict to the real notification-before-service-destruction path, report that root cause before behavior edits, then agree the smallest deterministic regression at this existing seam. Any gate must synchronize the **actual** connection teardown seen in that capture, not create an externally retained database/client. If a transaction survives instead, follow that actual owner before designing a fix. Do not force unlock, retire/delete the database, clear shared routers speculatively, add sleeps/retries, or copy the unified fence solely by analogy.

### Disposition and validation

Documentation-only result. `git diff --check` is the only change validation; no Rust validation is claimed or permitted in this pass. No deterministic failing holder capture, red/green regression, lifecycle fix, feature-matrix or CRAP result. Acceptance boxes remain unchecked; ticket09 is **open/released**. Other timeout families remain unresolved and ticket07 is untouched. The source gap warrants the targeted capture above, not an unvalidated behavioral patch.

## Independent connection destruction fence, 2026-10-26

### Claim, authorization and fixed inputs

User explicitly authorized executable progress at the existing independent owner shutdown/reopen seam, including a deterministic real connection teardown gate and the existing lifecycle precedent rather than another source-only handoff. Claimed ticket09 on `implement/setup-09-fence` in `.worktrees/setup-09-fence`; clean entry HEAD, requested base and integration tip were all **`8abf464c800a5750f433bae67177bcc11bef956f`**. Loaded TDD/ponytail; read AGENTS, spec, ticket/evidence09 including latest owner-source resumption, and evidence07's deterministic fence and queued-reply regressions. Complexity/risk/priority: medium implementation complexity, high lifecycle risk; fix the proved destruction ordering without changing worker orchestration or attributing all historical failures to it.

Actual unchanged dependency inputs:

- `ofdb`: `e3242170c94d5fc4bcc121db812310c8106c7ab6`, clean.
- `ofnet`: `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, clean.
- `offs`: `df4c696a910296635473ec296d9235acbaa60d9b`, clean.
- Cargo.lock SHA-256: `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`, unchanged. Actual locked build includes Axum 0.8.9, Hyper 1.11.1, hyper-util 0.1.20, Tokio 1.53.1 and **Iroh 1.2.0**; do not infer Iroh 1.1 from manifest lower bounds or prior prose.
- `rustc 1.99.1 (58cf8f9c3 2026-10-13)`; unchanged project profiles (`dev` opt-level=1, dependencies opt-level=3, ordinary debug info).

Initial worktree target was absent. The main `of/target` had reappeared at inspection, only **932 MiB**, while an editor-owned `cargo check --workspace --all-targets` was active. It exited before this agent's successful compilation launch; no unrelated process was killed. This was not the old complete target. Builds explicitly used `/home/nathan/code/rust/aicacia/of/target`, preserving whatever artifacts existed and continuing productive compilation in bounded stages. No cache deletion or profile/environment debug/optimization override.

The three supplied `.worktrees/{ofdb,ofnet,offs}` symlinks initially resolved one level too high (`../../../ofdb` → `.../rust/ofdb`). First Cargo invocation failed loading the missing dependency manifest **before compilation**. Corrected only these worktree dependency links to `../../{ofdb,ofnet,offs}`; no sibling repository edits. No spec/map, ticket07/11, provider/profile/OS trust or security changes.

### Deterministic real-connection regression: red before green

Regression: `setup_server::teardown_tests::independent_shutdown_joins_actual_connection_teardown_before_reopen` in `crates/api/src/setup_server_teardown_tests.rs`.

This uses the existing independent `IndependentSetupHost::prepare` → `into_create` → actual `host.router()` → shared serving boundary → shutdown → single retained-host reopen. Its transport is neutral HTTP for deterministic teardown, not production TLS/provider provisioning. A generic-listener extraction initially preserved the old `axum::serve(...).with_graceful_shutdown(...).await` behavior exactly. The production TLS listener and test listener call the same extracted function.

The test makes and fully consumes a **real HTTP/1 keep-alive request** to the installed independent status route. Its accepted TCP IO wrapper gates only that connection's **actual destructor** with a Condvar after cancellation. It does not manufacture or externally retain a database, client, control, router or native transaction. Axum's close notification occurs before this destructor; the gate therefore makes the dependency-level ordering window deterministic. The gate has RAII release, including assertion/timeout unwind, and runs with two Tokio worker threads; the five-second whole-test bound detects hangs, not a wait-before-reopen or increased production deadline.

The test requires serving to remain pending while the actual connection destructor is blocked, requires the existing owner exclusion to remain held, then releases the gate and awaits serving. One immediate `IndependentSetupHost::prepare` must succeed with byte-identical retained admission. No sleeps, retries, scheduler-yield loop or externally held routing/native owner is used.

Command, cwd this worktree (all Cargo commands below use `--locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1`):

```sh
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p api --lib independent_shutdown_joins_actual_connection_teardown_before_reopen -- --nocapture --test-threads=1
```

**Red, before fence:** exit 101, **0 passed / 1 failed**, **0.09s**:

```text
shutdown escaped actual connection teardown: Ok(())
```

Before reaching executable red, the first compilation stage hit its predeclared stage ceiling; the next found two test-plumbing compile errors (missing dev-only multi-thread runtime feature and missing generic listener address Debug bound). Corrected those before the actual failing execution. The executable red build then completed in **4m21s**. Compilation errors/stops are not counted as behavioral red.

### Minimal production fix and ownership review

Only `crates/api/src/setup_server.rs` changes production behavior. `SetupConnection` forwards the same real Router service and owns a private watch sender **after** the router in declaration order. Its clones are the services installed in Axum's actual accepted connections; request dispatch is unchanged. The sender is never sent on or exposed. Shared serving awaits channel closure after Axum's graceful listener future, **while still retaining the original lifecycle guard**. Closure therefore follows destruction of all connection service routers, rather than merely Axum's earlier close notification. The guard is then destroyed before the serving task returns.

Unlike waiting for all independent controls to disappear, this fence does not require a still-running owner worker to release its control before the listener can finish. It fences the actual serving services; existing independent wrappers still cancel and join their owner workers in their original order. No control-cycle cleanup, forced native unlock, premature normal-router clearing or speculative worker rewrite. Existing command admission/cancellation/drain behavior is untouched. The previously fixed unified worker's queued-reply handling is also untouched.

The existing production **10-second** listener shutdown bound still encompasses both graceful shutdown and the destruction fence. An error/timeout is not upgraded to successful shutdown; existing task abort/error handling remains. This does not newly claim to join arbitrary upgraded WebSocket sessions or unrelated native transaction owners. Those are distinct owners and the local Create seam does not invoke the Join-history WebSocket route.

**Green:** exact regression passed, **1/1**, **0.04s**, **29.94s** rebuild. Initial fix compilation required an explicit `IncomingStream<'_, L>` closure annotation; that compile error was corrected, not counted as a failed runtime test. A subsequent lint-only `_teardown` field rename removed the dead-code warning before full API validation. Added only a dev feature on the existing Tokio dependency (`rt-multi-thread`); no new package or lock change.

### Focused validation, including actual production Device reopen

| Command / scenario                                                                                                                                                          | Actual result                                                                                                                                                                                                                                                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact new regression, unfixed implementation                                                                                                                                | **Failed**, 0.09s, exit 101, real shutdown returned before connection destruction.                                                                                                                                                                                                                                                                       |
| Same regression, fixed implementation                                                                                                                                       | **Passed**, 0.04s, exit 0.                                                                                                                                                                                                                                                                                                                               |
| `cargo test ... -p api --lib -- --nocapture --test-threads=1`                                                                                                               | **42 passed / 2 child-only ignored**, 8.21s, 26.74s rebuild. Parents actually exercised isolated installation cancellation/future-drop/abort children and native-root delayed-readiness child; all passed. Includes independent admission, cancellation, exact identity/recovery, CORS and trust/reset refusal regressions.                              |
| `cargo clippy ... -p api --lib --tests --no-deps -- -D warnings`                                                                                                            | **Passed**, 1m18s.                                                                                                                                                                                                                                                                                                                                       |
| `cargo fmt --all -- --check`; `git diff --check`                                                                                                                            | **Passed**.                                                                                                                                                                                                                                                                                                                                              |
| `cargo test ... -p idp-unified --no-default-features --features cli --test owner_boundaries --no-run`                                                                       | **Passed** after three productive bounded-stage stops; final stage 7m29s. No Swagger UI/default feature build.                                                                                                                                                                                                                                           |
| Fresh owner executable, `--exact owner_composition_tests::create::production_independent_create_and_offline_reopen --nocapture --test-threads=1`, `RUST_MIN_STACK=67108864` | **1 parent / 1 child passed**, child **112.60s**, parent **112.76s**. This is the existing production HTTPS Create/current-readiness/normal-use/awaited owner shutdown → immediate Device native Database reopen path, plus its retained restart/negative checks. No test-body changes. One invocation, not a retry or repeat reliability qualification. |

Only rustdoc/field-comment clarification and ticket/evidence text followed executable validation; production behavior and test code were unchanged. Final formatting/diff checks cover these text changes.

The deterministic regression proves the independent serving ownership defect using the actual connection and real host/admission exclusion. The separate existing production scenario supplies an executable check of the real Device embedded-client/native-store path. It is **not** a deterministic before/after reproduction of historical C1's native lock conflict. No native holder in the historical failing process has been retroactively identified.

### Resource protocol and local receipts

Every individual tool call was below **600000ms**; Cargo/scenario children ran in new owned process groups, at jobs=1, supervised by `/tmp/setup09-fence-monitor.py`. The monitor sampled actual MemAvailable, owned-PGID aggregate RSS and memory PSI every 10s (15s in initial build stages). Stop conditions: MemAvailable <2 GiB, some avg10 ≥20 or full avg10 ≥10 for three consecutive samples, or stage elapsed >550s. Stops terminate/reap only the owned PGID, with TERM then KILL if needed. Full swap alone did not prevent productive launches.

- Initial red build stage and owner-build stages 1–3 stopped at the **stage time ceiling**, not a memory/PSI threshold. Completed artifacts were retained for subsequent stages; no concurrent builds/tests.
- Saved resource receipts across executable red/green, API tests, Clippy and owner build/execution have sampled minimum MemAvailable **5159 MiB**. Owner final-link stage peaked at **4,762,060 KiB aggregate owned RSS**; maximum sampled some/full avg10 **7.59/7.00**, below the sustained stop thresholds. No RAM/PSI stop was triggered. First launch reported 6694 MiB available; initial preflight had only 4426 MiB. This is sampling, not a guarantee about between-sample peaks or global idleness.
- Build/runtime output: `/tmp/setup09-fence-{red-stage3,green2,api-tests,clippy,owner-build1,owner-build2,owner-build3,owner-build4,owner-create}.log`; corresponding `-resources.log` files preserve samples. Earlier initial-stage output uses `red-build.log`, `red-build-stage2.log` and `red-resources-stage2.log`.
- Red log SHA-256: `4950a38d1fbaaa418539419d34f427c5a047d4531c6ef04d8d1269cbd0cf96b8`.
- Green log SHA-256: `f262b5c1981d94911818b045d58bcb5337ed0a79d8d66a3bce58ef14d08f55df`.
- Full API log SHA-256: `80cdf8e8febb1d095461d666c50cc3aa3af90a9cb72ebc91da1ad9ef2141b2a8`.
- Production Create log SHA-256: `eac6e2ca61ac8646fa0267718452345d3f59465092dc47c050848e5c50c9f4a4`.
- Final API test executable `api-27c60d16edd1879f`: SHA-256 `fde37b51d01f026953aa4ad4e6f816ad2fa5c8744aa985b8f891492931a47d80`.
- Fresh production owner executable `owner_boundaries-82e9ad7da03710d6`: SHA-256 `39e73bf94835b69f6755aca23b85670bde9abda87e822072b5e0bfc29ca01f48`. This mutable filename is **not** the historical C1 executable bytes.

### Disposition: implemented defect, still-open ticket

The actual independent listener/connection destruction gap is now fixed and has executable failing-before/passing-after evidence, plus one passing production native-reopen scenario. This is implementation progress, superseding the preceding source-only handoff for this defect. Preserve the distinction between that proved defect and historical attribution.

Ticket09 remains **open/unresolved**, with aggregate acceptance boxes unchecked. Remaining concrete work:

1. Localize the distinct source-offer/current-readiness terminal timeout family and ordinary authorized HTTPS permission/request failures at fixed inputs, with categorical timing and a causal failing regression before any behavior change. The traced approval path's cumulative >5s work remains only partial localization; operation-lock wait was zero, not a diagnosed lock bottleneck.
2. Historical C1 attribution, if still required, needs a real failing-holder capture distinguishing connection service, live native transaction and another open/process. The new deterministic fence is not such a capture.
3. Complete repeated finalization, interruption/exact retry and source-offline qualification with final integrated code, separating serial/concurrent/tracing conditions. A single Create smoke and the earlier Join results do not close these aggregate gates.
4. Broader feature matrix/coverage/CRAP and platform qualification remain parent-coordinated; **not run here**, per explicit instruction. Ticket07/map/spec/11 remain unchanged. No sleeps/retries/deadline inflation or security relaxation is proposed for these remaining tasks.

## Current-code untraced qualification and bounded closure, 2026-10-26

### Predeclared execution and immutable inputs

Resumed the same branch/worktree under the exclusive Rust slot. Fast-forwarded to requested integration **`734e65070ec0dfa7dcc1d03934ab7c63ed8ca456`** before work; reclaimed ticket09 and loaded TDD/ponytail. Parent prepared ticket11 read-only and did not own a competing build. This slice executes existing public scenarios and causal regressions; **no Rust behavior/test/dependency changes** were necessary or made.

Before compilation/execution, declared **one S cell (serial, threads=1)** and **one C cell (concurrent, threads=2)** in ticket09. Each cell contains separate non-overlapping unified/owner subgroups. Order: S-unified → S-owner → C-unified → C-owner. Same binaries, filters, feature choices and environment throughout; no retries or additional diagnostic/tracing cells. A subgroup failure would have stopped automatic progression for diagnosis; none failed. Libtest parents, not ignored children, were selected; all parents exercised their isolated children normally.

- Unified filters: `production_api_join_`, verified by executable `--list` to select exactly `local_provisioning_tests::https_bootstrap_tests::{production_api_join_complete_unretired_finalization,production_api_join_completion_and_offline_reopen,production_api_join_interruption_preserves_private_host_intent}`.
- Owner filters: the same five exact parent names declared above: production independent Create/offline reopen; unsupported-record refusal/current reset; signer-free operation/delayed-authority reopen; remote Create/Join/reopen/Device reset; ordinary independent permission via production Management HTTPS.
- Both cells **untraced**. `OF_SETUP_TIMEOUT_TIMING` and `RUST_LOG` explicitly removed from the child environment. Existing default categorical production stage logging remains enabled by the scenarios. `RUST_MIN_STACK=67108864`, `CARGO_BUILD_JOBS=1`; no other optimization, deadline or security override.
- Main and sibling HEADs, clean sibling status and lock hash matched the preceding fence implementation's recorded dependencies. Main HEAD was `734e65070`; only the ticket claim differed from it during execution. A before/after manifest verified identical source status and all four HEADs. Production `Cargo.toml`, Cargo.lock, `crates/`, `src/`, `tests/` had no diff. Lock SHA-256 remains `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`.
- Toolchain remains rustc **1.99.1 (58cf8f9c3 2026-10-13)** / cargo **1.99.1 (5f94df478 2026-08-27)**. Main target existed at **19 GiB**; used it explicitly, without deletion/profile changes. No broad feature matrix, coverage, CRAP, real provider/OS trust or sibling edits.

Build commands (cwd this worktree) both passed at jobs=1, ordinary unchanged profiles:

```sh
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p unified-server --features cli --lib --no-run --message-format=json
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --no-default-features --features cli --test owner_boundaries --no-run --message-format=json
```

Unified build **539.622s** supervisor wall / Cargo 8m58s; owner build **37.046s** / Cargo 36.84s. Neither hit its resource/stage bound. Unified emitted the same three unused shared test-support warnings. No Clippy rerun was needed for this documentation-only source delta; preceding strict changed-area results stand separately.

Actual artifacts resolved from successful Cargo JSON events, verified with `--list`, hashed before execution and rechecked unchanged after both cells:

| Executable under main `target/debug/deps/` | SHA-256                                                            | Actual target features                           |
| ------------------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------ |
| `unified_server-f70d87b770f6bdb5`          | `88b22ad448d01de951e2e5f5761e89855a6374a5789519cc0b08334a1364d75e` | `cli, default` (no Swagger UI)                   |
| `owner_boundaries-82e9ad7da03710d6`        | `39e73bf94835b69f6755aca23b85670bde9abda87e822072b5e0bfc29ca01f48` | `cli, unified-server` (no root default features) |

### Actual public-scenario results

Commands execute the above binaries directly: unified `production_api_join_ --nocapture --test-threads=N`; owner all five full names with `--exact --nocapture --test-threads=N`. Full argv and process-group/resource/exit receipts are saved per subgroup. Existing scenario parent/request/readiness/listener bounds are unchanged.

| Cell / subgroup | Result                                  | Supervisor wall seconds | Max sampled group RSS KiB | Kernel max individual child RSS KiB | Min available KiB | Max observed memory PSI avg10 some/full |
| --------------- | --------------------------------------- | ----------------------: | ------------------------: | ----------------------------------: | ----------------: | --------------------------------------- |
| S / unified     | **3 parents + their 3 children passed** |                 263.323 |                    95,548 |                              80,584 |         6,367,272 | 0.80 / 0.75                             |
| S / owner       | **5 parents + their 5 children passed** |                 394.205 |                   150,072 |                             135,040 |         6,255,584 | 0 / 0                                   |
| C / unified     | **3 parents + their 3 children passed** |                 178.320 |                   175,076 |                              80,584 |         6,361,916 | 0 / 0                                   |
| C / owner       | **5 parents + their 5 children passed** |                 229.145 |                   260,852 |                             134,324 |         6,200,848 | 0 / 0                                   |

Thus **6/6 current-code Join parent executions** cover finalization, interruption/exact retry and completion/source-offline reopen, each once under each scheduling condition. **10/10 independent-owner parent executions** cover the four readiness/deployment representatives plus the separate ordinary HTTPS permission representative under both conditions. Serial child times: Join 89.19/90.39/83.45s; independent owners 116.91/119.90/68.20/22.97/65.73s in test-name order. Concurrent owner completion order differs, as expected. No failed scenario was rerun to obtain these results.

All four runtime logs contain zero reported `panicked`, `timed out`, `timeout` or `Database already open` messages. This supplements, rather than replaces, the scenario assertions. Production Create/Device native reopen assertions actually ran; their optional diagnostic probe text was absent because timing instrumentation was disabled.

Default Join `public_bootstrap` phase timings were **49,827–51,174ms serial** and **50,810–54,875ms concurrent**. These are whole bootstrap stages, **not** the duration of an individual approval HTTP request. Current ordinary authorized approval and permission requests satisfied their existing client assertions/bounds; instrumentation-disabled cells do not supply an exact current per-handler timing decomposition, and none is invented.

### Current causal-regression checks

After the declared cells passed, ran the existing deterministic ownership regressions as separate focused checks, **not additional scenario repetitions**:

- Current unified executable, exact `setup_host_tests::shutdown_joins_router_teardown_before_releasing_host_and_admission` and `setup_host_tests::shutdown_rejects_queued_request_before_joining_router_teardown`: **2 passed**, 0.06s test / 0.090s supervisor. Preserves both the original ownership fence and queued-reply cancellation/drain protection.
- `cargo test` with the same locked target/jobs=1 flags, `-p api --lib independent_shutdown_joins_actual_connection_teardown_before_reopen -- --nocapture --test-threads=1`: **1 passed**, 0.06s test / 4.758s supervisor (4.55s rebuild). API executable hash remains `fde37b51d01f026953aa4ad4e6f816ad2fa5c8744aa985b8f891492931a47d80`.
- `cargo fmt --all -- --check` and `git diff --check`: **passed**. Final edits are ticket/evidence only.

These green executions complement the recorded **actual red-before-green** ownership regressions, not a newly inferred cause based solely on passing repetitions. With no failing current-code ordinary scenario in the declared cells, no speculative timeout/performance/security change was made.

### Resource supervision and reproducible receipts

`/tmp/setup09-qual-monitor.py` creates a fresh owned PGID for each build/subgroup, samples MemAvailable, aggregate owned-group RSS and full `/proc/pressure/memory` at approximately **one-second** intervals, and saves samples plus a JSON exit/resource receipt. Also records kernel `RUSAGE_CHILDREN.ru_maxrss`: maximum individual waited child/descendant high-water RSS, **not** simultaneous aggregate group memory. Aggregate RSS peaks in the table are actual sampled peaks; between-sample group peaks and shared-page accounting are not claimed as exact physical RAM usage.

Stops: MemAvailable <2 GiB; some avg10 ≥20 or full avg10 ≥10 sustained ≥20s; or elapsed >550s. TERM then KILL/reap applies only to the owned PGID. All tool calls bounded below 600000ms. **No resource or stage stop fired**, and no unrelated process was killed. Swap remained full, but actual memory/PSI permitted safe jobs=1 compilation and execution.

Build peaks/minima: unified sampled group RSS **4,031,864 KiB**, kernel max child **4,707,392 KiB**, min available **4,854,040 KiB**, max some/full avg10 **6.81/6.39**; owner sampled group RSS **5,237,952 KiB**, kernel max child **4,890,172 KiB**, min available **5,195,508 KiB**, max some/full **4.73/4.55**. Runtime minima/peaks are separate in the table; no resource-cause attribution is made. Background editor/system processes existed, so serial is a no-competing-build control, not a globally idle-machine certification.

Local fixed-input manifest: `/tmp/setup09-qual-artifacts.json`. Each `/tmp/setup09-qual-<name>.json` contains full argv, PGID, exit, wall time, memory/PSI maxima/minima and log hash; `-samples.jsonl` preserves all resource samples. Logs and SHA-256:

| Name                  | Log SHA-256                                                        |
| --------------------- | ------------------------------------------------------------------ |
| `S-unified`           | `69959a5333c68761d6c768d7aa6c6b4a238be64813e7b1b324d0dec293e81acb` |
| `S-owner`             | `6dca8bf5b440e4d5e9a6942ae43747ce246543e45c866c90a3666be18a4e4aec` |
| `C-unified`           | `01d7df0f59788785391b6be3d0a9cabd9b5be1741f3037a595a5a5efd1d46bc0` |
| `C-owner`             | `e2ea95691646e03e91e0375ca53c4e2877fd1dd3edc808173a7432aa1aa9e93c` |
| `unified-regressions` | `62e5b5d262fc2289fb5f0511bb641f3d8be82b1a8ff163df08b88f02232e09e0` |
| `api-regression`      | `f4b4fd83f35876eb5c764c0209016410e1347bcd46c752022d53a6acf8a85a8c` |

All names use `/tmp/setup09-qual-<name>.log`. Build receipt/log names are `unified-build1` and `owner-build1`. Logs retain ordinary scenario output without enabling payload/header/body tracing or exposing setup codes/passwords/bearer tokens in this evidence.

### Acceptance and residual uncertainty

**Ticket09 resolved for the declared current-code ordinary-behavior investigation/qualification scope.** A concrete lifecycle defect has a demonstrated causal ordering, actual failing regression, minimal fix and current green regression. The public ordinary scenarios requested by the user are now executable **16/16 passing** on one fixed integrated source/artifact set across the predeclared serial/concurrent cells, with **3/3 current lifecycle regression checks** passing. No concrete still-failing current-code ordinary scenario remains in this scope. These are two executions per selected scenario under different schedules, **not three idle serial repetitions**, a complete feature matrix, a statistical reliability guarantee or production platform qualification. Older passes are not pooled into the current counts.

Historical uncertainty remains explicitly unresolved: this does **not** identify C1's historical surviving native holder, prove every old source-offer/readiness/permission timeout shared the fixed destruction defect, or establish a root cause for every archived failure. Those facts cannot be reconstructed by another passing replay; they are retained diagnostic limits, not a fabricated still-failing current scenario or grounds for speculative patches.

The earlier **instrumented** approval failure remains a recorded failure: client deadline 5001ms, server handler success 5439ms, operation-lock wait 0ms, cumulative enrollment/verification/refresh/reply work. That phase evidence demonstrates where the instrumented request crossed the existing client bound, **not** that tracing is the sole cause of all historic timeouts or that ordinary approval needs a larger bound. Current normal serial/concurrent approval succeeds without tracing; no current traced pass is claimed, tracing was not repeated here, and no bound was relaxed. Further tracing-induced slowdown alone is not authorization for a production behavior change. A new ordinary failure would need its actual phase/owner evidence and a focused deterministic failing regression before a fix.

Broader matrix/coverage/CRAP, GUI/native platform acceptance and parent integration gates remain separate. Ticket07/map/spec/11 are untouched by this slice; the parent can reconcile their criteria using this bounded evidence. Resolution of09 does not declare the entire setup effort qualified.

## Reopened default-concurrency diagnosis, 2026-10-26

### Inputs and scope

The preceding bounded closure is **superseded**, not extended to the newly failing matrix. Merged requested integration **`9f54bf4447622aabbd4c45565bfc3f10da6f7ad8`** into the same branch/worktree and claimed09 before work. Loaded TDD/ponytail; read ticket11's new execution evidence, `/tmp/setup11-final-matrix.log`, `-resources.jsonl`, and `-result.json`. Verified archived log SHA-256 **`74a753c9e73d098347cf68d8842822b70b8bb3579aeb7258563e3c7b71170c78`**. Exclusive Rust slot; parent source review read-only. No sibling/source instrumentation outside the stated root paths, provider/profile/trust, spec/map/07/11 or assertion/bounds changes.

Root Rust/config/test trees at matrix base `12c7fda8e97f3ec0e2c8e4000e193122e692b296` and current integration had **no diff** before observations. Lock SHA-256 remains `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; same clean sibling revisions as above. Toolchain/profiles unchanged. The observed-source patch is saved as `/tmp/setup09-default-observation.patch`, SHA-256 **`01c4a68053e7293053b9b934c7db972939e9227b773580f519529421d8c17b3d`**. Final instrumentation cleanup only borrows the owner label instead of allocating it and bypasses the poll observer entirely when the flag is unset; it is not a behavior fix or another default-concurrency execution.

### Source and archived-resource inspection before launch

- Archived resources measured memory pressure/free space/RSS only; **no archived CPU, run-queue, disk-I/O or CPU/I/O PSI samples exist**. Runtime archived minimum MemAvailable was about4.66GiB and peak sampled group RSS about0.95GiB; memory guard absence does not diagnose these failures.
- Failing Storage requests go through `storage-server/src/idp_client.rs`: service-token acquisition/cache, then authenticated IdP introspection; runtime policy checks can involve a second service client. Its internal HTTP bound is10s while the existing external verification client has5s. This is source ordering, not permission to change either bound.
- `idp-service/src/util.rs` invokes synchronous Argon2 inside async service/repository paths; default secret hash parameters remain47MiB/3 iterations/4 lanes. No crypto settings changed. Async repository/service code also calls the embedded native database synchronously; a long handler poll is not automatically all password hashing.
- Independent IdP ready-state validation checks current signer/login/user/relationship metadata and private reference evidence. However, a host with `view != ready` returns its ordinary `ready:false` projection **without invoking that callback**. This distinction matters in the actual diagnostic below.
- Create/Join/operation/remote isolated child fixtures use four Tokio worker threads; permission child uses two. The full libtest suite also has many concurrently running native fixtures. No duplicate fixture work was deleted: the two Create/Join variants qualify distinct retained/reset refusal behavior, and their common setup prefix is not proof of obsolete coverage.
- The on-upgrade history path and embedded query execution remain distinct possible owners of elapsed time. No unmeasured attribution to SDK, signer, native-lock, DB contention, thread explosion, stdout capture or the environment was made before execution.

### Observations and one diagnostic invocation

Added opt-in, categorical observations controlled by the **existing** `OF_SETUP_TIMEOUT_TIMING` flag:

- `crates/api/src/independent_create.rs`: owner request entry/exit for readiness/normal/token/introspection/databases, and synchronous `Future::poll` wall time >=50ms at the actual serving seam. No raw URL, ID, header, token, payload or password is logged.
- `crates/idp-service/src/util.rs`: std-only password hash/verify begin/end wall times and thread/process IDs. No secrets, hash parameters or verifiers are emitted; no_std code path remains unchanged.
- `crates/idp-server/src/independent_create.rs`: categorical subphases of current IdP ready-state validation.
- Existing test timing logger/readiness helper: PID/Unix-ms correlation for categorical output. No request construction, retry, assertion or deadline changed.

Built exactly the failing suite's **all-features** root target, jobs=1:

```sh
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --all-features --test owner_boundaries --no-run --message-format=json
```

Initial compile attempt exposed a diagnostic placement error (the status route's existing utoipa attribute had attached to the new private struct); corrected before execution. Build stage2 stopped at its predeclared550s ceiling, retaining artifacts. Stage3 passed in327.679s. No RAM/PSI stop. Resolved executable from successful Cargo JSON, verified its50-test listing: `owner_boundaries-fbdb13f2c672f60d`, SHA-256 **`1686f000aa1a3937318f34cd27e37bdcf2d7ab9dbc0f65a8b05bd2ce1d67044c`**.

**One diagnostic run only**: whole owner executable, no filters, no `--test-threads`, no `RUST_TEST_THREADS`/Tokio-worker override, **no `--nocapture`**. Environment adds `OF_SETUP_TIMEOUT_TIMING=1`; same stack67108864 and jobs1. This preserves default concurrency and libtest capture, but is explicitly an **observed diagnostic**, not uninstrumented qualification.

Actual result **exit101**, **36 passed / 8 failed / 6 ignored**, **271.219s** supervised. No retry-to-green or second default workload invocation.

- Both Create parents failed the same invalid-bearer Storage send, now line782 only because the observation added lines.
- Operation failed the same authenticated Storage send, now create.rs453.
- Both Join parents failed bounded IdP readiness, now create.rs260.
- Remote failed bounded **Storage** readiness this time, not the archived IdP readiness phase.
- Permission parent additionally failed an ordinary IdP applications policy-denial request send at permissions.rs310.
- The eighth failure is a **separate prerequisite miss**: `native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes` reports missing built `idp-client/esm/index.js` in **this** worktree. Ticket11 had prepared its different worktree. This omission means the run cannot be presented as equivalent prerequisite-complete matrix qualification. It is recorded rather than fixed/skipped/retried; no SDK/sibling/generated-source changes made. The five matching ordinary failures and other observed request/readiness failures still occurred in the actual full default-concurrency suite.

### What the captured failing paths establish

**Storage request chain, not just a passing subset:** intact correlated records for one Create process show Storage handler entry at wall_ms1793042017649; two IdP token handler entries at1793042017651 and1793042017656. Token calls completed after1546ms and3411ms; synchronous token polls included1546ms and1465ms. Storage handler ended at1793042022650, elapsed5000ms, with the client's actual invalid-bearer send timeout. The other Create process has the same pattern: first token1510ms, overlapping token3381ms, Storage5001ms. These are real service spans; an absent token or database-lock error is not inferred.

For the operation failure, Storage entry1793042034641; token entries1793042034643/1793042034645; token completions1677ms/**4220ms**. One synchronous token poll took1624ms. The Storage handler's span ended at1793042039642 (**5001ms**) with its authenticated send timeout. The spans narrow work to the dependent IdP/policy chain, but do not fully decompose every token query/JWT/policy/response-body stage. They **do not** demonstrate that offloading Argon2 alone would fix the5s request.

There were79 intact password end records: maximum hash **445ms**, verify **409ms**. Concurrent stderr formatting interleaved some records; only complete records are used for these maxima, not reconstructed/mangled fields. Actual synchronous IdP ready-state polls reached **3404ms**, and token polls around1.5–2s. These identify substantial synchronous work on Tokio workers beyond the measured password span, **not a deterministic causal fix**. A further root repository/query/signing breakdown is needed before deciding which operation to move/rewrite.

**Join readiness is not demonstrated ongoing ready-state DB-check contention:** the two failed Join processes repeatedly received HTTP200 JSON `ready:false`, including ~0–4ms responses late in the unchanged60s budget. There are **zero entered `idp_readiness` callback spans** for them. Both reached approval-wait completion (~11.49s/11.18s); Device public import completed4311/4691ms, but IdP's `public-import` completion log never appeared. Source narrows the blocked interval to `idp-server/src/independent_join_worker.rs:189–214`: history challenge/request, authenticated history synchronization/import, then signer readback. This does not yet distinguish which nested await/operation stalled. A readiness timeout must not be relabeled a slow readiness SQL checker when that checker never ran.

Remote's changed Storage failure and the additional ordinary permission timeout remain distinct pending diagnosis. No missing-signer/native-lock error was the reproduced failure. Shutdown endpoint warnings following parent panics are consequences, not identified causes.

### CPU, disk, scheduler and capture measurements

`/tmp/setup09-default-monitor.py` extended the existing owned-PGID monitor with approximately one-second samples: global CPU counters, CPU/I/O/memory PSI, diskstats, load, per-process CPU/I/O/output FD targets, and per-thread schedstat/comm/wchan. No perf/ptrace/syscall tracing was used (`perf_event_paranoid=4`); no payload observations. Sampling/extra logging perturb execution and are not silently equated to ordinary execution.

- Maximum observed owned concurrency: **10 processes / 86 threads**. This demonstrates concurrent runtimes, not an unbounded per-process default-runtime explosion.
- Sampled host CPU busy was about97–99.9% through representative41–121s samples. Owned group CPU consumption over measured20–40/60–80/90–110s windows was about**9.95/14.03/12.39 core-equivalents**, using verified CLK_TCK100 on the16-logical-CPU host. Maximum CPU PSI some avg10 **14.58**. This is actual CPU pressure, not proof that every failure is solely runner capacity.
- For the operation's failing request window, nearest samples span4.480s. Four Tokio workers consumed about1.891/1.109/0.473/0.159s CPU and accumulated0.418/0.272/0.082/0.067s runnable wait respectively. Run-queue wait accounts for some time but **does not account for the whole failed request**. Snapshot alignment is approximate; the first Create process exited before an end sample, so no invented complete per-thread delta is reported for it.
- Max sampled I/O PSI some avg10 **2.87**; representative iowait around0.1–1.8%. Wait-channel samples included18 `folio_wait_bit_common`,13 `submit_bio_wait`,12 `jbd2_log_wait_commit`; most samples were futex/event polling/running. This is insufficient to prove or exclude DB disk cost or a particular lock wait.
- **Zero sampled pipe-write wait channels**. Existing stderr records from separate processes nevertheless interleaved, and stdout/stderr sampling cannot exclude between-sample capture stalls. No blanket stdout-capture cause or exoneration.
- Runtime min MemAvailable **4,568,912KiB**, max sampled owned RSS **915,256KiB**, kernel max individual child RSS **670,064KiB**, max memory PSI some/full avg10 **0.14/0.12**. No resource guard fired. Full swap did not veto launch.
- Successful build stage sampled group RSS peak **4,794,072KiB**, kernel max child **4,669,760KiB**, min available **4,017,428KiB**, max memory PSI **9.84/8.96**; no sustained threshold fired. Only the earlier compilation stage's550s ceiling fired. Owned PGIDs were verified absent afterward; no unrelated kill/cache deletion/profile change.

Local artifacts: `/tmp/setup09-qual-default-run1.log` SHA-256 **`315268be3af2e0826ba673995e754d7df3b8268d00cac769b986a947bdb7fec3`**; `-samples.jsonl` (6,737,737bytes) SHA-256 **`e515bd2212510776282a97da3dcf2922009fa5761deafb4fa3fcf4d65cde528a`**; `-run1.json` full argv/exit/resources; `/tmp/setup09-default-artifact.json` Cargo artifact; observation patch hash above. Build/validation receipts use `/tmp/setup09-qual-default-{build1,build2,build3,clippy,clippy-final,api-tests,service-check}.{log,json}`. All individual tool calls <600000ms. Live telemetry includes CPU/I/O/scheduler data absent from ticket11's archived memory-only samples.

### Validation and remaining causal gate

- Full all-features existing default-concurrency suite: **failed as above**, not qualification and not retried.
- `cargo test ... -p api --lib -- --test-threads=1`: **42 passed / 2 child-only ignored**, parents exercised isolated children,8.62s test/42.057s supervised. This validates changed instrumentation/lifecycle behavior, **not** default-workload reliability.
- `cargo clippy ... -p api -p idp-service -p idp-server --lib --no-deps -- -D warnings`: passed; final-source repeat3.975s. Scoped libraries, not whole-workspace/test Clippy.
- `cargo check ... -p idp-service --no-default-features`: passed71.671s; retained unrelated unused-import warnings in oauth2 authorization/scope modules. Confirms std-gated observations do not break the no_std build; no dependency/features added.
- Formatting and diff checks passed. No production behavioral fix or new deterministic causal regression is claimed. TDD gate remains: establish the actual root mechanism with a focused deterministic regression **before** changing implementation. Instrumentation is not a fix.

**Ticket09 stays open.** Actual next tasks, not another passing subset: (1) decompose the measured long synchronous IdP token polls into repository/native query versus signing/policy work, using root-owned seams first; sibling instrumentation requires explicit ownership/authorization; (2) capture the nested history request/synchronization/readback stage in the failed Join import interval, not more ready-state checker timing; (3) retain remote Storage and ordinary permission failures separately until their actual dependent stage is identified; (4) prepare/verify this worktree's SDK prerequisites before any further same-suite qualification. A future reproduction is a separately declared diagnostic, not an automatic retry. No available evidence justifies offloading a guessed operation, changing password cost, reducing concurrency, extending deadlines, caching current readiness, skipping a scenario or deleting shared fixture setup.

## Nested history preparation and measured RAM stop, 2026-10-26

### Ownership and prerequisites actually completed

Merged clean integration **`aec614b32b7288bd714d8b7b33d72c4fdf3f5bde`** into the same worktree/branch, claimed09 and loaded TDD/ponytail. This slice owns main09 evidence/ticket and relevant root API/IdP/Device history/crypto; the separate Storage agent owns **only** `crates/storage-server` and its separate denial evidence. No overlapping Storage edits, sibling instrumentation, generic task framework, new dependency, profile, deadline, retry, trust or identity changes.

Before any proposed full-suite diagnostic, executed the existing prerequisites **in this worktree**, using the existing cached pnpm store rather than assuming ticket11's other worktree artifacts apply:

```sh
pnpm install --frozen-lockfile --ignore-scripts --offline && pnpm sdk:build && node --test scripts/generated-package-exports.test.mjs
```

**Passed**,3.896s supervised.465 packages reused, zero downloads, all four owner-client ESM builds passed, built exports **5/5 passed** (including Storage generated export). No root node_modules replacement. Own worktree dependencies and completed SDK outputs retained. The same eight pass-generated tracked map/bin/symlink paths were saved as `/tmp/setup09-history-prereq.patch` plus an exact path list, required to match that saved entire binary diff before restoring only those paths to HEAD. No unknown changes restored. Restoring those generated tracked paths does not delete the built ESM exports or caches.

### Nested observations now implemented

Plain categorical observations, no task-management abstraction:

- `api/src/independent_history.rs`: challenge HTTP request start/completion, typed timeout/connect/HTTP-status failure, then callback-channel receive start/completion. Does not print source URLs, approval bodies, nonces or capabilities.
- `idp-server/src/independent_join_worker.rs`: challenge, import, imported signer readback and retained signer readback separately, with result ErrorKind and elapsed time. Existing cancellation/error propagation remains unchanged.
- `device-service/src/join_history.rs`: source callback timing; client WebSocket connect; transport send/receive; source/recipient synchronization and each current authorization recheck. Native `SyncError` is exhaustively classified into fixed categories (pre_apply/engine/transport/staging/configuration/protocol/limits/version/unexpected_message/remote_abort), without raw error strings that could disclose history values.
- Source WebSocket upgrade no longer **silently discards** synchronization failure: native category and source-sync ErrorKind are observable via warnings. It still returns/cleans up exactly as before; this is error observability, not a claimed protocol fix.
- `idp-service/src/oauth2/service.rs`: client-credentials lookup, secret verification, active-root lookup, principal lookup and issuance phase markers, to distinguish the previously measured1.5–2s poll from password verification alone. Std-only optional output, no crypto settings changed.
- Existing timing logger admits only the added API/history categorical targets. Flag-enabled logger records are formatted into one buffer and written through stderr's locked writer, avoiding the previous multi-piece formatting for **these** records. No packet/payload/header tracing or raw URI/identity output.

The exact phase instrumentation is ready to distinguish the previously narrowed `independent_join_worker.rs184–214` interval. **No newly captured failing nested operation/error is claimed**, because the prerequisite-complete test binary did not finish building below the measured safety guard. Likewise, no demonstrated crypto responsiveness defect or deterministic behavioral regression/fix exists from this slice. Do not infer a fix from these observations being implemented.

### Actual builds, safe stop and validation

Proposed cell remains one full existing `owner_boundaries` suite, root all-features, default libtest concurrency/capture, timing flag enabled, no filters/retries/deadline changes. Build command actually attempted at jobs1, normal profiles/shared target:

```sh
cargo test --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --all-features --test owner_boundaries --no-run --message-format=json
```

| Attempt                       | Actual outcome                                                   | Seconds | Min MemAvailable KiB | Peak sampled owned RSS KiB | Max memory PSI avg10 some/full |
| ----------------------------- | ---------------------------------------------------------------- | ------: | -------------------: | -------------------------: | ------------------------------ |
| `history-build1`, PGID3360563 | Guarded stage-ceiling stop; completed library artifacts retained | 551.064 |            3,626,796 |                  1,829,704 | 0.38/0.38                      |
| `history-build2`, PGID3364587 | **RAM-floor stop**, not compile/test error or swap veto          |  99.808 |        **2,086,616** |                  1,878,588 | 0.69/0.69                      |
| `history-clippy`, PGID3365939 | **Passed**, strict changed-library Clippy                        |  12.353 |            3,180,980 |                    688,652 | 0.18/0                         |

The RAM stop is below the declared2GiB floor (**2,097,152KiB**). It terminated/reaped only the owned PGID; no unrelated kills. After the compiler exited, available RAM remained about3.4–3.5GiB. Process snapshots showed newly present unrelated browser processes; they were neither killed nor declared the cause of any ordinary request failure. Full swap alone did **not** prevent the initial guarded launches. Relentlessly relaunching the same memory-heavy compile under the lower baseline would not be productive causal diagnosis.

Strict library validation actually passed:

```sh
cargo clippy --locked --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p api -p idp-service -p idp-server -p device-service --lib --no-deps -- -D warnings
```

This compiles/checks the new observations in all four relevant libraries; it is **not** a completed root test build or runtime regression. `cargo fmt --all -- --check` and `git diff --check` passed. No full-suite diagnostic, reduced-concurrency substitute, behavioral regression test, coverage, matrix or CRAP ran. No current no_std build result is added for this slice's std-gated token markers.

Receipts `/tmp/setup09-qual-history-{prereq,build1,build2,clippy}.{json,log}` and `-samples.jsonl` retain commands, exits, one-second memory/RSS/PSI observations and hashes. Important SHA-256:

- Prerequisite log: `b60713dd18cacbc9152928fc5aa8d2de8726bf251f4868c9582e3a390faf685c`.
- RAM-stopped build log: `5a2063be757ccfc97ad80d9fd1f523233476beb96bacf7bdaf9daa65a667ac96`.
- Strict library Clippy log: `c09f9697dc32b7b31f03ab79a0e74cc083b50ffedcfd489648224692d70cef8a`.
- Root Rust/test diff before documentation: `417579786f28872a7a339fba8764c7bcfffe00a96f7a9cdb71eb3a98c47d00b7`.
- Cargo.lock unchanged: `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`.

### Handoff and actual remaining gate

**Ticket09 open/released. Rust slot explicitly released to the Storage agent after the passed low-memory checks; no further builds/tests in this slice.** Owned compilation groups were verified absent. Main09 and source-synchronization observations do not modify the Storage agent's owned files/evidence. Parent coordinates broader11; no07/spec/map/11 edits.

The blocker is concrete: finish the same root all-features test build with adequate measured headroom, then execute the **still-unrun, one prerequisite-complete default-concurrency diagnostic**. SDK preparation is now done. Use the nested operation/error records, not another undifferentiated readiness duration, to locate the Join stall; separately use client-credentials phase evidence before deciding on any bounded crypto execution change. Only an actual causal mechanism plus deterministic public-seam failing regression authorizes a behavioral fix. The existing ordinary failures remain unresolved; no new cause or passing qualification is invented to compensate for the resource stop.
