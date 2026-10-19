# Receipt 36 — Native shared-host lifecycle and final HTTPS qualification

## Scope, source and acceptance boundary

Validation ran in MAIN `/home/nathan/code/rust/aicacia/of`, branch `integration/ui-driven-node-bootstrap`, starting at `41ad8bc47` after the frontend adopted `apps/app/src-tauri/NATIVE-SETUP.md` (receipt 35). The native reconciliation is the implementation from `3202a8e69`, already integrated before this run.

During validation MAIN advanced to `5e8b29c82ae2f33e0167a3f0d122ece7920f2210` (documentation reconciliation only). `git diff --exit-code 41ad8bc47..HEAD -- '*.rs' '**/Cargo.toml' Cargo.lock .cargo/config.toml rust-toolchain.toml` passed: the tested Rust sources/manifests/configuration did not change. No runtime, test, frontend or generated-client source fix was needed or made in this qualification pass. This receipt is the only new file from this pass.

**Tickets remain open pending final acceptance review.** This qualifies the native/shared-worker and production HTTPS seams below, not rendered GUI approval, real OS CA/provider installation, physical peer discovery, WAN deployment or other platforms. Frontend adoption is recorded in receipt 35; its frontend suites were not rerun here and no TS/Svelte was edited.

Complexity/risk/priority: validate the high-priority security/lifecycle reconciliation without creating another orchestrator. The critical seams are exact host selection, setup admission/Origin, one listener/endpoint owner, retained identity and attempt evidence, provider-first reset, and approved Join/offline reopen. Existing tests were executed unchanged; there was no weaker assertion, ignored failing test, new timeout, or broad repair.

## Environment and resource controls

- Ubuntu Linux x86_64, kernel `6.8.0-147-generic`.
- `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, LLVM `23.1.1`; `cargo 1.99.1 (5f94df478 2026-08-27)`.
- MAIN's existing `target` directory; offline/locked, `CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, `RUST_MIN_STACK=16777216` on every Cargo build/test/Clippy command.
- Receipt 33 selective dev profile, not a global unoptimized build: native and unified packages debug 0 / opt-level 0; Storage/IdP/Management server debug 0. Other workspace/dependency settings retain their configured optimization/debug behavior. Cargo's overall banner still says `[optimized + debuginfo]`.
- Exactly one Rust command at a time, each with `timeout_ms:200000`. After every compilation timeout, the process check below returned no matches before the identical command was retried.
- Existing scenario bounds were unchanged: native provider parent 90s / readiness poll 60s; independent parents 90s; unified API Create parent 90s; the existing production HTTPS bootstrap parent 300s remains subject to the 200s outer tool bound here. Existing client and local poll timeouts were not altered.
- Native trust tests inject command outcomes. Native provider lifecycle and production HTTPS parent harnesses use isolated temporary stores, in-memory protected providers and test-only native certificate roots in their children. No production trust-store command, real secure-provider mutation, real profile reset, browser warning bypass, or GUI run was performed.

## Results on this source

Build times below are Cargo-reported successful-invocation times; tests are harness-reported elapsed times. Compilation-only timeout history is recorded separately, not counted as passing execution.

| Seam / command filter | Final result | Successful build / test time |
| --- | --- | --- |
| Native production and test-target `cargo check -p lidp --tests` | Pass, no diagnostics | 23.19s check |
| `native_host::tests` | **5 passed**, 13 filtered | 12.31s build; 2.42s tests |
| `native_shared_create_reopen_and_provider_safe_reset` | **1 parent passed**, explicitly launched **1 child passed** | 1.70s build; parent 15.08s / child 15.01s |
| `setup_reset::tests` | **6 passed**, including retained Join references | 0.53s build; 0.24s tests |
| `setup_peer_revocation::tests` | **1 passed** | 0.48s build; 0.04s tests |
| `localhost_trust::tests` | **2 passed** | 0.49s build; 0.00s tests |
| `setup_completion::tests` | **2 passed** | 0.51s build; 0.01s tests |
| Strict library Clippy: `lidp`, `unified-server`, `api` | **Pass**, `-D warnings`, no diagnostics | 45.06s |
| Unified `api_create_tests::` | **4 passed / 1 deliberately ignored child**; parent explicitly launched that child, which passed | 1m47s build; module 17.60s / child 16.72s |
| Unified `production_api_join_completion_and_offline_reopen` | **1 parent passed / 1 launched child passed** | 1.00s build; parent 49.56s / child 49.49s |
| Whole `production_https_create_join_and_offline_reopen_with_isolated_native_roots` | **1 parent passed / 1 launched child passed** | 0.37s build; parent 80.30s / child 80.20s |
| Unified `production_api_join_interrupted_retry_and_rejection` | **1 parent passed / 1 launched child passed** | 0.34s build; parent 45.69s / child 45.54s |
| Independent `production_independent` | **2 parents passed / 2 launched children passed**, Create and Join with offline reopen | 0.35s build; parents 82.52s total; children Create 25.18s / Join 57.08s |
| Shared runtime `builds_three_prefixed_services_with_one_endpoint` | **1 passed**, including stopped reopen | 0.37s build; 72.75s tests |
| Scoped Rust formatting, whitespace, server dependency boundaries | **Pass** | No direct server-to-server dependency edges |

