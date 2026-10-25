# Ticket 07: HTTPS reliability follow-up

Latest result: the [P1 queued-reply follow-up](#p1-review-queued-replies-must-be-rejected-before-the-teardown-fence) fixes a circular wait introduced by the original teardown fence. Deterministic red/green, 100 lifecycle executions and three final-code serial HTTPS suites (9/9 scenarios) pass. The original lifecycle and qualification runs below remain historical; ticket07 remains open conservatively, with no claim that earlier offer/request timeouts or broader validation failures are eliminated.

## Initial investigation disposition

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


## Focused lifecycle follow-up: deterministic red → green

### Scope and source investigation

User requested continued diagnosis of the admission `WouldBlock` **after awaited shutdown**, a deterministic regression and a minimal fix only if proven. Reclaimed ticket07 before work on the same branch, starting at `2c0ed384ff27f827a323768dfb478a61b9e16be6`. Invoked TDD and ponytail again. Read source first while the independent validation build ran; did not launch a concurrent full HTTPS repeat suite. The first focused Cargo command waited on the shared target lock before compiling. All Cargo commands retained the main target cache, `CARGO_BUILD_JOBS=2` and `RUST_MIN_STACK=67108864`.

Proven ownership path:

1. `SetupHost::into_worker` transfers the exclusive `BootstrapConfig` host guard to `RuntimeSetupOwners`, and admission to the shared hosted worker.
2. `HostedSetup::router` / `build_setup_router` / the normal fallback retain `Arc<Shared>`. `Shared.admission` retains `Arc<SetupAdmission>`; its `LocalState.lock: File` holds the kernel lock until final destruction. No explicit unlock or durable admission retirement is appropriate during interrupted shutdown.
3. `SetupHost::serve` / `serve_verified` and native `NativeHost` await their listener and then the worker. The old `HostedSetup::run` only withdrew published owners and awaited owner shutdown; it could return even when router/control references still held admission. Its concrete owners/host guard were then destroyed, making the host operation lock available before the admission lock.
4. Axum 0.8.9 `handle_connection` drops its `close_rx` completion notification before the pinned connection/service locals leave scope. The listener can therefore finish waiting for that notification while connection-owned router destruction is still outstanding. Awaiting the worker did not previously close that ownership gap.

This source ordering and the deterministic regression prove a **real lifecycle defect** consistent with the recorded failure. They do not retroactively provide a syscall trace of the earlier failing process, and do not establish a shared cause for pending-offer or approval-request timeouts.

### Regression and minimal fix

Authorized public lifecycle seam: `SetupHost` → `into_worker` → `router` → `run` cancellation → existing-only reopen. New `setup_host_tests::shutdown_joins_router_teardown_before_releasing_host_and_admission` uses a neutral HTTP host, no domain provisioning, provider, OS trust or Iroh.

The test deliberately retains one connection-equivalent router clone during teardown. It cancels the worker and polls its shutdown future once; no scheduler race, delay or retry is involved. Shutdown must stay pending with **both** the host guard and admission exclusion intact. Dropping the last router must wake shutdown; after awaiting it, a single immediate `BootstrapConfig::open` plus `SetupHost::open_http` must succeed with byte-identical retained admission. The five-second test bound detects a lost wakeup; it does not delay or retry reopen.

**Red:** added only this test, ran:

```sh
cargo test --manifest-path /home/nathan/code/rust/aicacia/of-setup-qualification/Cargo.toml -p unified-server --lib shutdown_joins_router_teardown_before_releasing_host_and_admission -- --nocapture --test-threads=1
```

With the documented shared-target environment, Cargo waited on the validation build lock, then finished in **16m06s**. The test failed in **0.05s**, exit **101**:

```text
panicked at crates/unified-server/src/setup_host_tests.rs:36:5:
shutdown must join outstanding router teardown
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 61 filtered out; finished in 0.05s
```

**Fix:** add a private watch-channel teardown fence to `Shared`, declared **last** so closure follows destruction of admission and all preceding shared fields. After unpublishing and owner shutdown, `HostedSetup::run` subscribes, drops its own shared reference and awaits channel closure. Concrete owners and their host guard remain owned by the shutdown future until the teardown join completes. It awaits destruction rather than changing admission, unlocking a live handle, testing reference counts in a loop, or sleeping. The channel is never sent on and no sender is exposed/cloned outside `Shared`.

The worker API now documents that callers drop issued routers/controls before joining shutdown. Audited all actual `run` callers: shared HTTPS/HTTP and native hosts already end listener/router serving first. One direct-router integration fixture held its router while awaiting the owner; moved that fixture drop **before** the join, without changing any of its retained-state/security assertions. No native implementation rewrite was needed. Existing Shared unit-test fixtures initialize the private notification field.

**Green:** reran the exact regression command; **1 passed**, **1.55s** (3m28s rebuild). No dependency or admission API change. Production implementation is confined to `crates/idp-server/src/hosted_setup.rs`; regression/caller fixture changes are in unified-server tests.

### Focused validation and actual lock destruction

| Command / scenario | Actual result |
| --- | --- |
| `cargo test -p unified-server --lib setup_host_tests -- --nocapture --test-threads=1` (worktree manifest/shared target) | **4 passed / 1 deliberately parent-launched child ignore**, 0.39s; parent actually ran both isolated TLS child modes, both passed. Final-source rerun: **4 passed / 1 child ignore**, 0.38s, rebuild 16.86s. |
| `cargo test -p idp-server --no-default-features --lib hosted_setup::tests -- --nocapture --test-threads=1` | **2 passed**, 2.59s (5m22s feature-graph build). Existing cancellation/retirement exclusion and request-disconnect versus explicit cancellation remain valid. |
| Execution-only `retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation` | **1 passed**, 0.29s, including all five retained-fault variants. |
| Execution-only `partial_credentials_reopen_preserves_recovery_for_create_and_join` | **1 passed**, 0.52s, existing exact-recovery/refusal variants. |
| Exact new lifecycle test, 50 serial execution-only repetitions | **50 passed**, 2.09s total. Each subprocess bounded at 30s; no full HTTPS repetition or compilation in that loop. |
| One execution-only `production_api_join_interruption_preserves_private_host_intent --nocapture --test-threads=1` | **1 parent/child passed**, child 79.33s / parent 79.39s. `BUILD_SNAPSHOT []` immediately before these focused executions; no observed Cargo/rustc at start. Not a globally idle-host certification or repeat reliability result. |
| `cargo clippy -p idp-server -p unified-server --no-default-features --lib --no-deps -- -D warnings` | **Passed**, 23.46s, scoped production libraries only. |
| `cargo fmt --all -- --check`; `git diff --check` | **Passed** on final source. An initial formatting check found only the new test layout; applied rustfmt before final validation. |

Commands used explicit worktree manifest or the freshly built main-cache `unified_server-f38d95073d23ab7e` executable. All were bounded (Cargo at 1200000ms, focused execution group at 600000ms, HTTPS child command at 400s); none exhausted its bound. Cargo emitted the same three unused shared test-support function warnings. Initial compilation wait is contention evidence, not a runtime failure attribution. No expensive feature matrix/CRAP run was added alongside the independent validation effort; those results remain separate.

One final **0.08s** syscall-traced exact regression proves the intended lock ordering. Command used only `flock,close,openat` tracing (no request data), same final executable, 30000ms tool bound. Relevant actual trace events:

```text
2718283 1792957754.977230 flock(12<.../setup-admission/lock>, LOCK_EX|LOCK_NB) = 0
2718283 1792957755.014335 flock(14<.../setup-admission/lock>, LOCK_EX|LOCK_NB) = -1 EAGAIN (Resource temporarily unavailable)
2718283 1792957755.014391 close(14<.../setup-admission/lock>) = 0
2718283 1792957755.016300 close(12<.../setup-admission/lock>) = 0
2718283 1792957755.023867 flock(12<.../setup-admission/lock>, LOCK_EX|LOCK_NB) = 0
```

The deliberate competing open is excluded while the router lives, the original descriptor actually closes after router release, then the **single** reopen obtains the lock. No `LOCK_UN` shortcut was added. Local trace `/tmp/07-admission-lifecycle-fixed.trace`, SHA-256 **01f7896688349b3b04118fd6fbf52413ca3230f35ff6ef0def3cf8b4137893a1**. Final test executable SHA-256 **5c8af934b71b32937e2802badbecd522cfa561810eb0a705c2392a1843456459**. Earlier repeated-executable hash above intentionally describes pre-fix code.

### Delivery boundary

The demonstrated **router/admission shutdown ownership gap is fixed**, with deterministic failing-before/passing-after evidence. Host shutdown now includes shared router/control teardown; retaining a control/router deliberately postpones shutdown rather than allowing a second lifecycle owner to race it. There is no timeout inflation, reopen retry, forced unlock, replacement identity, or permission/readiness relaxation.

Ticket07 stays **open**, both integrated durability boxes unchecked. One interruption smoke pass is not the requested repeatable integrated finalization/interruption/offline qualification; source-offer and ordinary HTTPS request timeouts still have no established root cause. Historical failures above are retained and no baseline/environment-only attribution is made.

Sibling revisions throughout this focused follow-up were observed as `ofdb` **e3242170c94d5fc4bcc121db812310c8106c7ab6**, `ofnet` **dbea925a0a3f99053fffc02618baccc2a1cbea9f**, `offs` **df4c696a910296635473ec296d9235acbaa60d9b**. No sibling modifications. Main lock hash still **bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f**; worktree-only Cargo lock refresh is not delivery. No main source, existing other receipts, spec/map, real profiles or OS trust changes. Integration tip is to be merged before handoff, never merged back by this agent.


## P1 review: queued replies must be rejected before the teardown fence

### Base, claim and demonstrated cycle

Merged integration **4ae84f2fc4bbbaf007222859ee48b0e29b641ddf** first (fast-forward from `0739d5855`), including the separate validation receipt. Verified that integration commit is an ancestor of HEAD, then reclaimed ticket07 before implementation. Used TDD and ponytail. Source/read review covered `HostedSetup::run`, `submit`, active/queued commands, owner unpublish, and the router/admission teardown fence. No spec/map, validation receipt, main source/lock or unrelated scratch modifications.

The reviewer identified a real P1 in the prior fix, not a hypothetical transport failure:

```text
run retains commands receiver
  -> buffered SetupCommand retains reply sender
  -> submit awaits reply, retaining Arc<Shared>
  -> run awaits destruction of Shared
  -> receiver/reply would only be dropped after that await
```

Cancellation is selected before receiving a queued command because the worker select is biased. Thus an authorized request can be in the queue without ever reaching durable admission/owner execution when the worker exits its loop. Dropping only the worker shared reference cannot break the cycle.

### TDD and minimal correction

New public lifecycle/router regression: `setup_host_tests::shutdown_rejects_queued_request_before_joining_router_teardown`.

- Prepare an isolated neutral HTTP `SetupHost`, retrieve its protected capability, retain original admission bytes, and obtain its production router. No secure provider, domain provisioning, real profile or OS trust.
- Start two distinct valid capability-authorized Create requests **without polling the worker**. One receives 409 from the full one-slot queue, deterministically proving the other is queued and waiting for a worker reply. This uses public routing/response behavior, not a new queue-inspection hook, sleep, or scheduling assumption.
- Cancel before worker consumption and drop the original router. The initial regression joined queued request and worker under the unchanged five-second test bound. **Before the correction it failed in 5.04s**, exit 101, after a 1m10s compile:

```text
panicked at crates/unified-server/src/setup_host_tests.rs:137:6:
queued reply and worker shutdown must not retain each other: Elapsed(())
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 62 filtered out; finished in 5.04s
```

**Correction: six production lines** before owner unpublish and before the shared-state fence:

```rust
self.commands.close();
while let Some(mut command) = self.commands.recv().await {
    if let Some(reply) = command.reply.take() {
        let _ = reply.send(Err(StatusCode::SERVICE_UNAVAILABLE));
    }
}
```

Closing forbids new enqueues. Draining via `recv().await` until `None` also accounts for a capacity permit acquired immediately before close; a one-time `try_recv` snapshot could miss an in-progress sender. Explicit 503 replies release the corresponding `submit` waiters without executing, admitting or accepting unconsumed work. No dependency or public API change.

**Green:** exact regression passed in **0.04s**, after a **3m17s** rebuild. Then strengthened that same regression to retain one connection-equivalent router while consuming the rejected reply: queued request returns 503; the host guard is still held; a new authorized request through that retained router returns 409 promptly because the queue is closed; consuming/dropping the last router lets the worker complete; one immediate host/admission reopen succeeds with byte-identical admission. Final strengthened regression and existing setup-host cases pass. No private queue hook, timeout inflation, retry or forced unlock.

### Await and ownership review

- The current command and its reply are resolved/dropped within the worker loop before cancellation exits it; the active record is cleared there. Buffered commands are now rejected before any cleanup await.
- `commands.close()` runs synchronously before draining. `recv().await` can wait for an already-reserved sender to finish, but does not wait for the queued request to drop Shared: each received reply is resolved directly, and closed receivers reach `None` without requiring all sender handles to be destroyed. No lifecycle mutex is held across this await. Public submit only uses synchronous `try_send`, not a permit held across its own await.
- Owner withdrawal clears published normal-router/readiness references before the final Shared wait. The owner shutdown result is retained, not used to skip queue closure or teardown joining.
- Host owners/guard remain retained through both drain and shared teardown. The existing last-field notification still closes only after preceding shared admission/router state is destroyed. Independent in-flight handler references must genuinely finish; they are not forcibly invalidated or silently unlocked.
- Active-work cancellation, durable binding/retirement and ordinary source approval/authentication are unchanged. No request can now be newly enqueued after close; already queued, unconsumed work is never reported 202.

### Focused and final HTTPS validation

All Cargo commands used this qualification worktree manifest, main target cache, `CARGO_BUILD_JOBS=2`, `RUST_MIN_STACK=67108864`; Cargo tool bounds up to 1200000ms. No tool timeout, raised test bound or assertion relaxation.

| Validation | Actual result |
| --- | --- |
| `cargo test -p unified-server --lib setup_host_tests -- --nocapture --test-threads=1` | Final strengthened-source **5 passed / 1 parent-launched child ignore**, **0.65s**; both isolated TLS child modes actually passed. Earlier green suite before the added closed-queue/guard assertions: 5 passed, 0.36s. |
| `cargo test -p idp-server --no-default-features --lib hosted_setup::tests -- --nocapture --test-threads=1` | **2 passed**, **0.03s**, after 1m18s build. Existing cancellation/retirement and disconnect behavior retained. |
| Execution-only filter `setup_host_tests::shutdown_`, 50 serial repetitions | **100 lifecycle test executions passed**: queued-reply regression plus original router-teardown/guard regression in every subprocess. Each subprocess bounded at 30s. |
| Execution-only retained Join mismatch/missing-evidence test | **1 passed**, all existing fault variants; unchanged security assertions. |
| Execution-only partial Create/Join credentials recovery test | **1 passed**, **1.90s**, unchanged retained-recovery/refusal variants. |
| `cargo clippy -p idp-server -p unified-server --no-default-features --lib --no-deps -- -D warnings` | **Passed**, **5.92s**, production libraries scoped as stated. |
| `cargo fmt --all -- --check`; `git diff --check` | **Passed**. |

Final-code executable SHA-256 **59979feae70512b228bc62a6ec7d74db576a5691dd2da0c296052526bf61ae28**. No compilation was launched during the timing-sensitive runs. A `/proc` snapshot before **each** suite found no `cargo` or `rustc` process (`BUILD_SNAPSHOT []`); this is not certification of a globally idle OS or absence of unrelated user/browser/editor activity.

Executed the same final binary serially three times:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 /home/nathan/code/rust/aicacia/of/target/debug/deps/unified_server-f38d95073d23ab7e production_api_join_ --nocapture --test-threads=1
```

| Start (2026-10-25, -0400) | Finalization child | Offline-reopen child | Interruption/exact-retry child | Suite |
| --- | --- | --- | --- | --- |
| 16:04:52 | pass 100.63s | pass 97.72s | pass 89.97s | **3 passed**, 288.61s, exit 0 |
| 16:09:41 | pass 99.74s | pass 99.66s | pass 91.22s | **3 passed**, 291.07s, exit 0 |
| 16:14:32 | pass 99.21s | pass 91.88s | pass 88.33s | **3 passed**, 279.96s, exit 0 |

Actual summary for every suite:

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 60 filtered out
```

All three parents launched their isolated real-Iroh/native-root children; every child passed. Existing negative authentication/Origin/intent checks, 15-second source-offer polling, five-second production HTTP request bound, readiness checks, exact retry and retained/offline identity assertions were unchanged. Each suite had a 1000s subprocess ceiling; the full focused-plus-three-suite tool invocation had a 1200000ms ceiling; neither fired. No final-code pending-offer, request, admission-lock or teardown timeout occurred in these **nine** scenarios.

Disposable capture `/tmp/07-p1-final-serial-https.log`, SHA-256 **39b61ce7912bcd61640a774953a62280ab9ab38c7300d4035733582f8df57828**. It records the no-compiler snapshots, each actual stdout followed by stderr, and full test summaries; streams are not claimed interleaved in original emission order. Historical failing receipts above are not rewritten or attributed to this queue defect.

### Latest disposition and boundaries

**Reviewed P1 fixed**, with deterministic failing-before/passing-after evidence and final serial HTTPS **9/9 passing**. The original teardown fence remains, now ordered after queue closure/rejection. Ticket07 remains **open** and its two integrated durability boxes remain conservatively unchecked: this is additional bounded passing evidence, not a proof that the earlier pending-offer/ordinary-approval HTTP timeout family has been eliminated. No common root-cause or baseline attribution is asserted from these passes.

Read the integrated [validation follow-up](07-validation-followup.md): its executed exact matrix failed in owner-boundary tests (**18 passed / 26 failed / 6 child ignores**, remaining matrix combinations unrun), not a compilation timeout. Its CRAP attempt was stopped after **435.34s**, exit **143**, when available memory stayed below 1.5 GiB; no coverage completion or score. Those outcomes remain separate. This fix neither repairs nor labels those failures unrelated/baseline; no new matrix/CRAP certification is claimed. No need to repeat the completed heavy validation just to obscure its concrete failures.

Sibling revisions before and after these tests: `ofdb` **e3242170c94d5fc4bcc121db812310c8106c7ab6**, `ofnet` **dbea925a0a3f99053fffc02618baccc2a1cbea9f**, `offs` **df4c696a910296635473ec296d9235acbaa60d9b**. They were not modified by this agent. Main lock SHA-256 remained **bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f**; only the generated worktree lock refresh is reverted/excluded. Delivery owns only the shared-worker correction, lifecycle regression, this evidence and ticket07. Integration tip must be merged again before reporting; no merge back is authorized.
