# Ticket 09: one coherent Client repository observation

## Authorization, inputs and scope

Implemented the approved root slice on `implement/setup-client-observation` in `of/.worktrees/setup-client-observation`, starting at **`89339620a49624f3a9463186044f4de385ec6a16`**. Confirmed that integration input is an ancestor (initial HEAD exactly matched it), loaded TDD/ponytail, and read root AGENTS/shared glossary. Approved test seam: public `ClientRepo` finders/listing over real native persistence. Complexity low-to-medium for the lookup rewrite; security risk high because credential revision, conflict denial, snapshot lifetime and cancellation must stay coherent.

All runs used sibling ofdb **`6286ca900f7a3fe73fa00e233656afab48d1fb81`**, including reviewed transaction-catalog reuse `2b22b9ee`. Existing ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`, offs `df4c696a910296635473ec296d9235acbaa60d9b`. No sibling edits or branch/main/push operations. The parent's reported sibling nine controls/152 passes and unresolved `scan_layout` 512-table timing failure are **not executions by this slice**, are not pooled into root results, and are not claimed resolved here.

Owned implementation: `crates/idp-service/src/replica/client_repo.rs`; dedicated `client_observation_tests.rs`; this evidence; minimal dev-test manifest/lock wiring. Strictly mechanical receiver migrations were separately approved, including compiler-discovered ones listed below. No issuance-revalidation, Storage response-recheck, key batching, password/signing/provider behavior, retry/deadline, persistent cache/session interface, broad concurrency, ticket/map/spec or SDK changes.

## Initial regression before rewriting production lookup

Added a test-only counting `Kernel`/`KernelTransaction` that delegates all work to **native redb**, behind the real Automerge `EngineExecutor` and existing SQL Unix-socket server. `DbClientRepo` uses its real public SQL client; no SQL/auth responses are fabricated. Counts are actual unbounded table/field-catalog storage-range traversals, not query-command counts. Successful schema reads consume the complete ranges. Fixture uses the two relevant table definitions from the actual IdP migration, real Client rows, matching local credential revision and known opaque verifier strings; unused private-key storage stays in memory.

The fixture is intentionally narrow, not a full owner catalog/HTTP benchmark. Public lookup results and security decisions are asserted through `ClientRepo`; direct engine access only constructs/mutates input evidence and counts underlying work. No production observation hook or new testing framework.

Test wiring adds only existing `futures` and `ofdb-sql` as dev dependencies and `sync`/`time` to the existing Tokio dev dependency. The first offline Cargo invocation updated **only two `idp-service` dependency edges in Cargo.lock** (`futures`, `ofdb-sql`); package/version sets and all other package entries were verified unchanged. No version regeneration. Subsequent Rust commands used `--locked --offline`. Original lock SHA `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`; final lock SHA **`a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1`**.

### Compiler prerequisites, not behavioral red

1. User-directed first migration: `crates/device-service/src/replica/schema.rs:88`, `let transaction` -> `let mut transaction` for `validate_device`'s existing conflict read.
2. Initial focused compilation then failed **E0596** at `crates/idp-model/src/replica.rs:77`, before the regression executed. Stopped and requested approval because that file was outside the initial ownership. User approved that binding plus strictly mechanical compiler-required receiver migrations. Changed only line76's binding to `mut` in `validate_identity`.
3. Later owner-target compiler check failed **E0596** at `crates/idp-server/src/router/node_join_approval.rs:346`. Changed `require_clear`'s `&Transaction` parameter to `&mut Transaction` and its enrollment caller to pass `&mut transaction`. Its other caller already had a mutable reference.

Those are **all** mechanical migrations in this slice. No ordering, validation, query, commit/rollback, error, authorization or other behavior was changed in those three files. The final all-features owner-target check passed.

### Actual red -> green

The exact regression filter is:

```text
replica::client_repo::observation_tests::singleton_lookup_reuses_one_native_catalog
```

- **Actual red**, production lookup still unchanged: `cargo test --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-service --features replica --lib <filter> -- --exact --nocapture`. Test executed, read the expected live Client/local verifier, then failed its storage work assertion: **actual `[4, 4]`, expected `[1, 1]`** table/field traversals. 0 passed/1 failed/63 filtered; body0.29s, supervised131.605s including compilation. This is distinct from the earlier compiler failure.
- **Green**, after rewriting only the lookup: same locked command/filter, **1 passed/63 filtered**, actual one table/field traversal each, body0.07s, supervised62.244s including compilation. No timeout/stack/profile/concurrency adjustment or repeated unchanged invocation to obtain green.
- Later final controls include that regression again, plus both actual conflict-denial controls. This is work elimination for a singleton, not per-key batching and not a latency benchmark.

## Implementation and security/resource contract

`registrations` and credential-hydrating `clients` now take caller-owned `&mut db::Transaction`. Both ID finders share a private finder that opens one existing transaction, reads metadata, inspects local-credential conflicts, reads the matching local credential, then inspects the selected Client metadata conflicts **inside that same transaction**. Reused the already-existing typed `local_credential_select` instead of formatting/parsing the same SELECT string.

Metadata-only registration lookup and listing similarly own one existing transaction. Listing still hydrates/checks local credentials **before** `skip`/`take`, then checks Client metadata conflicts **only for selected rows**, preserving prior pagination/error behavior.

Each read path captures its result, awaits explicit **rollback**, and only then returns it. A successful result is withheld if rollback fails; an existing lookup error is preserved after the cleanup attempt. No read completion calls `commit`, so this does not reintroduce the separately observed durable SELECT completion path. Existing create/update write paths remain unchanged. Their standalone conflict preflight remains outside the read transaction; no `self.engine` standalone call is made while holding a read transaction.

No password verification, key-provider operation, signing or owner HTTP call was moved into these transactions. Each public lookup creates a **new** transaction/snapshot; issuance revalidation and response reauthorization remain separate existing fresh invocations. One lookup now observes coherent metadata/credential/conflict evidence rather than four potentially torn snapshots. A concurrent commit after snapshot creation is intentionally observed on the **next lookup**, not midway through this one; this is coherence, not a claim that it is strictly fresher at every intermediate instant.

Existing native admission lasts across the bounded lookup, and remote SQL transactions span the existing command exchanges. Per-command SQL deadlines and stream-drop cancellation semantics are unchanged; opening/completing a remote transaction adds exchanges, so a remote-latency improvement is not asserted. No transaction object or catalog is retained across invocations.

## Final focused behavioral controls

Ran:

```sh
cargo test --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-service --features replica --lib replica::client_repo:: -- --nocapture
```

**13 passed /0 failed /58 filtered**, body3.76s /supervised66.221s. Eight new native controls plus five existing Client repository tests:

- Singleton external-ID lookup: one table/field-catalog traversal each, correct local verifier returned.
- Repeated by-ID and metadata-only lookups: each starts a fresh catalog; metadata returns no verifier.
- Matching credential, learned public revision mismatch, newly matching local revision/changed verifier, and absent local credential are observed on subsequent calls with unchanged outcomes.
- Metadata lookup succeeds with the local credential table absent and exposes no verifier; hydrated lookup fails closed; later native write succeeds after the failed lookup released admission.
- A subsequent lookup observes committed Client revocation, including metadata-only lookup.
- Independently introduced real concurrent Automerge conflicts in **local credentials** and **Client metadata** each deny both hydrated finders. Metadata-only lookup ignores credential hydration but still denies Client metadata conflicts. Next lookup sees explicit conflict resolution and the correct verifier. These exercise both retained conflict inspections; no empty/fabricated conflict responses.
- Pagination excludes an unselected Client metadata conflict, denies a selected one, but still denies an excluded local-credential conflict because hydration precedes pagination.
- Cancelled lookup while waiting for **actual held native admission** does not prevent a subsequent fresh lookup after the held transaction rolls back. This covers admission cancellation, not every possible mid-query cancellation point.
- Existing local durable verifier/reopen/missing-verifier, failed registration rollback, secret disclosure/rotation, denied private capture, and public/confidential creation tests pass unchanged.

Final executed unit-test artifact (resolved from the controls Cargo output): `/home/nathan/code/rust/aicacia/of/target/debug/deps/idp_service-3ce1b9745a9173f0`, SHA-256 **`35ad3c739fc68657313b5940757f5d300984fefd282b81427f1f1abe717afb7d`**. Earlier red/green executables at that path were replaced by subsequent compilation; their separate source/log receipts, not this final hash, establish those runs.

## Compiler, lint and formatting validation

- `cargo check --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-unified --all-features --test owner_boundaries`: initial attempt found the mechanical `require_clear` receiver error above; after that exact migration, **passed**,19.381s supervised. Covers the production owner composition and owner-boundaries test target; no owner HTTP tests executed here.
- `cargo clippy --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-service --features replica --lib --tests --no-deps -- -D warnings`: **failed** solely on existing `clippy::items_after_test_module` in unchanged `crates/idp-service/src/oauth2/config.rs:52` (implementation follows its tests). No changed-file diagnostics were reported. Did not change unrelated code or add a lint allowance.
- Same Clippy scope with `--lib` only, omitting `--tests`: **passed strictly**,51.966s supervised. This is not a claim strict test-target Clippy passed.
- `rustfmt --edition 2024 --check` on the five changed Rust files and `git diff --check`: **passed**.

No full feature matrix, full default-concurrency owner workload, coverage or CRAP run in this parent-approved focused slice. Full qualification remains parent-owned. Neither the old readiness/Storage timeout nor the sibling scan-layout timing failure is declared fixed by this work-count regression.

## Supervision and receipts

Exclusive Rust slot, all Rust stages sequential with jobs1, shared target, `RUST_MIN_STACK=67108864`. Adapted the existing supervisor example without overwriting it. Guards: MemAvailable below2GiB, free disk below30GiB, memory PSI avg10 some>=20 or full>=10 for20 sustained seconds,7200s ceiling; bounded polling throughout. No unrelated process was terminated. No resource/ceiling guard fired; full swap alone was not a veto.

Private receipts: **`/tmp/setup09-client-observation/`**, directory0700/files0600. Each `<stage>.json` records actual argv/cwd/heads/source hashes, return code and resource summary; `<stage>.log` and `<stage>.resources.jsonl` retain raw output/samples. The first stage is named `red` but is explicitly a compiler failure; `red-ready` is the actual behavioral/work-count red.

| Stage | Exit | Supervised s | Minimum available KiB | Peak sampled owned RSS bytes | Max memory PSI some/full avg10 |
| --- | ---: | ---: | ---: | ---: | ---: |
| red: compiler prerequisite failure |101|487.165|3,074,388|1,175,363,584|1.08/1.08|
| red-ready: actual regression failure |101|131.605|3,029,824|1,718,550,528|1.99/1.99|
| green |0|62.244|3,430,520|1,149,960,192|0/0|
| controls |0|66.221|3,366,776|1,007,865,856|0/0|
| clippy: existing test lint |101|72.555|3,540,836|868,483,072|0/0|
| owner-check: receiver failure |101|79.416|3,734,672|747,352,064|0/0|
| owner-check-migrated |0|19.381|3,638,100|748,666,880|0.18/0.18|
| clippy-lib |0|51.966|3,752,772|555,139,072|0/0|

Minimum free disk across all stages396,967,931,904 bytes. No SDK install/build/generation occurred. The primary worktree's seven external SDK diffs (including the symlink target) were fingerprinted before execution and verified unchanged afterward; no restoration or edits there. Existing sibling `.worktrees/` infrastructure remains untouched.

Log SHA-256 receipts:

| Stage | SHA-256 |
| --- | --- |
| red | `14ac5b795375524db92d405fd0d7a41a55549205839ad5378359a9c3793b0adb` |
| red-ready | `91ca5de2034ab0af9aff712973ef3ae785841563948b860afc003a84c8d57441` |
| green | `68fd0f5f7680a5319b447a50f817313a560251390ecc8bbb7d1f4ed8931af6b5` |
| controls | `b0603415e359faf61b4472b66998e3228bf1704df1b0c228a06729777b048834` |
| clippy | `d4ee5715512f123afb7b3cc1a91de8cbb540c02c8180e34f6cf7098bf5656c38` |
| owner-check | `c1579215bdb9a76f5f594bc15569ade017b1e404e2b9ef8a4b80e090740c4809` |
| owner-check-migrated | `45550b42821b7486cc8833f5d384c109b1f7df8812c8758fb3ab1387c2b4fa16` |
| clippy-lib | `ce5a31f90aff2af4b63d79cb0141a5b84f2fed943baca11c131ef31cdf34cfca` |

All known owned Rust groups have exited. Integration tip checked at89339620a before commit/merge confirmation; this branch alone owns the implementation/evidence commit. Slot released after final checks. No background validation remains and no main/push/sibling mutation is part of handoff.
