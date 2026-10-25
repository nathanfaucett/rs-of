# Ticket 07 validation follow-up: read-only diagnosis

## Scope and base

2026-10-25. Isolated worktree `/home/nathan/code/rust/aicacia/of-setup-validation`, branch `setup-simplification-validation`, HEAD `54816fa00764ff81ccc0811f560b77103a4a4253` (requested integration `54816fa00`). Clean at entry. Read AGENTS, spec, ticket07, map, both 07 qualification/review receipts, issue-tracker guidance, justfile, root Cargo.toml and .cargo/config.toml; inspected relevant build scripts, registry Swagger implementation, fingerprints and artifacts.

Only this new evidence file is delivery. No build, test, matrix, coverage, network download, cache repair, source/config/lock modification or OS trust/profile operation was performed. No ponytail/TDD invocation was needed: no code fix is proposed or applied. Any subsequent code/config fix requires coordination first and those skills where applicable. Existing receipts/ticket/spec/map remain untouched; qualification remains open. No commit made.

Main `of/Cargo.lock` was already modified at entry; SHA-256 before and after inspection is `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`. Worktree lock changes are not delivery. Editor tools reject this worktree as outside mounted roots; inspection used absolute paths and `git -C` from mounted `of`, never a branch switch.

## Swagger: exact cause and artifacts

Installed `utoipa-swagger-ui 9.0.2` build script:
`/home/nathan/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/utoipa-swagger-ui-9.0.2/build.rs`.

Observed build-script fingerprints enable only `axum`, not `cache`, `reqwest`, `url` or `vendored`. Therefore “cached” means a file inside that build invocation’s OUT_DIR, **not** an OS-wide Swagger cache. Default URL is `https://github.com/swagger-api/swagger-ui/archive/refs/tags/v5.17.14.zip`.

| Artifact | Bytes | SHA-256 | Inspection |
| --- | ---: | --- | --- |
| `/home/nathan/code/rust/aicacia/of/target/debug/build/utoipa-swagger-ui-2b6d48cbe457daf8/out/v5.17.14.zip` | 4,388,280 | `481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc` | Valid ZIP; all 1,097 entries pass CRC (`ZipFile.testzip()` returns None); contains `swagger-ui-5.17.14/dist/swagger-initializer.js`. |
| `/home/nathan/code/rust/aicacia/of/target/debug/build/utoipa-swagger-ui-7787c518a6cb5c2e/out/v5.17.14.zip` | 4,388,280 | same | Same bytes/hash, valid ZIP directory. |
| `/home/nathan/code/rust/aicacia/of/target/debug/build/utoipa-swagger-ui-4171541ce4ca67c5/out/v5.17.14.zip` | 4,388,280 | same | Same bytes/hash, valid ZIP directory. |
| `/home/nathan/code/rust/aicacia/of/target/llvm-cov-target/debug/build/utoipa-swagger-ui-d07eaf3bfcc9abe6/out/v5.17.14.zip` | 970,752 | `5decad6d8c43865922ea2b0705b068875d8d1ba095b9fa2c1ffeb3ba50751800` | Invalid ZIP: **exact byte prefix** of the valid archive. Missing end-of-central-directory. |

This is an incomplete real archive, not HTML, a rate-limit/error body, an unrelated ZIP or a dependency-source failure. The three normal build outputs report successful download/extraction. The coverage copy has only about 22% of the archive.

Root cause of persistent retry failure is proven in the upstream build script: downloads use `curl -sSL -o <final-path>` directly (no temporary/atomic install, no `--fail`); existence alone selects reuse, followed by ZIP parsing. Interrupted/failed first downloads can leave a partial final file, which subsequent invocations never redownload. This exact prefix strongly supports an interrupted transfer; the historical interruption mechanism (tool timeout versus network/curl failure or another writer) is **not established**. Lack of `--fail` can also cache HTTP errors generally, but that is not the observed file here. No need to patch upstream code, add vendoring/dependencies, alter certificates or change SDKs.

## Minimal repair proposal (NOT EXECUTED)

Coordinate with the HTTPS agent and ensure no Cargo/coverage writer uses the shared target first. Replace only the identified bad ZIP from the already CRC-checked good local copy, with hash/shape guards and an atomic same-directory rename. No network needed; do not clean target, delete fingerprints, or copy whole build output directories.

Run this Python snippet only after coordination:

