# Ticket 09: coherent key and Node-signer trust observations

## Approved scope and immutable inputs — 2026-10-27

Implemented on `implement/setup-authority-observation` in `of/.worktrees/setup-authority-observation`, starting at integration **`dd41049ee2e7d68efa5c71b9fab734e6a35756bb`**. Loaded AGENTS, TDD and ponytail; read the setup spec, current-read profile, Client-observation evidence, glossary map and IdP glossary. Approved public test seams: `KeyRepo` reads/root selection and `DbReplicaSignerRepo::trusted_signers`, over real native persistence. Assessment: medium complexity, high security risk; priority is eliminating demonstrated repeated catalog work without weakening authority decisions.

All Rust runs used sibling ofdb **`6286ca900f7a3fe73fa00e233656afab48d1fb81`**, including existing transaction-local catalog sharing; ofnet `dbea925a0a3f99053fffc02618baccc2a1cbea9f`; offs `df4c696a910296635473ec296d9235acbaa60d9b`. No sibling changes. Root `Cargo.lock` stayed byte-identical, SHA-256 **`a39e0f5211fb541b56d9e6cd8c498b2e9ce330c8ca2281c80cd4205ef5b532d1`**. No manifest/dependency changes.

Production scope is only `crates/idp-service/src/replica/key_repo.rs` read methods/private helpers and `replica_signer_repo.rs` trusted-signer read/private helper. Create, write, enrollment, rotation and revocation implementations remain unchanged. The old standalone key read used by creation retains the same SELECT and decoding behavior; decoding was extracted without changing it. No User/Application/Client production repository changes, provider/signing changes, new protocol, generic public API, cross-request cache, timeout/retry/concurrency changes, ticket/spec/map edits, SDK generation, main or push.

## Actual native red → green, before each production change

Narrowly extracted the existing `client_observation_tests` counting `Kernel`/`KernelTransaction` into test-only `observation_test_kernel.rs`; retained all existing Client test behavior. It delegates real work to native redb behind the real Automerge `EngineExecutor` and Unix SQL server. Public repositories use the real remote SQL client. No SQL/auth/conflict responses are fabricated. Counts are actual unbounded `tables`/`table_fields` storage-range traversals, not query commands or latency samples.

New authority fixtures use the three relevant CREATE TABLE definitions from the actual IdP migration. They omit secondary indexes, like the existing narrow Client harness, rather than pretending to be a full owner catalog or HTTP workload. Keys are explicit fixture rows; trust enrollment/rotation/revocation use their existing public methods. Direct native access constructs learned conflict/malformed evidence and observes storage work, not repository outcomes.

Shared command prefix for the following test stages:

```sh
cargo test --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-service --features replica --lib
```

| Stage | Exact filter followed by `-- --exact --nocapture` | Outcome |
| --- | --- | --- |
| `key-red` | `replica::authority_observation_tests::singleton_key_read_reuses_one_native_catalog` | Real live key returned, then failed: actual **[2, 2]**, expected [1, 1]; 0 passed/1 failed, body0.09s. Production key code still at base. |
| `key-green` | same | **1 passed**, body0.08s; one table and one field catalog traversal. |
| `trust-red` | `replica::authority_observation_tests::singleton_trust_read_reuses_one_native_catalog` | Real enrolled pair returned, then failed: actual **[3, 3]**, expected [1, 1]; 0 passed/1 failed, body0.10s. Production trust code still at base. |
| `trust-green` | same | **1 passed**, body0.09s; SELECTs and both conflict tables reuse one catalog. |

These are singleton savings, **keys2→1 and trust3→1**, not the previously rejected batch-only N=1 proposal. Empty reads still perform their normal SELECT/catalog observation. No latency or whole-workflow improvement is asserted.

## Implementation and authority/resource contract

`DbKeyRepo::read_keys` owns one existing `db::Transaction`, executes the existing full SELECT, decodes **every** `KeyRow` and complete `Key`, then applies the caller's existing active/entity/ID selection and clears selected keys in that same transaction. `list_active`, `list_by_entity_type_and_id` and `find_by_id` use it. Root selection/alias still goes through the entity listing: all active matching keys, including older roots and children, must clear **before** the newest root is chosen. Unselected conflicts do not deny an ID/entity read, but an unrelated malformed row still refuses the read because full decoding precedes filtering. Current-time filtering and full key fields/public-JWK decoding are retained.

`trusted_signers` opens one existing transaction for its combined two SELECTs, candidate construction and conflict clearance. `trusted_signers_in` inspects every selected signer first, then **every selected membership**, retaining one boolean per pair. A false signer clearance does not short-circuit member inspection or inspection of other pairs. Only afterward are conflicted pairs filtered and the existing ambiguity check applied. Exact binding, malformed JSON/row, canonical issuer, approval/expiry, revocation, public-key validation and ambiguity logic are unchanged.

