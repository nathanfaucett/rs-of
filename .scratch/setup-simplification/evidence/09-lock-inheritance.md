# Setup-secret lock inheritance: deterministic real-operation regression

2026-10-27 UTC. Root-only work on `implement/setup-lock-inheritance`, prepared from integration `4537e248bdcb0fb0b867410db8c30acdf2668a8b` (HEAD and merge-base verified equal before editing). Scope: the setup-operation destructor, one dedicated integration test, and this evidence. Existing tickets/map/spec, library unsafe-code forbid, sibling sources, dependencies, and external SDK changes are not implementation scope.

## Result and attribution boundary

**A real-operation behavioral red establishes descriptor inheritance as a setup-secret lock lifetime defect.** The exact same integration test passes after explicit owner unlock before closing the guard's File. This does **not** identify the holder in `/tmp/setup11-supported-matrix.log`, reproduce that full workload, explain its other failures, or qualify the full setup workflow. The historical initial-driver WouldBlock remains unattributed.

Linux `File::try_lock` uses `flock`. A forked child's descriptor refers to the same open file description, so parent close alone need not release the lock. `O_CLOEXEC` closes the child's descriptor at exec, not at fork. A concurrently spawning test can therefore temporarily retain a completed operation's lock without opening the same fixture root or invoking another setup method. A parent post-File-drop diagnostic event alone does not establish kernel unlock.

## Public seam and deterministic ordering

`crates/idp-server/tests/node_provisioning_inheritance.rs` uses public `OwnerProvisioner::open_with_private_keys`, inventory preparation, `prepare_node_signer`, and `verify_prepared_node_signer`. Its minimal in-memory `PrivateKeyRepo` follows the existing unit-test store pattern; it pauses the actual first private store, not a manually acquired lock.

1. Park real signer preparation inside the store, with its operation guard alive.
2. Spawn the test executable with `--list`; a pre-exec byte handshake proves the child has forked while that guard is held.
3. Verify that the still-running operation denies a competing verification with `WouldBlock`.
4. Resume and join preparation. Verify the same signer **before releasing the child to exec**.
5. Release/acknowledge the child gate, reap the child, and verify again after exec. Assert identical public signer and exactly one private store.

All assertions follow operation/thread/child cleanup. Read/write timeouts are installed before fork (10s), and store entry/release is bounded (30s). The child has a bounded post-exec wait (10s), followed by kill/wait on error. The acknowledgment prevents child timeout from masquerading as a passing early-unlock check. Normal pre-exec blocking lasts only for the two real verification calls and preparation completion; no sleep schedules the race.

The only unsafe block is in the Linux integration test, outside the unchanged `#![forbid(unsafe_code)]` library. The callback uses fixed stack buffers, byte comparisons, and single std UnixStream reads/writes only: installed Rust std routes these to async-signal-safe `recv`/`send`; ErrorKind conversions do not allocate. No locks, allocation, logging, assertions, environment reads, or test/store callbacks run in pre-exec. Socket timeouts and Command configuration are prepared beforehand. No libc or other dependency was added.

## Red, then minimal production fix, then green

Environment for every supervised Cargo command:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target
CARGO_BUILD_JOBS=1
RUST_MIN_STACK=67108864
```

The identical red/green command, from the prepared root worktree:

```sh
cargo test -p idp-server --test node_provisioning_inheritance --all-features -- \
  --exact completed_signer_operation_releases_lock_before_inheriting_child_exec --nocapture