```python
from pathlib import Path
import hashlib
import os
import zipfile

root = Path('/home/nathan/code/rust/aicacia/of/target')
source = root / 'debug/build/utoipa-swagger-ui-2b6d48cbe457daf8/out/v5.17.14.zip'
target = root / 'llvm-cov-target/debug/build/utoipa-swagger-ui-d07eaf3bfcc9abe6/out/v5.17.14.zip'
good, bad = source.read_bytes(), target.read_bytes()
assert hashlib.sha256(good).hexdigest() == '481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc'
assert hashlib.sha256(bad).hexdigest() == '5decad6d8c43865922ea2b0705b068875d8d1ba095b9fa2c1ffeb3ba50751800'
assert good.startswith(bad) and not zipfile.is_zipfile(target)
with zipfile.ZipFile(source) as archive:
    assert archive.testzip() is None
    assert 'swagger-ui-5.17.14/dist/swagger-initializer.js' in archive.namelist()
temporary = target.with_name('v5.17.14.zip.validation-repair.tmp')
with temporary.open('xb') as output:
    output.write(good)
    output.flush()
    os.fsync(output.fileno())
os.replace(temporary, target)
assert target.read_bytes() == good
```

This repairs the known failing OUT_DIR only. New feature/profile OUT_DIRs may still download independently; do not blanket-copy or enable new crate features. If a future download is required, download to a new staging file using `curl --fail --location --retry 3 --connect-timeout 20 --max-time 180 --output <staging-path>` with the exact default URL, CRC/shape-check before atomic installation, and record its hash. No network operation has been performed here. The existing local archive is structurally validated, not independently authenticated against an upstream published digest.

## Feature matrix and recompilation

Confirmed cost multipliers, not a proven Cargo defect:

- Root dev profile uses `opt-level = 1`; **all dependency packages use opt-level 3** via `[profile.dev.package."*"]`. Test compilation inherits dev defaults absent a test override. Cold dependency builds are therefore optimized, not cheap debug builds. Do not change delivered profiles merely to obtain qualification.
- `--all-targets` builds binary/examples/integration tests as well as libraries. Root dev-dependencies include all owners, generated clients, native database/network stacks and sibling path dependencies. The first root all-features run is already a large graph; earlier receipts stopped near completion of its first run, not evidence of hundreds of repeatedly completed matrix runs.
- Feature powerset intentionally creates different feature graphs. Stored fingerprints demonstrate multiple `api`/`unified-server` feature and profile variants; reuse across all combinations is not expected. Root has default cli/completions/swagger-ui plus the implicit optional unified-server feature. Prior reported run counts apply to that invocation, not a completed workspace matrix.
- `just test` explicitly uses `--workspace`; the historical raw command `cargo hack test --feature-powerset --all-targets` does not. Preserve and label their scopes; a root-only result is not workspace qualification.
- Coverage has a distinct `target/llvm-cov-target`, confirmed on disk. Instrumented compilation cannot reuse normal compiled objects unchanged. Alternating matrix and coverage is not one reusable cold build. `just crap` runs llvm-cov before cargo-crap and only runs the latter on success.
- Shared cache across changing worktrees/sibling sources can force legitimate rebuilds. `.cargo/config.toml` sets `CARGO_WORKSPACE_DIR` relative to each worktree; no local build script explicitly uses it, and scanned normal build-script JSONs contained no recorded reference to it. It is not proven to be the rebuild trigger. Local scripts are ordinary Tauri build/plugin calls. No active Linux linker flags are configured; mold is installed but disabled.
- No Cargo/Rust/Swagger cache environment overrides were present in this inspection shell and no config was found at inspected ancestor/home Cargo config locations. Historical command-specific environments still matter. No fingerprint trace from a repeated identical completed invocation exists in these receipts, so an exact unnecessary-recompilation trigger cannot be asserted read-only.

Minimal operational plan: after the idle-host gate, use this one fixed worktree, one fixed shared target, stable flags/toolchain and unchanged sibling inputs. Do not alternate with coverage or other agents’ builds, clean caches, add target triples/linkers, suppress features/dev-deps or change profiles. Capture Cargo fingerprint tracing on the real next matrix run to distinguish dirty inputs from expected new feature units. A timeout at 600 seconds compiling this graph is not a matrix failure assertion, nor proof Cargo discarded its cache. Prefer a longer coordinated bound over restarting it every ten minutes.

## Tools and resource snapshot

Inspection only; values change over time:

- rustc `1.99.1 (58cf8f9c3 2026-10-13)`; cargo-hack `0.6.44`; cargo-llvm-cov `0.8.7`; cargo-crap `0.5.0`.
- Available: just, curl, Python 3 (zipfile/hashlib), unzip, zipinfo, timeout, clang, mold. No sccache. No installation needed. `cargo hack --help` confirms partitioning but it was not used; no feature exclusions proposed.
- `target/debug` 157 GiB; `target/llvm-cov-target` 5.4 GiB; `target/flycheck0` 760 KiB; Cargo registry 1.5 GiB; Cargo git cache 12 KiB. Validation/review worktrees have no separate target directory.
- Filesystem: 937 GiB total, 176 GiB available (81% used); target and /tmp share the same filesystem.
- 16 logical CPUs; RAM 30 GiB total, about 6.8 GiB available, 643 MiB free at snapshot. Swap 2 GiB essentially full. Large editor/rust-analyzer processes and unrelated browser/wasm testing were active; none stopped. Memory PSI at subsequent snapshot showed no sustained immediate stall, so full swap alone does not prove active thrashing. `/sys/fs/cgroup/memory.max` was absent; no effective cgroup limit determined.
- Keep build jobs at 2 or lower once permitted; more jobs are not a justified remedy under this memory pressure. Do not terminate others’ processes. Shared `/tmp/lcov.info` is another reason to serialize coverage.

## Next commands: gated, NOT RUN

Run from `/home/nathan/code/rust/aicacia/of-setup-validation` only after the other agent confirms timing-sensitive runs are finished and shared-cache writes are safe. First recheck HEAD/status, main lock hash, free disk/RAM, and sibling revisions/status; record those inputs. Do not silently refresh a lock when using `--locked` fails. Matrix remains the exact requested command; add `--workspace` only for the separately identified workspace gate matching `just test`.

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 CARGO_LOG=cargo::core::compiler::fingerprint=info timeout --signal=TERM --kill-after=30s 7200s cargo hack test --feature-powerset --all-targets
```

The two-hour bound is a proposed scheduling ceiling, not a measured completion estimate. Capture output externally to an agreed disposable log. Do not run a separate warm-up compilation now. If repeated identical builds remain unexpectedly dirty, use the trace to identify exact changed inputs before any config fix. A separate workspace gate, when authorized, is the same environment with `cargo hack test --feature-powerset --workspace --all-targets`; no claim that the root gate covers it.

After the precise ZIP repair and matrix, with no other coverage job:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target CARGO_BUILD_JOBS=2 RUST_MIN_STACK=67108864 timeout --signal=TERM --kill-after=30s 7200s just --justfile /home/nathan/code/rust/aicacia/of-setup-validation/justfile --working-directory /home/nathan/code/rust/aicacia/of-setup-validation crap
```

Do not add `--no-clean` or reroute instrumented artifacts into normal debug output without separately understanding llvm-cov behavior. A repaired archive clears only this known build-input blocker; later compilation/tests/CRAP may still fail. No complete matrix, coverage result, score, HTTPS reliability or acceptance is claimed. Await follow-up before executing repair/heavy validation.


## Authorized execution follow-up (2026-10-25)

The user authorized the narrow cache replacement and one attempt each at the exact matrix and `just crap`, generously bounded up to 1,800,000 ms, with resource safety taking precedence. This section supersedes the earlier “not executed” scheduling status, not its historical inspection findings. No source/config/test/profile/lint changes were made. No ponytail/TDD was required because no code fix was attempted.

### Integration and cache repair

Before execution, `git merge --no-edit setup-simplification` in this isolated branch reported **Already up to date** at `54816fa00764ff81ccc0811f560b77103a4a4253`. The same merge check after execution again reported Already up to date at that tip. No other agent’s source changes were merged or altered.

Executed precisely the guarded Python repair above: asserted both original hashes and exact-prefix relation, checked every good ZIP entry CRC and initializer presence, wrote a new exclusive same-directory `v5.17.14.zip.validation-repair.tmp`, flushed/fsynced it, and `os.replace`d only the identified bad coverage ZIP. Result: 4,388,280 bytes, SHA-256 `481244d0812097b11fbaeef79f71d942b171617f9c9f9514e63acbe13e71ccdc`. Final inspection confirms the repaired path still has that size/hash. No broad cache deletion or new download. The repair itself is verified; coverage did not get far enough to certify its subsequent Swagger build-script execution.

### Execution method and resource guard

Both subprocesses ran with cwd `/home/nathan/code/rust/aicacia/of-setup-validation` and exactly:

```text
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
CARGO_BUILD_JOBS=2
RUST_MIN_STACK=67108864
```

A Python supervisor started each command in its own process session, redirected all output to its own disposable log, sampled resources every five seconds, and would terminate only its process group on: three successive samples below 1.5 GiB MemAvailable, less than 20 GiB disk free, or 1,760 seconds elapsed (allowing graceful/forced termination within the tool’s 1,800,000 ms bound). SIGTERM, then SIGKILL if needed after 15 seconds. No fingerprint logging override, extra feature restriction, thread/test filter, assertion change, profile change or lint suppression was used.

At start: 4.4 GiB MemAvailable, essentially full 2 GiB swap, 251 GiB free disk. Before coverage: 2.9 GiB MemAvailable and 251 GiB free disk. This allowed a guarded attempt, not unlimited parallelism. No other agent’s/editor/browser processes were stopped.

