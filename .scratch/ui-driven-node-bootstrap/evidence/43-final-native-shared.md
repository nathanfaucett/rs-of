# Receipt 43 — Final native/shared-unified regression qualification

## Source and scope

Working directory: MAIN `/home/nathan/code/rust/aicacia/of`, branch `integration/ui-driven-node-bootstrap`. Started from clean `a4caa2f712ed9173ccfad95a9a132aea6912752e`, after reading [receipt 42](42-finalreview.md) and [receipt 41](41-management-cleanup-client.md). This is the requested final native/shared check after the common setup HTTPS startup probe switched to OPTIONS and signer-trust conflict checks were batched.

MAIN advanced during execution to `ea91aa73f703c8ca55825802c8e3bb56c7f13526`, the parent's documentation-only reconciliation. Its changed files are README, `NATIVE-SETUP.md` and public docs. The literal comparison below passed with no output:

```sh
git --no-pager diff --exit-code a4caa2f71..HEAD -- '*.rs' '**/Cargo.toml' Cargo.lock .cargo/config.toml rust-toolchain.toml
```

The tested Rust source is unchanged. **No source fix was needed or made. This pass adds only this receipt.** No tickets, acceptance statuses, public documentation, frontend, generated clients, dependency manifests or other agent work were edited.

Complexity/risk/priority: narrowly scoped regression qualification of high-priority shared lifecycle/security changes. Reuse the existing isolated parent/child test wrappers, not a new test framework or broader matrix. Critical seams are HTTPS listener/worker ownership, setup admission and Origin, retained identity, protected-provider reset ordering, authenticated Create/Join and offline reopen.

## Controls and limits

- One exclusive Rust token; one Cargo command at a time. Every Cargo tool invocation used `timeout_ms:200000`.
- Receipt-36 profiles, MAIN target directory, offline/locked, one build job, incremental disabled, `RUST_MIN_STACK=16777216`.
- Selective debug 0 / opt-level 0 for `lidp` and `unified-server`; Storage/IdP/Management server debug 0. Other workspace/dependency profile settings remain unchanged; the Cargo banner still says `[optimized + debuginfo]`.
- Unchanged scenario/client bounds: native provider parent 90 seconds / readiness poll 60 seconds; unified API Create parent 90 seconds; existing whole production HTTPS parent 300 seconds, with the 200-second outer tool bound here. No source, assertion, retry, HTTP timeout or deadline was changed.
- Native tests inject trust command outcomes. Provider/HTTPS wrappers explicitly launch isolated children with in-memory protected providers and temporary native certificate roots. No real OS trust store, protected provider, real profile, browser trust setting or GUI was modified/run.
- No independent rerun, Clippy sweep, workspace/feature/platform matrix, Cargo Hack or CRAP was performed. Receipt 42 retains the current/historical independent Create/Join, transfer, actual signer-free operation and strict affected-library qualification; receipt 41 retains SDK/frontend qualification.
- Native test qualification is **not rendered native GUI certification**. The five inherited Svelte/Storage SDK export errors from receipt 41 were not rerun, fixed or represented as passing. No ticket is closed by this receipt; final acceptance remains with the parent.

## Results

| Check | Final result | Successful build/check and test elapsed |
| --- | --- | --- |
| Native `cargo check -p lidp --tests` | PASS, no diagnostics | 19.84s check |
| Native complete small library suite | **17 passed / 1 guarded child ignored in outer harness**, 0 filtered; the wrapper explicitly launched and passed that child | 2m32s build; outer suite 17.13s, child 14.48s |
| Unified `api_create_tests::` | **4 passed / 1 guarded child ignored in outer harness**; the production wrapper explicitly launched and passed that child | 0.32s build; module 15.65s, child 15.06s |
| Combined production HTTPS Create/Join/offline reopen | **1 parent passed / 1 explicitly launched child passed** | 0.31s build; parent 74.19s, child 74.10s |
| Whitespace and unchanged-source/provenance checks | PASS | No source diff; hashes below verified before commit |

The native suite includes listener/admission/Origin and trust/verification failures, retained port/host/TLS evidence, actual shared-worker Create/completed retry/offline reopen, mock-provider reset refusal/exact retry, retained Join references, local reset/recovery, peer revocation and completion-marker safety. The isolated child is intentionally not run directly without its parent's provider/root setup; it is not an unexecuted feature hidden by an ignore.

