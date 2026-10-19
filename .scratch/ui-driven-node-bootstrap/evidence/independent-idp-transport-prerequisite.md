# Independent IdP transport prerequisite — partial delivery, not ticket acceptance

Date: 2026-10-19. Worktree: `of/.scratch/worktrees/bootstrap-independent`, branch `implement/bootstrap-independent`. Base/integration tip: `bba9d49218fdc4cb0f91e0cc4df5e816c8dafa7b`. Design commit: `1a6cebc65cd919e995c5a776c27aa623e77a0502`. Tested implementation: `d9fe9ea05892abcb91f177ca6e4dbd417c9cbea5`. Integration merge after the implementation commit: already up to date.

## Acceptance limit

**The requested independent-owner vertical slice was not completed. Tickets 02/03 remain unresolved.** No independent Create/Join completion, independent worker cancellation/recovery/finalization or independent challenge exchange is qualified. There is no external design-decision blocker: the reviewed design is sufficient; those implementations are still missing. No ticket/status changes, new setup routes, generated contract/client edits, frontend edits, generic framework or placeholder workers were added.

The credential-handoff design was written before implementation in `../independent-trust-design.md`. It names each owner capability, consumer-local protected secret retention, exact HTTPS bindings, worker-only challenge initiation, single-use exact challenge registration, no redirect/reflection/replay, and lifecycle serialization. It is a design record, not implemented protocol evidence.

## Completed change

- Add `build_oauth_runtime` using the already-open IdP handle, existing secure-provider signer loading, existing normal router and remote Management permission configuration, without acquiring transport identity or Management state.
- Independent existing-state IdP CLI serving now uses that builder. It no longer opens/generates a Management transport key or starts an unused Iroh router merely to serve OAuth. Delete the obsolete rejecting data-protocol file.
- Keep composed `build_runtime` with its injected endpoint/private-key consistency check and explicit pairing controller. Do not equate the retained token-signer binding endpoint with a separate injected transport endpoint. Actual approved signer/private-key validation remains unchanged.
- RouterState has optional transport capability. Existing composed constructor supplies it; OAuth-only construction does not. Join approval/current enrollment verification require that explicit capability and fail closed if unavailable. Ordinary bearer/audience/action/target checks remain unchanged.
- Add a production-router regression: unrelated transport plus valid retained signer issues normal OAuth tokens; OAuth-only runtime does too; forged bearer denies; no Management Device owner is gained; stopped/offline IdP reopen still issues tokens after all endpoints close; unbound pairing denies; invalid retained signer denies. Test provider is the existing isolated mock provider. This is the router seam, not production independent setup HTTPS.

## Results

All Rust commands used the MAIN target directory, one Cargo job, incremental off, 16MiB stack, offline/locked, receipt33 selective profile, and tool bound 200000ms. No scenario bound was increased. No powersets/CRAP, privileged trust/provider changes, or actual GUI/deployment certification.

| Check | Result |
| --- | --- |
| Runtime regressions, final source | 2 pass, 10.91s; includes new independent signer/OAuth-only/offline-reopen checks |
| Remote-vs-local permission configuration and actual signer refusal | 1 pass, 3.95s |
| Source Join owner interleaving/revocation/reply-size | 3 pass, 28.81s |
| Source Join exact audience/both permissions/disconnected reply retry | 1 pass, 20.50s |
| IdP library Clippy, warnings denied | Pass, 0.29s final invocation |
| IdP all-target Clippy, warnings denied | FAIL inherited `clippy::items_after_test_module` at untouched `crates/idp-server/src/node_join.rs:275`, `invalid` after test module; not suppressed/fixed |
| Unified test-target check | Pass, 23.78s |
| Unified production HTTPS Create/authentication/recovery/offline reopen module | 4 pass / 1 deliberately ignored child, child explicitly launched and passed. Parent 15.28s, child 14.63s. Final compile 2m26s |
| Unified production HTTPS approved Join/authenticated Storage/source-offline reopen | Parent 1 pass, 50.98s; child 1 pass, 50.90s |
| Changed-file rustfmt, git diff --check, just boundary-check | Pass; no server-to-server dependency edges |

Create stage observations: acceptance 1ms; endpoint 21ms; IdP 3711ms; Management 3335ms; runtime 150ms; worker 7577ms; observed ready 8117ms; stopped reopen ready 1458ms. Join: public bootstrap 23383ms (IdP import 9670ms, Management 6886ms), login Client 1172ms, runtime 418ms, publication 729ms, completion 449ms. These are unified regressions, not independent-worker timings. Build time and mocked provider/trust costs are not installation performance claims.

Join emitted transient `endpoint is not allowlisted` refusals before retrying and completing within existing bounds, as receipt33 also observed. Cause not established; no denial was bypassed and no broader reliability claim is made.

## Red/green and failures

1. Initial worktree build refused missing sibling dependency paths. Added local ignored symlinks under `of/.scratch/worktrees/{ofdb,ofnet,offs}` to the existing sibling repositories; no manifests/dependencies or sibling source were changed.
2. First test compile used an unexported identity helper; corrected to existing `DeviceIdentity::new`.
3. Behavioral TDD red: the new independent-signer regression failed at the existing `local signer endpoint does not match the injected Node endpoint` guard (3.98s scenario). Remove that coupling while preserving endpoint possession and actual signer validation.
4. First green attempt got HTTP 422 because the test form omitted required `client_id`; corrected the fixture, not production parsing. It then passed (4.96s).
5. Extending the test to endpoint-free OAuth construction produced the expected missing-builder compile failure. Implement the actual builder and wire the independent CLI. One test compile then needed the existing pairing-controller trait import; fixed.
6. Strict all-target Clippy failed as recorded above. Library Clippy passes; no unrelated lint cleanup.
7. First unified Create invocation hit the 200s TOOL bound during compilation, before scenarios ran. Checked no Cargo/rustc/test child remained; retried the identical command at 200s. Final compilation and tests pass as recorded. No scenario deadline changed.

## Literal validation commands

Working directory for every command below: `/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-independent`. All terminal invocations bounded at 200000ms.

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib runtime::tests:: --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib permission_runtime_rejects_remote_database_configuration_before_opening_it --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib node_join_review_tests:: --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --lib node_join_requires_both_permissions_and_retries_after_disconnected_reply --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p idp-server --lib --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p idp-server --all-targets --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo check --offline --locked -p unified-server --tests --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0'
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_completion_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

```sh
rustfmt --edition 2024 crates/idp-server/src/runtime.rs crates/idp-server/src/router/state.rs crates/idp-server/src/router/node_join_approval.rs crates/idp-server/src/permission_evaluation_tests.rs crates/idp-server/src/node_join_review_tests.rs crates/idp-server/src/cli/run.rs crates/idp-server/src/lib.rs
git --no-pager diff --check
just boundary-check
GIT_EDITOR=true git merge --no-edit integration/ui-driven-node-bootstrap
```

## Remaining concrete work

IndependentSetupHost still serves only awaiting-choice status/admission and refuses retained partial domain state. Each startup still diverts fresh owners into that host. App independent mutations still reject. Implement the three concrete owner workers, durable exact operation/recovery/cancel/finalization and same-listener normal-router dispatch; implement the recorded recipient-generated secure credential/challenge handoff through the existing plaintext Client constructor; perform separate approved Join public-state import without copied endpoint keys or cross-owner opens; wire the existing app flow and regenerate contracts only if changed; qualify actual independent HTTPS Create/Join/interrupted retry/offline reopen/no-secret disclosure. The passing unified regressions above do not satisfy those obligations.