### Exact matrix outcome: FAILED, not timeout

Command: `cargo hack test --feature-powerset --all-targets`.

- Supervisor elapsed **875.89 s**; cargo-hack exit **1**, inner Cargo exit **101**; no resource/timeout guard fired. Minimum sampled available RAM **1,912,184,832 bytes**.
- First run: `cargo test --all-targets --all-features` on root `idp-unified` **(1/10)**. Compiled successfully: `Finished test profile [optimized + debuginfo] target(s) in 11m 13s`.
- Root main unit executable: 0 tests, successful. Then `owner_boundaries-d4b09abcca5a0cbe`: **18 passed / 26 failed / 6 ignored**, 197.66 s. These are existing ignored child-entry tests; no ignore was added. Parents launched children, some of which failed as shown below.
- The full requested matrix **did not pass or complete**. Remaining feature combinations and later targets were not qualified; no workspace-wide matrix claim. No identical compilation was repeated after the concrete test failure.

Actionable failure locations from this exact invocation:

| Location | Actual failure |
| --- | --- |
| `tests/support/native.rs:515,544` | Native User/storage token requests: HTTP 500, `invalid input: local node private signer is missing`. |
| `tests/support/native.rs:275` | `independent locally trusted signer`: `InvalidInput("local node private signer is missing")`. |
| `tests/owner_boundaries/permission_http_tests.rs:109` | Independent IdP runtime construction fails with the same missing private signer; parent reports child panic at line 44. |
| `tests/owner_boundaries/node_join_bootstrap_tests.rs:286` and `node_bootstrap_interruption_tests.rs:254` | Production approving owner runtime fails with the same missing private signer. |
| `tests/owner_boundaries/policy_cleanup_http_tests.rs:50` | HTTP 500 with the same missing signer description. |
| `tests/owner_composition_tests/permissions.rs:310` | `real policy denial` request to HTTPS loopback `/idp/applications`: `reqwest::Error ... source: TimedOut`. Parent fails at line 94. |
| `tests/owner_composition_tests/create.rs:196` | `bounded independent device readiness` in Create and normal-operation children; remote setup child reports `bounded independent idp readiness`. |
| `tests/owner_composition_tests/reset.rs:389` | Join child assertion: `local.load("", "", client).expect("partially deleted client").is_none()` is false. Parent fails at `join.rs:114`. |
| `tests/owner_boundaries/native_api_tests.rs:547` | `built exported SDK operations against actual routers` parent assertion fails. No SDK refactor attempted. |

These are distinct observed failures, not all attributed to one cause. Missing signer evidence is actionable but no isolated baseline or investigation here proves its origin, relation to the other agent’s lock-release race, or pre-existence. The exact requested invocation uses ordinary default test concurrency; it is not an idle serial HTTPS reliability claim. Existing test bounds/assertions remain unchanged.

Complete terminal failure summary:

```text
failures:
    native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes
    native_api_tests::independent_idp_service_loss_denies_response_and_api_without_cached_fallback
    native_api_tests::native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations
    native_api_tests::native_local_token_read_response_denies_learned_revocation_before_transmission
    native_api_tests::native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations
    native_api_tests::native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner
    native_api_tests::qualification_tests::bundled_storage_direct_idp_checks_current_bearer_registration_and_signer_offline
    native_api_tests::qualification_tests::bundled_storage_direct_idp_preserves_real_local_owner_gate
    native_api_tests::qualification_tests::native_catalog_device_restrictions_do_not_revoke_bearer_results
    native_api_tests::qualification_tests::native_catalog_response_denies_learned_application_reassignment_without_rollback
    native_api_tests::qualification_tests::native_socket_device_restriction_preserves_bearer_response_and_next_operation
    native_api_tests::qualification_tests::native_socket_response_binds_namespace_even_when_resource_id_exists_in_new_application
    native_api_tests::qualification_tests::native_socket_response_denies_application_reassignment_and_retains_completed_write
    node_bootstrap_interruption_tests::bootstrap_current_boundaries_preserve_admitted_import_and_native_retry
    node_bootstrap_interruption_tests::desktop_bootstrap_orchestration_completes_both_public_contexts_and_retries_offline
    node_bootstrap_interruption_tests::production_durable_transitions_preserve_imports_and_verified_readiness_on_reopen
    node_join_bootstrap_tests::authorized_node_bootstrap_imports_both_contexts_and_reopens_offline
    owner_composition_tests::create::production_independent_create_and_offline_reopen
    owner_composition_tests::create::production_independent_create_legacy_record_reset
    owner_composition_tests::join::production_independent_join_legacy_record_reset
    owner_composition_tests::operation::production_independent_signer_free_operation_and_delayed_authority_reopen
    owner_composition_tests::permissions::independent_permission_production_management_https
    owner_composition_tests::remote_setup::remote_create_join_reopen_and_device_reset_preserve_shared_installation
    permission_http_tests::normal_identity_and_infrastructure_lifecycle
    permission_http_tests::normal_permission_http_administration
    policy_cleanup_http_tests::real_policy_http_chain_retires_copies_reselects_generation_and_gates_transfer

test result: FAILED. 18 passed; 26 failed; 6 ignored; 0 measured; 0 filtered out; finished in 197.66s

error: test failed, to rerun pass `--test owner_boundaries`
error: process didn't exit successfully: `/home/nathan/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo test --all-targets --manifest-path Cargo.toml --all-features` (exit status: 101)
```