The Create module additionally preserves partial Create/Join credential recovery, malformed/conflicting retained-intent refusal, retirement/authenticated completed retry and offline reopen. Its intentional partial-setup attempt logged `succeeded=false` before exact recovery and success; no test failed. The combined HTTPS scenario exercised the existing full lifecycle and stopped/offline retained Node, including authenticated Storage catalog use. No zero-test filter is used as evidence.

### Timing observations from actual paths

The unified Create child logged: acceptance **1ms**, endpoint **19ms**, IdP **3701ms**, Management **3359ms**, runtime **157ms**, worker setup **7616ms**, observed ready **8147ms**. Stopped reopen logged runtime **1561ms** / ready **1563ms**. Acceptance is not completion.

The combined HTTPS child logged Join public bootstrap **21629ms**, login Client **1129ms**, runtime **527ms**; public imports were IdP **9282ms**, Management **6855ms**. Authenticated Storage catalog returned 200 in **1.296s** after retained reopen. The **74.10s** child total includes the larger unchanged scenario, not just fresh Join or seed work. These mock-provider/selective-profile observations are not a deployment latency SLA or a controlled benchmark attributing improvements to signer batching.

## Failure / timeout record

There was **one compilation-only timeout**, no compiler error and no failed test:

- First native library test invocation reached the 200000ms tool limit while compiling `idp-server` (captured build progress 835/845). No test had started.
- The process check below returned exit 1 / no matches. Retried the **identical native command**, with unchanged profile and tool/scenario bounds.
- Retry finished compilation in 2m32s and passed the entire 17-test outer suite plus its explicitly launched provider child.

All other requested Cargo commands completed on their first invocation. There was no need for a source-fix attempt or scope expansion.

## Literal Cargo commands

Each command ran from `/home/nathan/code/rust/aicacia/of` with tool bound **200000ms**. The native library command ran twice for the timeout/retry history above; the other three ran once.

### Native production/test-target compilation

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p lidp --tests --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

### All native library tests, existing isolated wrapper

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p lidp --lib --config 'profile.dev.package.lidp.debug=0' --config 'profile.dev.package.lidp.opt-level=0' --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Shared unified Create/recovery/offline module

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

### Combined production HTTPS Create/Join/offline reopen

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_https_create_join_and_offline_reopen_with_isolated_native_roots --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

## Cleanup and provenance

`git --no-pager diff --check` passed. Before starting, after the compilation timeout, and after the last test, the literal process check returned exit 1 / no matches:

```sh
pgrep -af '(^|/)(cargo|rustc|clippy-driver|rust-lld|mold|collect2|lidp-[0-9a-f]+|unified_server-[0-9a-f]+)( |$)'
```

The test wrappers joined/reaped their child processes. No Cargo/rustc/linker/fixture process remains running. This pass did not pause/resume analyzers or change editor configuration. Commit hooks are disabled explicitly for the evidence-only commit to prevent an automatic unqualified build; no further Rust command follows token release. The sole Rust build token is released after this commit and the final cleanup check.

SHA-256 values captured after all tests, before this evidence-only commit:

```text
772c3cbb21d67968a4e7fa104b08f3785a976fa5e4367af3e3d6eb785cc7bb84  Cargo.lock
365749d2cad7ed6aa8c84ba7a1aec23741525529da48fad241d9f335a6848356  apps/app/src-tauri/Cargo.toml
4a2798ba5b67d9267aad777f7cbc42f9d61f5f5330ef7ddd9489ca5286f31fdf  apps/app/src-tauri/src/native_host.rs
4aa6d624644f0036eb6ba99b2b06d8734f69666ff127280a01a921d6540a144b  apps/app/src-tauri/src/native_provider_lifecycle_tests.rs
29acd363a016984332e989444b761516d85de99fa07185c46355855c37b57dbc  apps/app/src-tauri/src/setup_reset.rs
2f56310f11ea8f661ec3c53f1a1738efa0131bd2e268680581214caf0f7e0288  crates/api/src/setup_server.rs
dfb137d389d9303a599232971ef4ccf5d757d9ff9cdc12c3f7e349243cf5169c  crates/idp-service/src/replica/replica_signer_repo.rs
d5b935a5bd64903eb3ebc042db21e2ebaa60f071eeaba2d5773811a605f3f9e3  crates/unified-server/src/api_create.rs
0df16367c7eebb0ab707adc1cdf0bdd2e2ddd38cf383fc5066dbf76074f391dc  crates/unified-server/src/api_create_tests.rs
cfc8c268274dd1c530d39671a337ec7a4177223bb19eb2dd00948840778b7a82  crates/unified-server/src/https_bootstrap_tests.rs
```
