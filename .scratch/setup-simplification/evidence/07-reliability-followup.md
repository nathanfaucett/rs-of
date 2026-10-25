# Ticket 07: HTTPS reliability follow-up

## Disposition

**Blocked qualification; ticket07 remains open, both integrated durability boxes unchecked.** No backend patch is justified by the evidence collected here. Seven serial three-scenario suites produced **20 passes / 1 failure**; a separate syscall-traced interruption pair produced **1 pass / 1 failure**. The untraced failure is admission-lock contention immediately after successful coordinated shutdown. The traced failure reproduces the ordinary authorized source-approval request timeout. Neither a common root cause nor a reliable baseline has been established. Do not accept later green repetitions as erasing either failure.

## Ownership and provenance

- Worktree `/home/nathan/code/rust/aicacia/of-setup-qualification`, branch `setup-simplification-qualification`; initial HEAD and integration `54816fa00764ff81ccc0811f560b77103a4a4253`. `git merge-base --is-ancestor 54816fa00 HEAD` succeeded. Worktree was clean before claiming **Status: claimed** in ticket07, before build/test work.
- Read AGENTS, tracker/triage/domain pointers, glossary map/shared glossary, setup-worker/security contracts, spec/map/ticket07 and both existing qualification receipts. Invoked TDD and ponytail. Existing user-authorized public `production_api_join_` HTTPS parents and their isolated child are the seam. No Svelte files were relevant or touched.
- Priority high; lifecycle/security risk high; investigation complexity medium. Prefer existing failing scenarios over another fixture. No deadline, security assertion, authentication, readiness or identity retention changed. No new dependency or speculative backend fix. This is a failed qualification/red result, not a red-to-green fix.
- Initial sibling revisions: `ofdb` **90a47d78fffddd6602ee38fea601658916039cd4**, `ofnet` **dbea925a0a3f99053fffc02618baccc2a1cbea9f**, `offs` **df4c696a910296635473ec296d9235acbaa60d9b**. Final observed `ofdb` **e3242170c94d5fc4bcc121db812310c8106c7ab6**; other two unchanged. All three reported clean at the later snapshot; none was edited, reset, stashed or committed by this agent.
- `ofdb` advanced independently during this investigation: `17178f4a5a4e2a475c0f50fb4aae6d344ea73d44` (14:31:02 -0400, native cold SQL changes), `7f19b7bf9ed0e4ac953bdf0d0e117b9d1800c3f1` (14:46:42), `e3242170c94d5fc4bcc121db812310c8106c7ab6` (15:01:13). These are **not pinned transitive build inputs**. Do not attribute a failure to them or claim these runs certify the latest sibling HEAD. The repeated executable bytes were stable.
- Main source/lock untouched. Main `Cargo.lock` SHA-256 at entry and after testing: **bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f**. Cargo refreshed the qualification-worktree lock only; that generated refresh is excluded from delivery.
- No real profiles, secure providers or OS trust changed; existing parents use private temporary native-root bundles and injected trust commands. No unrelated user process was stopped. No matrix, coverage, GUI, WAN or platform certification is attempted here.

## Executable verification and commands

The existing `unified_server-f38d95073d23ab7e` had a 12:28:18 build time, matching test parents and a Cargo fingerprint, but that alone did not verify all final transitive sources. Rather than assume freshness, ran Cargo against this worktree, sharing only the target cache:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 cargo test --manifest-path /home/nathan/code/rust/aicacia/of-setup-qualification/Cargo.toml -p unified-server --lib production_api_join_ -- --nocapture --test-threads=1
```

Cargo rebuilt, finished the optimized/debuginfo test profile in **8m15s**, and passed the three scenarios in **266.01s**. Three unused support-function warnings (`seed_permissions`, `infrastructure_token`, `run_lifecycle_http_checks`) were emitted; no compilation error. Overall tool bound was **1200000 ms**, not exhausted.

Then ran six sequential executions, in two bounded batches, with no concurrent qualification compilation/test launched by this agent:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-f38d95073d23ab7e production_api_join_ --nocapture --test-threads=1
```

Working directory was the qualification worktree. Python subprocess wrappers captured combined output, took `ps -eo pid,pcpu,etime,args --sort=-pcpu` snapshots before each suite, and bounded each suite at **1000s**, each three-suite tool invocation at **1200000 ms**. No wrapper/tool timeout fired. Binary SHA-256 before/after repeats: **ad9d22e193d31fb660d20cce06b5097e74883450cf89af0d52e9e5647d400220** (mtime 14:34:17 -0400).

