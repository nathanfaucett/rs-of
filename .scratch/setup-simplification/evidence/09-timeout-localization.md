# Ticket 09: timeout localization — observation-only handoff

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
