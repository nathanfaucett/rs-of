# Ticket 08: provider ownership isolation

## Scope and provenance

- Claimed ticket 08 before implementation on `implement/setup-followup-08`, worktree `/home/nathan/code/rust/aicacia/of/.worktrees/setup-08`.
- Initial HEAD, requested base, and integration merge-base all verified as `da5bc63419623ab48f93dea14dfce20f3e10ffc9`; initial worktree clean.
- Read root AGENTS, issue-tracker instructions, shared and IdP glossaries, IdP local-security contracts, setup spec, ticket 07 and its validation diagnosis. Loaded TDD and ponytail.
- Authorized seam: public private-key repository ownership/load/store/delete/reopen and existing concurrent owner-boundary acceptance tests. No new authorization seam or real credential-provider/profile/trust access.
- Complexity/risk/priority: low implementation complexity, security-sensitive provider selection, high priority. Retain an existing `Arc` and build entries directly; no dependency, provider registry, suite lock, sleeps, retry loop, relaxed assertion, or authorization change.
- Sibling dependency HEADs observed: ofdb `e3242170c94d5fc4bcc121db812310c8106c7ab6`, ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, offs `df4c696a910296635473ec296d9235acbaa60d9b`. No sibling source changes. Added only worktree-layout symlinks `.worktrees/{ofdb,ofnet,offs}` to the existing sibling repositories so unchanged relative Cargo paths resolve; these are not committed source changes.

## Implementation

`PrivateKeyKeyringRepo` now owns its supplied backend. `new_with_store` no longer installs a process default. Load/store/delete build entries directly through that backend, retaining the existing IdP modifiers, names, encoding and error handling. A repository reconstructed with the same store retains the signer independently of other repositories' construction and lifetimes.

Default-constructed private and raw repositories capture the startup default once rather than resolving a mutable global for each operation. Initialization honors an explicitly installed startup provider and retains the existing failed-initialization retry/concurrent-initialization behavior.

Owner-boundary fixtures whose production owner/runtime APIs reopen via the default constructor now use one `OnceLock` startup mock backend, with existing unique issuers/namespaces isolating credentials. The identity seeder and native fixture no longer alternate fresh/global providers. An already-installed isolated child provider is retained, not replaced. Removed the duplicate native mock adapter.

Callers that intentionally installed a startup provider via a discarded `new_with_store` return value now explicitly call `keyring_core::set_default_store`: isolated native/HTTP/setup children, host fixture provider setup, full-Node provider composition and the system-test service entry point. This preserves their intended provider selection rather than silently falling back to an OS backend after removing the constructor side effect. No production service authorization is changed.

The process default remains a startup-only selection, not an API for dynamically moving live default-opening hosts between providers. Explicit host fixture default installation is still visible at those call sites; this is not a guarantee of arbitrary concurrent default replacement or all host fixture suites. Independent backends should use the repository injection seam and retain that backend for reopen.

## Deterministic red → green