```

- **Behavioral red:** exit101, 0 passed / 1 failed, 2.35s test body. Failure at test line195: `completed operation must release exclusion before child exec: Custom { kind: WouldBlock, error: "another setup secret operation owns this directory; retry after it finishes" }`. All preceding assertions passed, including live-operation exclusion, child acknowledgment/reap, successful post-exec verification, and exactly one store. Production guard was still unchanged.
- **Fix applied only after that red:** `SetupOperation::drop` takes its owned File, explicitly calls `File::unlock`, then drops it before the existing release diagnostic. Acquisition stays nonblocking and fail-closed. No retry, deletion, waiting, new lock, or network-spanning serialization. Unlock errors cannot be returned by Drop; File close remains the fallback, and a failed unlock does not grant acquisition to a competitor.
- **Green:** exit0, 1 passed / 0 failed, 2.27s test body. Test source SHA256 is identical in red and green.

An earlier unit-test placement attempt did not run: the library's unsafe-code forbid rejected `pre_exec` during compilation. After explicit approval, only that agent-added uncompiled test block was removed; `node_provisioning_tests.rs` is byte-identical to the starting commit. That compile failure is **not** the behavioral red.

## Fixed inputs and receipt hashes

Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, `cargo 1.99.1 (5f94df478 2026-08-27)`; Linux, glibc2.39. Sibling inputs, with no edits:

- ofdb: `a8c23a54540e591aa024396a5674dc016b6cce3a`
- ofnet: `dbea925a0a3f99053fffc02618baccc2a1cbea9f`
- offs: `df4c696a910296635473ec296d9235acbaa60d9b`

SHA256:

| Input / output                  | SHA256                                                             |
| ------------------------------- | ------------------------------------------------------------------ |
| Cargo.lock                      | `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f` |
| Production source, red          | `0be4ec1525b3f1efd81e0d41751606dec88db7a5af3b8d2570625b35549db5b1` |
| Production source, green        | `4a6b207e8593f13bc1d90061182eb0d83c1e7b0ad581d4ebf77ba8c6dc16123d` |
| Integration test, red and green | `6547c7e980cf26829df320457f38016f9b33505e505d6aa3728bd74d093b8a19` |
| Unchanged unit tests            | `c6a5659e380440752c79cd0609326ebe9f305f62e8f0472ab75bbc0109eb345d` |
| Red executable                  | `a9488e6e37df9b9f6f19fb2abf1c6fc9d9f4a199e38af8858bd406bb46ea8bd1` |
| Green executable                | `a68e8aef4675988a81fb57a394dde46619b419904b0f2c86deabb139b518a8d1` |
| Behavioral red log              | `ced815455d5cc41eef79d0876751746b5e6401343801f77d5dc68926dacd1f03` |
| Green log                       | `dd1a3fdffd9c1ef5c8f8ecd185732ba08b31b3ca071436407d78eea2c64a2fdc` |

Private local receipts: `/tmp/setup-lock-inheritance-receipts/`. `run.py` launches each Cargo command in a new owned process group and records command/environment, source hashes, heads, wall time, exit status, log digest, and one-second `/proc` samples of actual summed group RSS, available RAM, PSI, and available filesystem space. Each label has `.log`, `.json`, and `.resources.jsonl`; `red` is the initial compile-blocked placement, `inheritance-red` is the actual red, and `inheritance-green` is green.

## Final focused validation and resource boundaries

PID3780043 was absent before the first Cargo command. No external process was terminated. All Cargo jobs use the environment above; no concurrent owned build, profile changes, cache deletion, or swap-only veto. Supervisor stops only its owned group for RAM below2GiB, disk below30GiB, sustained20s PSI some>=20/full>=10, or the7200s command ceiling.

| Receipt label         | Wall seconds incl. compilation | Min available KiB | Peak owned RSS bytes | Result                                                                        |
| --------------------- | -----------------------------: | ----------------: | -------------------: | ----------------------------------------------------------------------------- |
| red (placement only)  |                        689.950 |         2,405,816 |        1,193,152,512 | Compile rejected unsafe unit test; no test run                                |
| inheritance-red       |                        298.573 |         2,194,104 |        2,635,698,176 | Behavioral red, exit101                                                       |
| inheritance-green     |                        168.362 |         2,372,408 |        2,885,083,136 | 1/1 pass                                                                      |
| contention-controls   |                        134.263 |         2,094,012 |        1,833,058,304 | Actual RAM floor; owned group terminated before tests                         |
| scoped-clippy         |                        190.107 |         2,306,668 |          900,927,488 | Dependency-inclusive command failed on 93 existing generated IdP-client lints |
| scoped-clippy-no-deps |                         54.274 |         3,174,368 |          701,796,352 | Strict selected library + integration-test Clippy pass                        |

The all-features control build was resource-stopped, not a test failure or pass. The smaller no-default-features build completed under the same resource thresholds: **3 passed / 0 failed / 64 filtered**, 4.14s test body, 494.330s including compilation. Minimum available RAM was2,573,428KiB, peak owned RSS3,252,064,256bytes, and minimum disk422,744,977,408bytes. It executes the same guard code without default CLI/Swagger features; this is not an all-features control pass. Log SHA256: `82c8e687a3bddf359fc2108f8b288d35678c2c9845419968d5ef6dccf570468e`.

```sh
cargo test -p idp-server --lib --no-default-features -- --exact \
  node_provisioning_tests::inventory_concurrent_public_operations_preserve_capture \
  node_provisioning_tests::inventory_fresh_authorization_requires_exclusive_operation \
  node_provisioning_tests::inventory_cleanup_retry_requires_durable_resetting_fence --nocapture
```

The concurrent-public-operations control preserves WouldBlock for cleanup, initial User/Node, Client provisioning, signer preparation/verification, and a separately opened descriptor while an actual private write is paused. It also verifies captured inventory is unchanged until release and exact cleanup/reopen succeeds afterward. The other controls preserve fresh-authorization exclusion and the durable resetting fence. All original assertions and bounds are unchanged.

Strict scoped Clippy passed:

```sh
cargo clippy -p idp-server --lib --test node_provisioning_inheritance \
  --all-features --no-deps -- -D warnings
```

The first dependency-inclusive invocation (without `--no-deps`) failed on93 unchanged generated `of-idp-client` lints, including empty docs and derivable defaults. No generated dependency source was edited and no selected-target lint was suppressed. Logs SHA256: `scoped-clippy` = `3c1cebd574c60a72f6b59f6168b1af833e12f620fb4a3033c42f7aefda75e29e`; `scoped-clippy-no-deps` = `fe2735324c66b737a2e9d496ccec95704387e0537a24c34eeb7a743040be7272`.

`rustfmt --edition 2024 --check` for both changed Rust files and `git diff --check` passed. Across all monitored attempts, max sampled memory PSI some/full avg10 was8.47/7.46; minimum disk was422,744,977,408bytes. There was exactly one resource stop (the recorded RAM floor), no sustained-PSI/disk/7200s ceiling stop, and no surviving processes in any owned Cargo process group at final audit.

Before completion, `GIT_EDITOR=true git merge --no-edit integration/setup-simplification-followup` reported `Already up to date`; integration remained `4537e248bdcb0fb0b867410db8c30acdf2668a8b`. The external root's seven SDK generated-dependency diffs were byte-preserved (before/after diff SHA256 `f668c6aa9ab74b1b5122bb6afe17b31b0852aa67cb68f4b6ea8d8d278bdaf07f`). `lib.rs`, existing unit tests, and Cargo.lock match the starting commit. `final-audit.json` records those checks and empty remaining-process lists. No sibling sources, existing tickets/map/spec, main branch, push, provider, or trust-store changes.

Full matrix, coverage/CRAP, and a historical failing-workload holder capture are not performed or claimed by this patch.
