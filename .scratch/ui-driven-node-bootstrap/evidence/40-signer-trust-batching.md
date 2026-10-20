# Receipt 40 — Operation-local signer-trust conflict batching

## Outcome and provenance

Implemented the source-proven signer/member conflict fanout reduction. Focused tests pass. The unchanged production independent source Join and historical-record Join/reset fixtures both pass in **two consecutive complete paired runs**, including their first authenticated Storage catalog GET and source-offline reopen. This is repeatability evidence for these fixtures, not a universal latency guarantee or a broader workspace qualification.

Base: `9c4de8db1` on `integration/ui-driven-node-bootstrap`, with the existing dirty lifecycle work and receipt-39 Management observation/predicates present. Entry status actually contained 16 modified Rust files plus the existing deletion of `evidence/review-transfer-lifecycle.md`; none was reverted, staged, or edited by this change. The Management observation remains uncommitted parent work. No edits outside this repository.

Only own Rust files:

- `crates/idp-service/src/replica/replica_signer_repo.rs`
- `crates/idp-model/src/replica.rs` (small pair-preserving batch interface and its real replicated-conflict test)

Final Rust SHA-256:

- model: `c9ddea46315c651558d40be72a02bc8fd86a30e40b2602c48d58fbf8a7ddaca7`
- service: `dfb137d389d9303a599232971ef4ccf5d757d9ff9cdc12c3f7e349243cf5169c`

## Root cause and change

Before this change, `trusted_signers()` selected members/signers, validated candidate bindings, then called `allows_authorization()` with alternating signer/member rows **inside each candidate-pair iteration**. Model `clear()` groups only adjacent same-table rows. Thus N conflict-clear candidates incurred 2N `row_conflicts_batch` calls per enumeration (conflict short-circuiting could reduce that number). `DbKeyRepo::trusted_node_signers()` forwards here; public JWK lookup, JWKS, and local signer loading consume that facade. The task identifies six enumerations in a Storage authorization pass; no runtime enumeration counter was added or claimed.

Now the repository retains the same validated candidate pairs and their actual signer/member **row IDs**, calls `allows_signer_trust_batch()` once, filters each pair by its own boolean, then runs the unchanged global ambiguity check over accepted pairs. The model interface performs exactly two table-specific `row_conflicts_batch` calls for nonempty candidates, zero for empty candidates. Input order and duplicate pair decisions are preserved. No all-or-nothing conflict reduction.

Canonical issuer, approval/revocation/expiry, public JWK/algorithm/curve validation, and principal-root verification are not relaxed. Device tables are not consulted. No new dependencies, inter-request trust cache, skipped current-state check, changed retry, warming request, deadline change, or changed final Management recheck. Evidence lives only in the current operation. This does not add a cross-table transactional snapshot guarantee that the previous implementation lacked.

Complexity/risk/priority: small local batching change, security-sensitive pair association and ambiguity are the critical risks; high priority because the repeated catalog bound was the remaining reported gate. The existing API suffices below the small model seam; no generic batching framework was introduced.

## TDD and focused behavior

Loaded `tdd` and `ponytail`. User-specified seams: public repository trust enumeration and the optional model batch interface.

1. Initial attempt to construct real replicated conflicts in service tests failed compilation with four E0599 errors: its dependencies do not enable sync export/apply. That attempt was removed, not hidden behind feature-dependent tests or a new dependency.
2. Placed real-conflict coverage at the model seam, which already has sync dev-dependencies. First build hit the 200000ms bound before compiling the test. Identical bounded retry was RED with four E0425 errors for the absent `allows_signer_trust_batch` API.
3. Implemented the interface: GREEN, 1 test / 14.79s; final repeat 1 / 14.75s. Two independent pairs plus a repeated pair are checked against conflicts on **each** of the signer/member tables: `[true,true,true]` becomes `[false,true,false]`, then returns to clear after resolution on the same database. Empty input is checked too.
4. Repository test uses the same repo instance with two independent pairs, revokes one while preserving the other, then revokes the survivor and observes denial on the next call. Conflict filtering and repository revocation are covered at their respective seams, not claimed as one end-to-end replicated-conflict/revoke test.
5. First full signer-repository run: 7 passed / 3 failed, 86.88s. Existing tests tried to drop or mutate Device tables absent from the current IdP schema. Corrected only those tests: absence is asserted directly; legacy malformed/duplicate Device data is explicitly created by a test-only fixture. No production Device schema was restored.
6. Final signer-repository run: **10 passed**, 94.75s (46.75s build). Retains malformed bindings/JWKs, issuer/expiry boundaries, global ambiguity, enrollment/rotation/revocation, and no-Device coverage.
7. First public-key test filter matched **zero** tests; not validation. Corrected `public_key_tests::` filter: **8 passed**, 68.80s. Includes public-only verification, principal-root binding, learned member/signer/client/principal revocation, independent-node introspection, native reopen, and no Management Device tables.
8. Strict affected-library Clippy: **PASS**, `-D warnings`, 37.59s. Scoped rustfmt check and `git diff --check`: **PASS**.

## Unchanged independent Join acceptance

Every completed paired run uses fresh isolated native roots and the existing test fixture's secure-provider/trust mocks. These are not real OS trust/provider certification. `use_joined()` still performs its normal Management checks followed by the first bearer-authenticated `GET /databases`; the same five-second HTTP bound and final current-policy checks remain in place. There is no extra catalog probe or acceptance retry.