Both read helpers capture their operation result, explicitly await **rollback**, then return. An original read error is retained after cleanup is attempted; a successful result is withheld if rollback fails. No read calls commit. Private-key provider access and signing are not moved into these transactions; existing public JWK parsing/verification-key validation remains part of the trust read.

One public read observes coherent rows/conflicts in one snapshot and reuses its validated catalog. Each subsequent public read opens a **fresh** transaction. No catalog/transaction survives the call. Native admission now spans the bounded repository observation; remote calls use the existing transaction command exchanges and deadlines. This may add remote round trips, so it is not a remote-latency claim. A commit after snapshot creation belongs to the next lookup, not midway through this lookup; this is coherent observation, not a blanket freshness guarantee at every intermediate instant. No new cancellation claim is made.

## Final semantic controls and pre-existing failures

`final-focused`, using the shared prefix plus `replica::authority_observation_tests:: -- --nocapture`, passed **11/11**, 71 filtered, body3.33s /supervised4.100s:

- Singleton key/trust catalog regressions, including public `KeyRepo::trusted_node_signers` delegation.
- Every key read/list/root alias starts a new one-catalog observation; insertion/rotation selects the newer root on the next call.
- All13 key fields round-trip, including parent, entity, derivation, hardening, name, future revocation/expiry, timestamps and public JWK.
- Current revocation/expiry and exact entity filtering precede conflict clearance; future revocation/expiry remains active; excluded inactive/unrelated conflicts do not spuriously deny a selected entity/ID.
- Real concurrent conflicts on **older roots and children** deny both root lookup aliases/listing before latest-root selection. Selected ID/all-active reads deny; unrelated ID lookup remains allowed; explicit native resolution is learned next call.
- Unrelated, inactive malformed key JSON/entity type/derivation index/timestamp still refuses reads, including a missing-ID read. Repair on the next native operation succeeds after refusal releases admission.
- Empty, inserted, deleted and newly revoked keys are observed by subsequent calls.
- Three independent enrolled trust pairs, with a real signer conflict in one and real membership conflict in another, retain only the unrelated clear pair. Additional bounded authority-document range counts are exactly **[3 signer, 3 member]**: no later selected signer or membership was skipped. Full catalogs remain [1,1]. Both concurrent branches change the original value. Explicit conflict resolution restores all three; public revocation removes only its pair on the next call.
- Empty/enrolled/rotated/expired/deleted trust, exact issuer and malformed JSON refusal; a subsequent native write succeeds after the failed read.
- A24-case native control retains exact member/signer/active-member/key/issuer/endpoint binding, approval, revocation and expiry refusals. Schema-invalid learned values (including malformed/null timestamps) are constructed through the existing native row codec, not by weakening normal SQL validation. Decisions are checked only through the public trust read.

`final-controls`, shared prefix plus `replica:: -- --nocapture`, executed all46 replica tests: **42 passed /4 failed**, 36 filtered, body22.83s /supervised88.336s. All11 new controls, all13 existing Client tests, existing key creation/path tests, trusted-pair/rotation/revocation tests, invalid-public-key and ambiguity tests, and the broad `local_identity_requires_exact_enrollment_active_user_and_current_trust` contract passed. Four existing tests failed during fixture writes with **`Invalid query: Column value has the wrong type`**:

1. `oauth2_authorization_code_repo::tests::code_consumption_survives_database_reopen_and_denies_replay` (`persist code hash`).
2. `oauth2_authorization_code_repo::tests::concurrent_consumers_have_one_winner`.
3. `replica_signer_repo::tests::trusted_signers_ignore_malformed_revoked_and_duplicate_devices` (`change public trust fixture`).
4. `replica_signer_repo::tests::trusted_signers_require_exact_active_member_and_signer_bindings` (`change public trust fixture`).

Verified provenance, not assumed: `baseline-existing` temporarily restored **only the two owned production files** to their exact `dd41049ee` content, ran `replica:: -- --skip authority_observation_tests --nocapture`, and reproduced **31 passed /the same4 failures**, body21.63s. Saved implementation bytes were restored in a `finally` block. Receipt source hashes prove the base implementations were used. No existing test was edited/skipped in the final broad run, and no unrelated repair was made.

The initial expanded `controls` run was36 passed/8 failed: the same four existing failures plus four new fixture mistakes. Corrected only new fixtures: use explicit `Engine::resolve_row` instead of SQL UPDATE on conflicted rows; make both concurrent timestamp branches actually differ from the original; use a schema-valid out-of-range derivation index instead of SQL assigning text to an integer. The ensuing nine-control run passed9/9; two more controls were then added for full-field decoding and malformed/exact trust evidence. These fixture errors are not reported as production regressions or hidden as successful validation.

## Lint, formatting and qualification limits