These were idle **with respect to this qualification workload**, not a globally idle-machine certification. Snapshots contain editor, browser, background processes and the independent browser/tooling activity. The unrelated user wasm activity observed at entry was left alone. Do not infer that CPU contention caused any failure, or claim global host idleness from process-lifetime CPU percentages.

## Untraced serial results

Times below are actual libtest child times; suite total includes subprocess overhead. Date: 2026-10-25, local zone -0400. Suite filters select exactly three parents; the isolated child is deliberately parent-launched, not skipped.

| Run | Finalization | Offline reopen | Interruption/exact retry | Suite result / time |
| --- | --- | --- | --- | --- |
| Cargo source verification | pass 102.09s | pass 79.08s | pass 84.03s | 3 passed, 266.01s |
| Repeat 1 | pass 85.58s | pass 80.93s | pass 76.29s | 3 passed, 243.07s |
| Repeat 2 (14:43:02) | pass 81.03s | pass 82.87s | pass 82.65s | 3 passed, 246.80s |
| Repeat 3 (14:47:08) | pass 89.27s | pass 83.82s | **fail 25.77s** | **2 passed / 1 failed, 199.21s; exit 101** |
| Repeat 4 | pass 86.48s | pass 78.31s | pass 79.30s | 3 passed, 244.64s |
| Repeat 5 (14:55:52) | pass 89.69s | pass 92.53s | pass 86.82s | 3 passed, 269.34s |
| Repeat 6 (15:00:21) | pass 91.26s | pass 79.48s | pass 73.80s | 3 passed, 244.77s |

Repeat 3 actual failure:

```text
thread ... production_https_bootstrap_child ... panicked at crates/unified-server/src/api_join_tests.rs:378:43:
incomplete Join API reopen: Custom { kind: WouldBlock, error: "another setup operation owns admission" }
...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 60 filtered out; finished in 25.77s
...
production HTTPS create/join/reopen child must succeed
...
test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 58 filtered out; finished in 199.21s
```

That line is `SetupHost::open(host)` after authorized cancellation, unfinished status, source-offer release, cancellation of the server, successful `server.await` cleanup, and successful `BootstrapConfig::open_explicit` acquisition of the retained host operation guard. It is before resubmission/second offer. Thus the retained host operation guard is available while the distinct admission guard is not. There is no delayed-reopen retry or arbitrary sleep masking this failure.

## Traced diagnosis: request timeout reproduced, not an idle timing qualification

To investigate admission descriptor lifetime without logging HTTP bodies/passwords/codes, ran a separate bounded interruption-only loop under:

```sh
strace -f -ttt -yy -e trace=flock,close,openat -o /tmp/07-reliability-lock-trace-N.log /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-f38d95073d23ab7e production_api_join_interruption_preserves_private_host_intent --nocapture --test-threads=1
```

Same environment/cwd, **400s per run**, **1200000 ms** outer bound; planned maximum six, stopped on first failure after two. Tracing changes scheduling/cost, so do not pool these with the untraced acceptance runs.

- Traced 1: **pass**, child **104.45s**, parent **104.80s**.
- Traced 2: **fail**, child **41.09s**, parent **41.47s**, exit **101**; no command timeout. Actual output:

```text
Node Join reopen stage=intent
Node Join reopen stage=history
Node Join reopen stage=credentials
Node Join reopen stage=owner

thread ... production_https_bootstrap_child ... panicked at crates/unified-server/src/api_join_tests.rs:425:14:
ordinary authorized source approval: reqwest::Error { kind: Request, url: "https://127.0.0.1:33953/device/node-joins/01a17476-2e92-763e-8a1e-fb556983d751/approve", source: TimedOut }
...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 60 filtered out; finished in 41.09s
...
production HTTPS create/join/reopen child must succeed
...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 60 filtered out; finished in 41.47s
```

This is the existing ordinary authorized approval, not a forged-token negative, compiler timeout, or parent 300-second watchdog. The production verification client still uses verified isolated native roots, no redirects/proxy, **1s connect / 5s whole-request timeout**. Existing pending-offer **15s**, nonready **30s**, ready **120s**, and parent **300s** bounds were untouched. The source offer was observed and approved-route processing reached; no claim is made about which internal approval stage stalled.