### Exact CRAP outcome: safely stopped, incomplete

Command: `just crap` (recipe: `RUST_MIN_STACK=67108864 cargo llvm-cov --workspace --lcov --output-path /tmp/lcov.info && cargo crap --workspace --lcov /tmp/lcov.info`).

- **435.34 s**, supervisor stop reason: **MemAvailable below 1.5 GiB for three samples**; minimum **1,066,430,464 bytes**. Recipe process exit **143** after SIGTERM. Not a Rust build/test failure and not the 30-minute timeout.
- Coverage was still compiling native workspace dependencies; final lines include `gtk-sys`, `gio`, `idp-server`, `device-server`. No compiler error or new malformed archive error appeared before stop. No coverage completion or CRAP score; `cargo crap` was not reached because it follows successful llvm-cov with `&&`.
- No retry under depleted memory. Final resource snapshot after termination: about 3.0 GiB MemAvailable, full swap, 250 GiB disk available. Disk was never close to the 20 GiB guard. Process inspection found no surviving Cargo/rustc/just from this validation worktree; it did find the other agent’s qualification-worktree test/compiler and editor main-worktree check, all left untouched.

### Logs, reproducibility and cleanup

Full logs remain disposable local evidence, not committed generated artifacts:

- matrix: `/tmp/of-setup-validation-matrix.log` (49845 bytes), SHA-256 `9f37b1dfb7fefb63a081f1c414a2e2c2c1a5d642f64a4329e6e30a57c0493894`; supervisor metadata `/tmp/of-setup-validation-matrix-result.json`.
- CRAP: `/tmp/of-setup-validation-crap.log` (6375 bytes), SHA-256 `721d57458dc6db4a3347938182e77d88ef0d5f71f3d6ee1762c4862f3444bca7`; supervisor metadata `/tmp/of-setup-validation-crap-result.json`.

The matrix log begins `Locking 1 package ... Adding ofdb-platform ...`; the isolated lock refreshed for current mutable sibling inputs. Its diff also changes sibling feature dependency edges (including wasm-related edges). This demonstrates source/dependency drift since the committed lock and limits reproducibility; it is not an excuse to edit main Cargo.lock or sibling sources. After inspecting the isolated lock diff, restore only this run’s generated isolated `Cargo.lock` to HEAD before committing notes. No lock change is delivery.

Sibling HEADs observed after execution (not pinned snapshots of concurrent dirty work):

- `ofdb`: `e3242170c94d5fc4bcc121db812310c8106c7ab6`.
- `ofnet`: `dbea925a0a3f99053fffc02618baccc2a1cbea9f`.
- `offs`: `df4c696a910296635473ec296d9235acbaa60d9b`.