- `cargo clippy --locked --offline --target-dir /home/nathan/code/rust/aicacia/of/target -j 1 -p idp-service --features replica --lib --tests --no-deps -- -D warnings`: **failed only on the existing `clippy::items_after_test_module` at unchanged `oauth2/config.rs:52`**. No changed-file diagnostics. No unrelated edit or lint allowance.
- Same strict Clippy command with `--lib` only (no `--tests`): **passed**, supervised5.134s.
- `rustfmt --edition 2024 --check` on all six changed/new Rust files and `git diff --check`: **passed**.

No full feature matrix, owner HTTP qualification, coverage or `just crap` in this approved focused slice. The parent's latest untraced **38 passed/6 failed/6 ignored** (four authenticated Storage timeouts/two JoinStorage readiness failures) is supplied context, **not an execution by this slice and not declared fixed**. The current-read profile's33/40 catalog stack hits motivate this boundary but are not invocation counts or a measured timeout allocation. Ticket09 and integrated qualification remain open.

Final test executable: `/home/nathan/code/rust/aicacia/of/target/debug/deps/idp_service-3ce1b9745a9173f0`, SHA-256 **`a98a01b12c257b4b091145bea55aeb89293bd266f2fa7b5d90c01bdddcd765de`**. Red/green executables at that path were replaced later; their separate source/log receipts establish their runs, not the final artifact hash.

## Supervision, receipts and handoff

Exclusive Rust-owner slot; every Rust stage sequential, jobs1, shared root target, `RUST_MIN_STACK=67108864`. Adapted the existing private supervisor. Guards: MemAvailable below2GiB, free disk below30GiB, PSI memory some>=20/full>=10 for20 sustained seconds,7200s per-command ceiling. No resource/time guard fired. No external process was killed. Minimum free disk across stages392,444,497,920 bytes. All eleven owned Rust groups were checked exited.

Private receipts: `/tmp/setup09-authority-observation/`, directory0700/files0600. Each stage has actual argv/cwd/heads/source hashes/result in `<stage>.json`, raw output in `<stage>.log`, bounded resource samples in `<stage>.resources.jsonl`. Source hashes include the unchanged lock; the baseline receipt records the old production hashes. `final-artifact.json` identifies the final executable. The primary worktree's **seven external SDK diffs, including symlink and resolved target contents, were fingerprinted and verified unchanged**. No SDK install/build/generation occurred.

| Stage | Exit | Supervised s | Minimum available KiB | Peak sampled owned RSS bytes | Max PSI some/full avg10 |
| --- | ---: | ---: | ---: | ---: | --- |
| key-red |101|191.633|7,800,500|1,749,680,128|0.90/0.72|
| key-green |0|104.151|8,018,272|1,085,300,736|0/0|
| trust-red |101|71.084|8,151,188|1,109,999,616|0/0|
| trust-green |0|77.765|8,020,784|1,037,119,488|0/0|
| controls: initial expanded fixtures |101|118.005|8,973,940|1,253,908,480|0/0|
| authority-controls: corrected nine |0|53.037|9,206,540|1,112,113,152|0/0|
| baseline-existing |101|87.096|9,103,532|1,172,627,456|0/0|
| final-controls: all replica tests |101|88.336|9,099,120|1,030,721,536|0/0|
| final-focused: eleven controls |0|4.100|9,817,996|139,653,120|0/0|
| clippy-tests: existing lint |101|8.176|9,560,648|599,531,520|0/0|
| clippy-lib |0|5.134|9,497,136|445,198,336|0/0|

Log SHA-256 receipts:

| Stage | SHA-256 |
| --- | --- |
| key-red | `410ce261dd4917249a3754b13f321a6c7ad0699d6bfb0c1e598950914a525a11` |
| key-green | `441678fb80ee73ffaac505559541fbe05279bb269323bf428c6c9aafe093842d` |
| trust-red | `a0068887262970fe10254d6578804c8d3b9ccb73dddaa84b425f13357cc628c7` |
| trust-green | `b8324210ab425db9a0c4886654d73df2a2b7c590d448d00b1fb6d1a24d92b80f` |
| controls | `6cec8fe15fad86bb3663ca3474bff3a31fa9f9924173bb15f0f7a9cf8aa99b42` |
| authority-controls | `5b3a0149d14f4f166218b65bd5ea0dd60d7c0bfeb257533e0bb7355769583eb6` |
| baseline-existing | `94c0611e2ed5696e1ab38d95bceb309a1faf6253beef2944b385ebc24336af13` |
| final-controls | `445c7c242a752303228b229c368a7ece948c53e6972d76cdd9b77df8911257cb` |
| final-focused | `a8367714216117a30a28c63b68fae6f8f13dc6e4ef8c639c2b324e5432b15961` |
| clippy-tests | `eb3a9ad4f3cd099b28437f5c9801f3fc358d0bcdf7803ca9581a7f49c0f4a2a3` |
| clippy-lib | `ed87f4cf50a08fcaa510174e888a91147e7c60adf9431d930cb182bfad9dc172` |

Integration tip checked at `dd41049ee` before commit/synchronization. No background validation remains; no main/push or sibling mutation is part of handoff.