The focused native filters ran all **17 non-ignored native tests**, plus the provider child explicitly launched by its parent. No zero-test selection is used as evidence.

### What the native seams prove

- The real shared setup HTTPS routes report one unified owner scope; unauthorized Create/Join deny without opening domain owners. The exact native UI Origin is accepted and an unrelated browser Origin is denied.
- Neutral startup and Create retain one listener/address. Close joins the listener/shared worker; the port and lifecycle guard can reopen. The provider lifecycle exercises actual shared-worker Create, submitting-response drop, completed authenticated retry, stopped/offline reopen, and stable public Client/host/owner journal.
- Addressing the other selected host does not provision the unselected bundled host: it stays `awaiting_choice`, with no IdP, Management, Storage or endpoint root. This is an HTTP/native-host seam test, not rendered frontend certification.
- Injected trust refusal and empty-root HTTPS verification failure preserve TLS identity, do not start domain owners, and release the listener for exact retry. Lost host/admission/TLS files refuse without replacement.
- Provider refusal during reset leaves host/owner manifests and the reset journal intact. Exact retry deletes protected inventory through its owner before data; data-pending retry does not reopen a provider. Missing identity evidence requires explicit exact reference recovery; unexplained owner state and symlink evidence deny.
- The retained Join-reference test uses the public shared BootstrapConfig/CredentialBootstrap/LocalJoin seams before owner production. It proves exact issuer/IDs/history survive refusal, lost-reference recovery and reset without starting an endpoint/provider. It is **not** a second native full Join/approval qualification; real approved Join/import/offline use is qualified by the production shared and independent HTTPS parents above.
- Peer revocation is a focused isolated HTTPS peer fixture: an issuer mismatch receives no User token; each owner receives its exact audience exchange; partial Device denial retains a pending target; acknowledgement occurs only after signer and Device deletion succeed. It does not claim real peer permission-server qualification by itself.
- Device completion markers preserve interruption/corruption behavior and never follow a symlink. They are not an API authentication-enable switch.

### Shared production evidence and timing observations

The unchanged unified Create/recovery test observed 1ms acceptance, 24ms endpoint preparation, 4288ms IdP work, 3996ms Management work, 154ms runtime construction, 8873ms worker setup, and 9456ms observed ready. Stopped reopen logged 1513ms runtime / 1515ms ready. Acceptance is not completion; these mock-provider/profile observations are not a fresh deployment performance SLA.

The API Join test logged public bootstrap 23590ms, login Client 1297ms, runtime 454ms, publish 848ms and completion 514ms, and verified authenticated Storage use before and after source-offline reopen. The whole HTTPS regression additionally exercises the existing full lifecycle and retained local trust. The one-endpoint runtime test is the existing loopback HTTP composition/authentication seam; it is not substituted for the production HTTPS tests.

The interrupted/rejection test intentionally logged `Node Join stage=public_bootstrap error=EOF while parsing a value at line 1 column 0` before its authorized exact retry. The Create module intentionally logged an initial `succeeded=false` setup attempt before recovery. Both scenarios passed unchanged; those injected refusals were not repaired, suppressed or presented as unexpected regressions.

## Compilation timeout record

There were **14 compilation-only tool timeouts**, no compiler/test failure: 12 while first building the native test graph and 2 while rebuilding the separate unified test graph. Every timeout used exactly the same command, profile and 200000ms tool bound as its successful retry. No test had started in these timed-out invocations.

| Native `native_host::tests` invocation | Last captured build item |
| --- | --- |
| 1 | `derive_more-impl`, 150/845 |
| 2 | `automerge`, 271/845 |
| 3 | `time`, 365/845 |
| 4 | `tungstenite`, 469/845 |
| 5 | `rustls-platform-verifier`, 564/845 |
| 6 | `strum_macros`, 603/845 |
| 7 | `iroh-relay`, 693/845 |
| 8 | `iroh`, 731/845 |
| 9 | `serde_repr`, 778/845 |
| 10 | `tauri-plugin-opener`, 818/845 |
| 11 | `idp-server`, 835/845 |
| 12 | `lidp(test)`, 844/845 |

Invocation 13 completed compilation and passed the five tests. Unified `api_create_tests::` timed out at `storage-server` (490/494), then `management-server` (492/494); invocation 3 compiled in 1m47s and passed the module. There was no scenario deadline extension and no lingering Cargo/rustc/test child was reused to manufacture a result.

## Literal Rust commands

Working directory for every command: `/home/nathan/code/rust/aicacia/of`. Each Cargo invocation used the tool bound `200000` milliseconds. Repetitions are counted above; commands were identical, not rewritten to increase their bounds.