Main lock hash at entry, after matrix and final inspection remains `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; its pre-existing modification was never reverted. No source, sibling work, OS trust/profile, dependency feature, profile or lint failure was changed. Only this evidence is committed. Ticket07/spec remain open: matrix has actual test failures and CRAP remains resource-blocked. Next action is investigate the concrete failing seams and arrange more available memory before another coverage attempt, not repeat the unchanged expensive graph.


## Failure diagnosis follow-up: source comparison and execution-only isolation

### Integration, scope and provenance

Merged integration **first** as requested: fast-forward `d249f2c97` → `4ae84f2fc4bbbaf007222859ee48b0e29b641ddf`. This incorporates the other agent’s `0739d5855` router-teardown/ownership fix and reliability evidence. Only this follow-up evidence is edited. Used ponytail for the investigation; no code fix/test modification, so no TDD implementation cycle. No full matrix, coverage or compilation was run in this phase.

All executions below reuse the exact matrix executable `/home/nathan/code/rust/aicacia/of/target/debug/deps/owner_boundaries-d4b09abcca5a0cbe`, cwd this validation worktree, `RUST_MIN_STACK=67108864`, `CARGO_BUILD_JOBS=2`, main `CARGO_TARGET_DIR`. This is the **earlier compiled matrix code**, not a rebuild of the newly merged teardown fix. The binary is appropriate for diagnosing those recorded failures, but its passes cannot qualify final integrated source. New integration changes `hosted_setup.rs`; it does not change the signer fixtures or keyring implementation discussed below.

Pre-spec source comparison uses **`191962716f3edd7fd0c0ffbd8e8ec6c008511393`**, parent of initial spec commit `1a9f3f847`. Retrieved with `git show`, not checkout/build. The root integration-test paths did not exist there: compare their original locations under `crates/unified-server/src/`, not an empty/new-file diff. Tests were subsequently extracted in `78d95df77` and `32b8131aa`. This is **source baseline evidence, not an executed baseline suite**. Mutable sibling dependencies remain a separate reproducibility limit.

### Partition of all 26 matrix failures

Parsing all 26 top-level `---- <name> stdout ----` blocks gives:

| Count | Actual failure family | Current diagnosis |
| ---: | --- | --- |
| 19 | Missing local private signer: 12 native API cases, two permission HTTP cases, policy cleanup, Node Join bootstrap and three bootstrap interruption/orchestration cases | Source-established process-global mock-provider isolation defect; two formerly failing representatives pass alone. Exact concurrent store-switch history for the original matrix was not recorded. |
| 1 | `generated_rust_and_built_javascript_use_actual_owner_routes` | Concrete missing built-JS prerequisite, not an SDK behavior failure. |
| 4 | Two Create parents, normal-operation parent, remote-setup parent: bounded independent owner readiness | Ordinary Create parent passes alone. Concurrency/resource sensitivity supported, but no exact cause or baseline equivalence established for all four. |
| 1 | Independent production Management HTTPS permission parent | Child request timeout in matrix, passes in a fresh isolated serial child. No persistent failure established. |
| 1 | Independent Join legacy-record reset parent | Reproduces identically in a fresh isolated child; fixture expects deletion from a record that the unchanged parser explicitly refuses. |

Do **not** describe all 26 as unrelated, all as a signer defect, or all as a setup regression. Full requested matrix remains failed. No ignored assertion, skipped feature, enlarged timeout, weakened test, trust bypass, or environment “fix” is used to change that result.

### Signer path: precise invalid concurrency contract

`tests/support/native.rs:221–248` creates a unique temporary root and issuer, then a module-static shared mock STORE and `PrivateKeyKeyringRepo::new_with_store`. Provisioning creates a fresh private signer and approved public record; `ensure_initial_node` verifies it (`:273–275`). Later reopen/runtime/token calls (`:297–339`, `:515`, `:544`) must load the same private bytes.

But `PrivateKeyKeyringRepo` **does not retain the supplied store**. It stores only `service_name`. `new_with_store` installs its argument with `keyring_core::set_default_store`; every load/store/delete constructs another `Entry` from the **current process-global default**. The initialization mutex serializes initialization calls only, not a fixture’s complete provisioning/token lifetime.

At the same time, `tests/support/identity.rs:56–61` and `:87–92` invoke `new_with_store(issuer, test_keyring_store())`. `test_keyring_store` at `:1390` creates a **new empty mock backend every call**, not a shared one. Other owner-boundary tests call these seed helpers while native fixtures are live. A legal scheduler interleaving is: fixture A creates its signer in backend A → fixture B installs backend B → fixture A reopens/loads from B and finds no entry. Unique issuers/directories prevent name collisions but do not pin backend ownership. Even replacing it with the native shared backend can strand entries created in B. `keyring-core 1.0.0` explicitly documents global-store replacement as a startup operation; multiple stores should construct entries directly instead.

`KeyService::load_node_signer` in `crates/idp-service/src/oauth2/node_signer.rs:54–68` first locates the matching approved public signer, then returns exactly `local node private signer is missing` when private backend load returns None. Thus this is neither a missing approval message nor a requirement to install a real provider. It is consistent with the original failures occurring at different points: initial node verification (`native.rs:275`) or later User/exchange token signing (`:515/:544`).

Baseline checks:

- `git show 191962716:crates/unified-server/src/native_api_tests.rs`: its static STORE → private provisioner → initial-node block is **identical** to the current extracted fixture’s corresponding block.
- `git show 191962716:crates/unified-server/src/permission_http_tests.rs`: seed calls also install `test_keyring_store()`, whose fresh-store function is **identical** to the current function extracted into `tests/support/identity.rs`.
- Entire current `crates/idp-service/src/repo/private_key_keyring_repo.rs`, `crates/idp-service/src/oauth2/node_signer.rs` and `crates/idp-server/src/node_provisioning.rs` are **byte-identical** to pre-spec versions.

The unsafe fixture concurrency contract demonstrably predates the setup spec. Test regrouping/extraction can alter scheduling/exposure, so this does not prove the exact pre-spec suite would fail at the same frequency. It establishes an existing fixture/provider ownership defect, not a new signer-provisioning algorithm introduced by this setup work. No production or fixture code is changed here; repairing it would require separately coordinated test/provider scope, not silently serializing the qualification gate or introducing OS keychain prerequisites.

Dynamic evidence is narrower than a deterministic race reproduction: two individual tests pass, but **both small concurrent pair experiments also passed**. Therefore the global-switch mechanism is established from source and matches the 19 symptoms; no per-event trace proves that it caused every original failure. Do not claim the pair runs reproduced it. No further permutation/retry loop was run.

### SDK prerequisite: exact hidden cause from the full log

Before the SDK parent assertion, Node reported:

```text
Error: Cannot find module '/home/nathan/code/rust/aicacia/of-setup-validation/packages/idp-client/esm/index.js'
  at .../scripts/qualify-built-openapi.mjs:6:72