All Rust commands used:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864
```

as environment assignments before Cargo. No concurrent Rust build commands, full feature matrix or coverage/CRAP run. Commands were bounded to 600000 ms, except formatting/diff checks (60000 ms). No command timed out.

1. Added `repo::provider_isolation_tests::overlapping_private_repositories_retain_signers_through_reopen` before changing production code. Two threads use channels to force A-store → B-construct/store → A-read/reopen/delete → B-read/reopen. Both stores use the same service, namespace and path but different fixed mock signer material. No scheduler luck or sleeps.
   - Command: `cargo test --locked -p idp-service --lib overlapping_private_repositories_retain_signers_through_reopen -- --nocapture`.
   - **RED** on original production code: exit 101; A loaded B's signer, failing `another fixture cannot replace the first signer`. The other thread then received channel disconnect, not a hang. This directly reproduces the ownership defect, not the entire historical matrix sequence.
   - **GREEN** after direct backend ownership: included in all unit runs below; A and B retain their own signer through overlapping lifetimes/reopen, and deleting A does not delete B.
2. Added `repositories_pin_the_startup_default_and_injection_does_not_replace_it`. Installs only mock startup providers, verifies explicit injection leaves the startup default unchanged, and verifies captured private/raw repos survive subsequent default replacement, including raw deletion.
   - Temporarily retained the original global-lookup behavior specifically in raw load while testing this second slice.
   - Command: `cargo test --locked -p idp-service --lib repositories_pin_the_startup_default_and_injection_does_not_replace_it -- --nocapture`.
   - **RED**: exit 101; raw load returned `None` instead of the previously stored mock secret after default replacement.
   - **GREEN**: raw load uses its captured backend; final 16-test library run passes.

## Executed validation

| Command (with common Rust environment above) | Actual outcome |
| --- | --- |
| `cargo test --locked -p idp-service --lib` | First green run: 15 passed. Final run including startup/default regression: **16 passed**, 0 failed, 0 ignored. |
| `cargo test --locked -p idp-unified --test owner_boundaries --no-run` | Passed; rebuilt current worktree sources in 9m49s. Executable `target/debug/deps/owner_boundaries-fbdb13f2c672f60d`. |
| `cargo test --locked -p idp-unified --test owner_boundaries -- --skip owner_composition_tests:: --skip generated_rust_and_built_javascript_use_actual_owner_routes` | **35 passed**, 0 failed, 0 ignored, 15 filtered out; 251.64s test time. Normal harness concurrency: no `--test-threads`, no `RUST_TEST_THREADS` override. |
| `cargo test --locked -p unified-server --test http_host http_setup_create_and_reopen_without_certificates -- --exact --nocapture` | **Parent passed and its isolated child passed**; 49.90s parent test time after 6m25s build. Child invoked by its parent, not manually. |
| `cargo clippy --locked -p idp-service --lib -- -D warnings` | Passed. |
| `cargo clippy --locked -p idp-service --tests -- -D warnings` | **Blocked**, exit 101: pre-existing `clippy::items_after_test_module` in `crates/idp-service/src/oauth2/config.rs:52`, implementation follows at line 65. File unchanged; no lint suppression. |
| `cargo fmt --all -- --check` | Initially found formatting in two changed fixture files; formatted only those files and final check passed. |
| `git diff --check` | Passed. |

The 35-test run includes native owner-cutover/capture/reopen and local token checks, native API/catalog/socket denial and learned-revocation cases, both normal permission HTTP cases, policy cleanup, Node Join bootstrap/offline reopen, all three bootstrap interruption/orchestration cases, residency and Device authority, and the remote identity/Device process parents (their guarded child functions also appear as no-op successes in the parent harness). No missing-signer failures recurred. Local untracked execution log: `/home/nathan/code/rust/aicacia/of/target/setup-08-owner-boundaries.log`.

## Limits and disposition

- **Ticket 08 resolved at the requested ownership seam.** No claim that this proves all 19 historical missing-signer failures had exactly this store-switch sequence.
- The 15 filtered cases include the independent owner-composition group and built-JavaScript prerequisite scenario. These are explicit scope exclusions, not passing results. The separate readiness/request timeout, SDK prerequisites, historical-reset fixture contract, and provisioned full-validation families remain with tickets 09–11 / ticket 07.
- No full matrix, `just crap`, coverage, GUI, physical/WAN, cross-platform or real OS provider/trust qualification. The native provider lifecycle/full-Node/system-service startup-call migrations were reviewed/formatted, not separately executed here. No real providers, profiles or trust stores were used.
- Test-target strict Clippy remains blocked as described; production-library strict Clippy passes. No provider-isolation blocker remains from the executed scope.
- Worktree and main `Cargo.lock` both retain baseline SHA-256 `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`. No map, other ticket, dependency version or sibling source edits belong to this change.