| Invocation | Result |
| --- | --- |
| Initial unified build | Tool timeout at 200000ms during owner-library compilation; no scenario result |
| Next identical command | 2m45s build, then tool timeout during source Join setup/public import; no scenario result, no test failure/pass asserted |
| First complete paired run | Source child PASS 64.05s; historical child PASS 67.98s; 2 wrappers PASS / 132.32s; build 0.41s |
| Consecutive complete repeat | Source child PASS 64.00s; historical child PASS 65.82s; 2 wrappers PASS / 130.07s; build 0.36s |

Recorded first-catalog-path phases (milliseconds; cumulative only within each route/authentication operation):

| Complete run / fixture | Management authentication + identity | Selection | FINAL recheck | Storage service-token | Storage introspection cumulative | Storage response-time introspection |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| First / source | 2411 | 2463 | 2755 | 894 | 2796 | 401 |
| First / historical | 2603 | 2658 | 2967 | 927 | 2938 | 418 |
| Repeat / source | 2388 | 2440 | 2728 | 849 | 2706 | 378 |
| Repeat / historical | 2388 | 2439 | 2729 | 892 | 2224 | 416 |

These phases overlap with other requests/background work: do not add different operations' counters into a whole-request latency. No new whole-GET stopwatch was added; passing the unchanged bound is the request-level evidence. Receipt39's latest source Join was red at the same first catalog request. Here no completed Join fixture is red, but two repetitions are finite evidence, not proof that future runs cannot flap.

Fresh fixture IDs:

- first source: `01a15971-e21b-7384-964e-9db63976d643`
- first historical: `01a15972-dcd7-7613-a776-2e5d1a835548`
- repeat source: `01a15974-3579-70c5-b8f3-a43e77446970`
- repeat historical: `01a15975-2ff5-7359-b96b-b5738202e782`

## Literal commands and controls

Working directory: repository `of`. All Cargo calls sequential, sole granted Rust token; `timeout_ms:200000`, offline, locked, one job, incremental disabled, 16MiB minimum Rust thread stack. Two existing rust-analyzer processes (17703/23931) were paused after observing a build-directory lock wait and resumed at token release. No editor configuration was changed.

All test commands below were prefixed exactly with `RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0` and used exactly these receipt38 profile flags before `-- --nocapture --test-threads=1`:

```text
--config 'profile.dev.package.unified-server.debug=0'
--config 'profile.dev.package.unified-server.opt-level=0'
--config 'profile.dev.package.storage-server.debug=0'
--config 'profile.dev.package.idp-server.debug=0'
--config 'profile.dev.package.management-server.debug=0'
```

Exact Cargo argument prefixes, in execution order (repeat counts include failed/timeout attempts):

```text
cargo test --offline --locked -p idp-service --features replica --lib trusted_signers_exclude_only_conflicted_pairs_and_recheck_revocation
cargo test --offline --locked -p idp-model --lib signer_trust_batch
cargo test --offline --locked -p idp-model --lib signer_trust_batch
cargo test --offline --locked -p idp-model --lib signer_trust_batch
cargo test --offline --locked -p idp-service --features replica --lib replica::replica_signer_repo::tests::
cargo test --offline --locked -p idp-service --features replica --lib replica::replica_signer_repo::tests::
cargo test --offline --locked -p idp-service --features replica --lib oauth2::public_key_tests::
cargo test --offline --locked -p idp-service --features replica --lib public_key_tests::
cargo test --offline --locked -p unified-server --lib production_independent_join
cargo test --offline --locked -p unified-server --lib production_independent_join
cargo test --offline --locked -p unified-server --lib production_independent_join
cargo test --offline --locked -p unified-server --lib production_independent_join
cargo clippy --offline --locked -p idp-service --features replica -p idp-model --lib
cargo test --offline --locked -p idp-model --lib signer_trust_batch
```

Clippy used the same environment/profile flags but ended with `-- -D warnings` instead of the test harness arguments. Formatting commands:

```sh
rustfmt --edition 2024 crates/idp-model/src/replica.rs crates/idp-service/src/replica/replica_signer_repo.rs
rustfmt --edition 2024 --check crates/idp-model/src/replica.rs crates/idp-service/src/replica/replica_signer_repo.rs
git --no-pager diff --check
```

No all-features/powerset/workspace sweep. `just crap` was inspected but not run: it invokes workspace-wide llvm-cov/tests and workspace CRAP with a different stack profile, outside this task's explicit bounded selective-validation constraints.

Captured Join output SHA-256 (temporary logs transcribed here and removed rather than committed):

| Invocation | SHA-256 |
| --- | --- |
| Compilation timeout | `06ea05db9e64edacf11898bad5a5f61a5f03094e37c6011d918b2d8f06dbcd71` |
| Build + scenario tool timeout | `84a3d450325636e01e25147db4c98879f75647037706dfcbf121bee5de1f350d` |
| First complete paired pass | `8a8b459e0ad5645bdc6065e8563417207d2c2a31956a90a74086320422f6a8d8` |
| Consecutive complete paired pass | `3d99e3982e6d95cf75b6033216cb2da2dd71ad408401cc03a84d06896961f767` |

## Handoff

Commit scope is the two owned Rust files plus this receipt only; parent dirty lifecycle and Management changes remain untouched and uncommitted. This delivers the local coherent trust-batching improvement with targeted correctness and twice-repeated unchanged Join acceptance, not a claim that unrelated broad acceptance has been run. Rust token released after commit/validation.