code: 'MODULE_NOT_FOUND'
```

That file remains absent. The script resolves the package’s built export before making its first SDK API call. `tests/owner_boundaries/native_api_tests.rs:530–547` launches it and checks exit success. Thus this particular SDK failure is a missing generated/built package prerequisite. Root `package.json` already provides `pnpm packages:build`; earlier qualification receipts explicitly built packages first, whereas this fresh validation worktree did not. No SDK source refactor or dependency feature change is justified. A later authorized prerequisite step would be `pnpm --dir /home/nathan/code/rust/aicacia/of-setup-validation packages:build` after ensuring frozen workspace installation is available; it was **not run** now and no missing package export assertion was bypassed.

### Isolated legacy-reset failure: exact source-level cause

Executed the **parent** `owner_composition_tests::join::production_independent_join_legacy_record_reset`, not the ignored child manually. The parent retains its actual per-run protected root (0700), isolated CA file/directory, injected no-op trust/removal commands, mock provider, loopback HTTPS endpoints and 180-second bound. It spawned its own ignored child using the original environment. No real trust/profile mutation.

In the isolated run, public Join, ordinary Storage use and source-offline reopen succeeded; then the child failed at **the same `tests/owner_composition_tests/reset.rs:389`** after about 100.5 s. This failure is **not** explained by cross-test process-global store replacement.

Root cause from exact code:

1. `qualify_completed` checks `OF_INDEPENDENT_LEGACY_RESET=1` and calls `retain_legacy_cleanup_references`, which rewrites worker `version` to **1**, removes `create` and adds historical `role`/`permissions` fields for Device.
2. The production `crates/device-server/src/independent_reset.rs` `References` uses `#[serde(deny_unknown_fields)]`, does not contain historical `role`/`permissions`, and requires **version 2** before calling `delete_setup_secrets`.
3. Therefore reset errors **before** any protected client deletion. The test accepts any error at lines 375/378/385, incorrectly interpreting it as the intended injected provider/endpoint deletion error; line 389 then requires that the Client was already deleted. The error precondition was never reached. The retained Client is consistent with refusal/preservation, not proof reset deleted the wrong slot or a changed endpoint deletion order.

The entire `tests/owner_composition_tests/reset.rs` is byte-identical to its pre-spec `crates/unified-server/src/owner_composition_tests/reset.rs`. The entire production Device reset implementation is also byte-identical to pre-spec. The only pre-spec→current `api/src/independent_reset.rs` diff is removal of an `io::Error::other` wrapper around admission open, not historical-record parsing or secret deletion. Thus the version-1 fixture versus version-2 parser mismatch is pre-existing source evidence, reproduced on the matrix binary; not a demonstrated new setup regression. Do not add migration/legacy compatibility or weaken the assertion here. A separately owned decision should reconcile the legacy-reset fixture’s intended contract with the deliberate retained-record refusal.

### Small execution-only runs

No builds, no full owner suite, no full matrix/coverage. All serial commands have the common prefix:

```text
/home/nathan/code/rust/aicacia/of/target/debug/deps/owner_boundaries-d4b09abcca5a0cbe <exact-test-name> --exact --nocapture --test-threads=1
```

| Exact test name | Actual outcome |
| --- | --- |
| `native_api_tests::native_owner_cutover_captures_initial_secrets_and_reuses_reopened_owner` | **1 passed**, 8.81 s test time (8.84 s supervised); previously missing signer at native.rs:544. |
| `permission_http_tests::normal_permission_http_administration` | **1 passed**, 57.14 s test time; previously missing signer at permission_http_tests.rs:109. |
| `owner_composition_tests::join::production_independent_join_legacy_record_reset` | **Failed**, exit 101, 100.76 s supervised; isolated child reset.rs:389, parent join.rs:114. |
| `owner_composition_tests::create::production_independent_create_and_offline_reopen` | **Parent and isolated child passed**, exit 0, 123.07 s supervised. The original matrix failed its bounded readiness wait. |
| `owner_composition_tests::permissions::independent_permission_production_management_https` | **Parent and isolated child passed**, exit 0, 23.42 s supervised. Original matrix timed out on authorized HTTPS `/idp/applications`. |