The traced failure did **not** reproduce the admission lock collision: its target admission lock was closed on thread 2684033 at epoch **1792955262.749378**, reopened and successfully `flock(LOCK_EX|LOCK_NB)` acquired on thread 2684031 at **1792955262.769661**. The one traced `EAGAIN` was the fixture intentional live-host `operation.lock` contention check, not this admission failure. A lock-lifetime cause cannot be inferred from this different failure.

## Root-cause investigation and remaining work

- **Pending offer:** no 15-second pending-offer failure in this follow-up. All reached pending offers used real Iroh with the harness existing shared in-memory address lookup/minimal preset and relays disabled. This is same-host authenticated transport, not production discovery/WAN qualification. Prior pending-offer timeout remains historical unresolved evidence, not disproved by these passes.
- **Shutdown/admission:** `SetupHost::serve_verified` awaits the API server, cancels, then awaits the owner. `SetupAdmission` holds its file lock until final Arc drop; hosted routers keep `Arc<Shared>` containing it. The installed Axum **0.8.9** connection task drops its graceful-shutdown `close_rx` notification before its pinned connection/service locals leave scope (`axum/src/serve/mod.rs`, `handle_connection`, around 396–415); the server awaits notification closure, not the connection task JoinHandle. This gives a concrete **candidate** ordering window in router-state destruction. It is not yet a demonstrated causal trace of repeat 3; do not call it proven or patch around it by weakening mutual exclusion. The next focused experiment should capture which admission Arc/descriptor survives the awaited host shutdown and prove destruction ordering at that seam.
- **Approval/request timeout:** traced replay reproduces the earlier symptom. Ordinary approval in `device-server/src/node_join_source.rs` serializes an exact offer, enrolls, verifies, reads/writes owner policy, refreshes approved endpoints, rechecks current offer and publishes the signed reply. Existing internal diagnostic prints are `cfg(test)` and not enabled when device-server is a dependency of unified-server tests; the combined output therefore does not locate a stalled stage. Syscall tracing here intentionally omitted network payloads and does not distinguish TLS/client scheduling, owner work or reply progress. Next experiment needs categorical phase/task timing through this public request without secrets, then a focused failing regression before a root-cause patch.
- No baseline comparison was performed and there is **no environment-only, integration-regression, sibling-change or Iroh-specific root-cause attribution**. The distinct lock and request failures must not be conflated.
- Both open ticket boxes require repeatable integrated Join/restart evidence. Finalization and offline reopen each passed seven untraced times; interruption failed one of seven, and authorized approval still timed out in the diagnostic replay. Accordingly **neither box is checked**, ticket is open, and no spec/map/historical receipt was edited. Full matrix/CRAP/tooling diagnosis remains the other agent responsibility; not rerun here. No frontend/native/UI/platform acceptance inferred.

## Reproducible raw-output locations

These local disposable logs are not delivery files. Relevant actual failures and every run summary are preserved above; the log hashes distinguish the original capture from later edits:

| Local file | SHA-256 |
| --- | --- |
| `/tmp/07-reliability-repeats.log` (repeats 1–3, process snapshots and combined output) | `4d58dfb0995ea8cda1497dd755a96c34bb0fd5f0832f9075e61ffe45b57eb0e6` |
| `/tmp/07-reliability-repeats-followup.log` (4–6) | `9da4c69cb1c766c2f22d947f1922c39fa36080b03d1a9d79a83511a7538e974f` |
| `/tmp/07-reliability-traced-interruption.log` | `00c13ee448cde11e711cff4aff8ebe9d11c0bc151b9cc0c36a250cf9a6d78a7e` |
| `/tmp/07-reliability-lock-trace-1.log` | `88bdba251757a393f6ba027403c497698673227d12782e7c5702657518cf227c` |
| `/tmp/07-reliability-lock-trace-2.log` | `43bba39cbbecc01aa618dcde4f0d0b14d72a950c4bda58f180862dfd13b4d427` |

Only ticket07 and this new evidence file are delivery changes. Documentation validation: `git diff --check`; no new implementation to lint or feature-matrix qualify. Integration tip must be merged on this branch before handoff; no merge back to integration is authorized.
