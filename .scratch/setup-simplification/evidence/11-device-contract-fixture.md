# Ticket 11: narrow Device contract fixture repair

## Ownership and inputs (2026-10-27)

- Worktree: `.worktrees/setup-device-contract`; branch: `repair/setup-device-contract`.
- Created from integration `d35a961fb89df254f20dbf958be37cc6cf5b9784`. Before final validation, hooks-disabled fast-forward merged current integration `a7b54ef091cef0c471467d10fbc0dc51b767b6d5` (ticket11's latest qualification receipts).
- Read root AGENTS, shared glossary, setup spec/testing decisions, ticket11, and latest evidence on `.worktrees/setup-11-final` (`b53842afc`); loaded TDD and ponytail. The user-confirmed seam is the existing Device enrollment/conflict integration test, not a new backend contract.
- Observed sibling inputs: ofdb `a8c23a54540e591aa024396a5674dc016b6cce3a`, ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, offs `df4c696a910296635473ec296d9235acbaa60d9b`. Existing parent sibling symlinks reused unchanged.
- Cargo.lock SHA-256 remained `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`.
- Complexity/risk/priority: low-complexity test setup repair; preserve security-negative assertions; narrowly unblock ticket11's concrete coverage test failure, without claiming broader qualification.
- Exclusive Rust slot explicitly granted. No production/sibling changes, parser changes, compatibility code, dependencies, profiles, generated artifacts, shared ticket11/evidence, map, spec, or ticket07 edits. Only `crates/device-service/tests/device_contract.rs` and this new evidence file are delivery scope.

## Actual red and root cause

Unmodified test:

```sh
cargo test -p device-service --features replica,utoipa --test device_contract approved_enrollment_denies_unresolved_owner_conflicts_even_with_exact_visible_binding -- --exact
```

Actual red: **exit101; 0 passed / 1 failed / 5 filtered**, at original `device_contract.rs:225:14`:

```text
replicated Management state: QueryError { kind: Storage,
  message: "Error: Incomplete catalog: data table devices has no definition" }
```

The first compilation stage was stopped by the existing monitor at 550.688s; it had not reached tests. Retained its productive cache and continued the exact unchanged command, which completed compilation and reproduced the reported failure in 71.328s. This continuation is not a retry-to-green: no fixture edit occurred until the actual red was observed. Initial `python` invocation failed before launching Cargo because only `python3` is installed; subsequent supervision consistently used `python3`.

The fixture applied a full state export one unit at a time into an empty database. The current import contract validates catalog definitions before accepting dependent Device rows; individual units do not constitute a full catalog import. Backend inspection confirmed `apply_sync_state_for` wraps a one-unit batch, while the batch importer validates catalog state before applying data. `Database` exposes a public full sync session but no public batch-apply method. This is stale fixture setup, not evidence for weakening the backend.

## Minimal correction

Only the initial copy into empty `right` now uses the existing public `Database::synchronize` session, initiator/responder over test-local Tokio channels. That session supplies the full catalog/data import through the current contract. The small transport is local to the existing test, uses installed dependencies, and creates no sockets, new shared harness, or production abstraction. `right` remains genuinely empty before synchronization: no independent schema creation, synthetic catalog identities, skipped catalog rows, or manually ordered partial imports.

Concurrent address mutations, the later import into already initialized `left`, the nonempty `row_conflicts` assertion, and the exact approved-enrollment rejection assertion are **byte-for-byte unchanged** from `d35a961fb`. Verified by comparing the entire suffix starting with the concurrent-update loop. No assertion was suppressed, broadened, or weakened.

## Commands and results

All Cargo commands below ran from this worktree with:

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target python3 /tmp/setup09-default-monitor.py <receipt-name> <cargo-command>
```

The existing monitor supplies `CARGO_BUILD_JOBS=1`, `RUST_MIN_STACK=67108864` (64 MiB), an owned process group, one-second RAM/PSI samples, RAM<2GiB and sustained-PSI guards, and a 550s stage limit. Every terminal call was bounded to 570000ms. Full swap alone was not a veto. No cache deletion, unrelated process termination, test-thread override, feature suppression, or Node build.

| Receipt name | Cargo command / outcome | Supervised seconds |
| --- | --- | ---: |
| `device-contract-red` | Exact focused command above; compilation-only stage-limit stop, exit143; no test result | 550.688 |
| `device-contract-red-cont1` | Same unchanged focused command; actual red, exit101 | 71.328 |
| `device-contract-green` | Same focused command after fixture correction; **1 passed**, 5 filtered, exit0 | 18.493 |
| `device-contract-suite` | `cargo test -p device-service --features replica,utoipa --test device_contract`; **6 passed**, none failed/ignored/filtered, exit0 | 2.293 |
| `device-contract-clippy` | `cargo clippy -p device-service --features replica,utoipa --test device_contract -- -D warnings`; exit101 on **93 generated of-idp-client errors**, including `empty_docs` and `derivable_impls`; unrelated generated code left untouched | 43.283 |
| `device-contract-clippy-nodeps` | `cargo clippy -p device-service --features replica,utoipa --test device_contract --no-deps -- -D warnings`; **passed**, exit0; scoped lint success, not dependency lint success | 15.270 |
| `device-contract-suite-integrated` | Full six-test command above, after merging `a7b54ef09`; **6 passed**, none failed/ignored/filtered, exit0 | 2.254 |

`rustfmt --edition 2024 --check crates/device-service/tests/device_contract.rs` and `git diff --check` passed before and after the integration merge. Final Rust test file SHA-256: `4fba764a2dbb1181ef3c9a4f5b48169a0a8ea557918d0761dd1f5323acdb91f4`.

Resource receipts: minimum sampled MemAvailable across these runs **8,499,032 KiB**; maximum sampled owned-group RSS **2,868,512 KiB**; maximum memory PSI some/full avg10 **1.08/1.08**. No RAM/PSI guard fired. Only the first productive compile hit its stage-time ceiling; every subsequent run ended normally (including actual Cargo failures). Initial disk available approximately300GiB. Samples are not guarantees of unsampled peaks.

## Raw evidence

Each receipt has `/tmp/setup09-qual-<receipt-name>.log`, `.json`, and `-samples.jsonl`. Log SHA-256 values:

| Receipt name | Log SHA-256 |
| --- | --- |
| `device-contract-red` | `18e6c29ba583362fe637ef8557d922a7dff841c0b85f354cc12c7a82288d03ea` |
| `device-contract-red-cont1` | `6f03ef08bbfd8ae577459613f1900f0e602f76081f1ff188166396da8ee0f924` |
| `device-contract-green` | `115ba3da3938dafa8a6b36764e47295da3d4577ce540fcf7ad54cea5e3334f94` |
| `device-contract-suite` | `619020df90df261693ff30189086b26ea050a7d719120297caa8dd7a27687436` |
| `device-contract-clippy` | `7e83eb9013775c0c702d04ec5db5ce0f3354cfa94bf711f781de092ff3f7cc8f` |
| `device-contract-clippy-nodeps` | `ae036e724c00dc53db59a17151190e9e8e21ca0ad6ecc2d52b49bac9e372c50e` |
| `device-contract-suite-integrated` | `64391acaa138e5668d10688d8685d76dbf13812bbbd686d7f21732501e8bd01a` |

## Qualification limit / handoff

This repairs and validates only the reported Device contract fixture. No full root/workspace feature matrix, instrumented workspace rerun, `just crap`, LCOV report, CRAP score, GUI, platform, or Node qualification is claimed. Those broad gates remain ticket11 ownership; its latest matrix/coverage failures are not retrospectively waived. Strict dependency Clippy remains blocked by the generated client errors, separately from the passing owned-target lint check. No production bug was found or changed.