Two bounded concurrency probes, each in its own new binary process and **not** a full suite:

- Native representative + permission-administration: list names with binary `--list`, pass `--skip <full-name>` for all 48 other names, `--nocapture --test-threads=2`: **2 passed**, 58.71 s supervised.
- `permission_http_tests::normal_ --nocapture --test-threads=2`: **2 passed**, 63.88 s supervised.

These probes do not waive the default-concurrency matrix. Child tests were invoked only through their parents. Parent isolation remains intact; no injected HTTPS policy, test deadline or assertion was modified. Original four readiness failures and request timeout are timing-sensitive observations, not proven environment-only bugs: the unchanged helper `create.rs:177–201` uses one 60-second deadline across sequential IdP/Device/Storage waits, and concurrent full-suite load can consume that budget. One standalone Create pass supports that explanation but does not prove it for the legacy-Create, operation and remote-setup parents, which were not rerun here. No source-only comparison can assign their runtime root cause conclusively.

### Can jobs=1 safely finish CRAP now?

**Not established; do not promise completion or start another heavy run in this phase.** Current snapshots show 4.0–4.2 GiB MemAvailable, roughly 0.9–1.2 GiB immediately free, 2 GiB swap still full, about 245 GiB disk free. Immediate memory PSI is zero at inspection, so this is not evidence of active thrashing/OOM. But the prior jobs=2 run fell to 1.0 GiB available and triggered the safety guard. No measured per-compiler peak or final coverage-link peak was captured, so halving build jobs is not a justified guarantee of sufficient headroom or completion within 30 minutes.

`CARGO_BUILD_JOBS=1` limits Cargo build parallelism only. It **does not serialize libtest/child fixture execution** and cannot repair the global-provider isolation contract, missing built JS prerequisite, legacy-reset mismatch, or original test failures. `just crap` also proceeds through all workspace tests, not only these passing representatives. More memory would not by itself make the gate green.

A later coordinated attempt could use jobs=1 with the same process-group safety guard and main cache, only after prerequisite/fixture ownership decisions and sufficient stable memory headroom. Do not change dev/test optimization, coverage semantics or assertions to reduce memory. No coverage was run alongside focused tests or at all in this phase, and no other process was stopped.

### Durable evidence and status

Disposable execution logs (hashes below) preserve the original complete outputs. No source fix is claimed; next scoped work is a coordinated fixture-isolation/legacy-contract decision, prerequisite package build when authorized, and separate investigation of remaining readiness timing. Ticket/spec and existing receipts remain unchanged/open. Final source is merged but the reused binary predates its lifecycle fix; this follow-up is diagnosis, not final-code qualification.

- `/tmp/of-setup-validation-native-serial.log`: 215 bytes, SHA-256 `fb49ae50f47024f52ce632c6a3e7930f25860b735c1253a878bdca5df02edffe`.
- `/tmp/of-setup-validation-permission-serial.log`: 4646 bytes, SHA-256 `c0d7fdbb2369942e2feaee69a71b74e74e91269c37d9efb6d749f9c710c485a3`.
- `/tmp/of-setup-validation-signer-pair.log`: 4749 bytes, SHA-256 `7d16428644e0d0f80b323d54c89e74ccc4853e35e7c59d35f9659b9b2ecdec71`.
- `/tmp/of-setup-validation-permission-pair.log`: 7738 bytes, SHA-256 `c735039074f69a20135a0fe4745c1d9f16954c8713bc20ff908b88115eedde2a`.
- `/tmp/of-setup-validation-join-reset-serial.log`: 5622 bytes, SHA-256 `c98ddb3ceb2e21c8cca5230e137c329d19dabf00484c422868cb460e3bf63834`.
- `/tmp/of-setup-validation-create-serial.log`: 6951 bytes, SHA-256 `cf1a6e7ae8749b283829284ec0fa2d5e55fad7b51ea5b8809d7fa62c9735b4a8`.
- `/tmp/of-setup-validation-https-permission-serial.log`: 488 bytes, SHA-256 `e53134a34203ade5b3311f4c52b1229b4436632befc3cb28369bc93a6c141fe4`.

Main Cargo.lock remains SHA-256 `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; isolated worktree lock was not modified in this execution-only phase. Only this evidence file is delivery.