### Native compilation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p lidp --tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### Native listener, admission, Origin and trust/verification failure

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib native_host::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native shared Create, offline reopen and mock-provider reset

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib native_shared_create_reopen_and_provider_safe_reset --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native reset and retained Join references

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib setup_reset::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native peer revocation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib setup_peer_revocation::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native injected trust commands

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib localhost_trust::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Native completion marker safety

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib setup_completion::tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Strict affected-library Clippy

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p lidp -p unified-server -p api --lib --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

### Unified production Create/recovery/offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Unified production API Join/offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_completion_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Whole production HTTPS Create/Join/offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_https_create_join_and_offline_reopen_with_isolated_native_roots --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Unified Join interruption/rejection/exact retry

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_interrupted_retry_and_rejection --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Independent production HTTPS Create/Join/offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Shared runtime one-endpoint composition and stopped reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib builds_three_prefixed_services_with_one_endpoint --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

## Formatting, boundaries and process cleanup

```sh
rustfmt --check --edition 2024 --config skip_children=true apps/app/src-tauri/src/app.rs apps/app/src-tauri/src/native_host.rs apps/app/src-tauri/src/runtime.rs apps/app/src-tauri/src/lib.rs apps/app/src-tauri/src/setup_reset.rs apps/app/src-tauri/src/setup_reset_tests.rs apps/app/src-tauri/src/setup_completion.rs apps/app/src-tauri/src/setup_peer_revocation.rs apps/app/src-tauri/src/setup_peer_revocation_tests.rs apps/app/src-tauri/src/desktop_lifecycle_tests.rs apps/app/src-tauri/src/native_provider_lifecycle_tests.rs apps/app/src-tauri/src/unsupported_reset.rs apps/app/src-tauri/src/localhost_trust.rs crates/unified-server/src/api_create.rs crates/unified-server/src/setup_host.rs crates/unified-server/src/host_reset.rs crates/unified-server/src/lib.rs crates/unified-server/src/local_join.rs crates/unified-server/src/local_provisioning.rs crates/api/src/independent_openapi.rs
git --no-pager diff --check
just boundary-check
```

All passed. Boundary output: `No direct server-to-server dependency edges found.` No Cargo Hack, CRAP, workspace/platform matrix or real privileged installation was run.

After each timeout, this process check returned exit 1 / no matches:

```sh
pgrep -af '(^|/)(cargo|rustc|clippy-driver|lidp-[0-9a-f]+|unified_server-[0-9a-f]+)( |$)'
```

Final expanded check also returned exit 1 / no matches:

```sh
pgrep -af '(^|/)(cargo|rustc|clippy-driver|rust-lld|mold|collect2|lidp-[0-9a-f]+|unified_server-[0-9a-f]+)( |$)'
```

Native and production parent harnesses joined/reaped their explicitly launched children. No Cargo, rustc, Clippy, linker or fixture process was left running. The sole Rust build token is released at completion of this evidence commit; no further Rust work is authorized by this receipt.

## Source provenance

Sibling repository HEADs used by the path dependencies:

```text
ofdb  238210c5b822cd8b5f14274ea63d941ef884f6a0
offs  614f2e5f80d4184a67de6fc43a7d26b6e1000714
ofnet a001f45836393d61a98652294b6d9ea9a73dfcaa
```

SHA-256 values captured after the final production run and static checks, before this evidence-only commit:

```text
772c3cbb21d67968a4e7fa104b08f3785a976fa5e4367af3e3d6eb785cc7bb84  Cargo.lock
365749d2cad7ed6aa8c84ba7a1aec23741525529da48fad241d9f335a6848356  apps/app/src-tauri/Cargo.toml
4a2798ba5b67d9267aad777f7cbc42f9d61f5f5330ef7ddd9489ca5286f31fdf  apps/app/src-tauri/src/native_host.rs
29acd363a016984332e989444b761516d85de99fa07185c46355855c37b57dbc  apps/app/src-tauri/src/setup_reset.rs
4aa6d624644f0036eb6ba99b2b06d8734f69666ff127280a01a921d6540a144b  apps/app/src-tauri/src/native_provider_lifecycle_tests.rs
d5b935a5bd64903eb3ebc042db21e2ebaa60f071eeaba2d5773811a605f3f9e3  crates/unified-server/src/api_create.rs
3b4586aa211f0231ad8a7c142ceddd47006098647e99f9a913f6bfc478f1e7ef  crates/unified-server/src/setup_host.rs
b673abd76c2b2384568ce22ba83c2d96139c6b26cd0c0534cd6deb0f84b24974  crates/unified-server/src/host_reset.rs
9a473f2ae5dd073d8d06ba75ab75c23874fddcdea0020bf39d6233d64849a24f  crates/unified-server/src/local_join.rs
0d67e4c130a78b9e10dbfc91f5195274444044e949e230314b0def812997772d  crates/unified-server/src/local_provisioning.rs
23fea3ea3f802a69f0236b043e8b452157cb96f2ef4823de1643e940fbbd0d7c  crates/unified-server/src/independent_create_tests.rs
9448240daadd22fe34c0aa773d97516bf55c2054f78f65548b5e23532bf16d40  crates/unified-server/src/independent_join_tests.rs
```
