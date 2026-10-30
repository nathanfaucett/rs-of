# 03: Resolve all five SQL review findings

Type: task
Status: resolved
Blocked by: 01, 02

## Comments

- Single implementer in `of-password-sql-review`, branch `implement/password-manager-sql-review`. Clean initial HEAD and merge-base with `integration/password-manager-sql` both confirmed exactly `d3eae862519b77d9ddfc9439aeff29eec945d608`.
- Read spec, tickets/map, AGENTS, domain/API and underlying owner code; used TDD/ponytail and loaded Svelte skills. Agreed seams are the spec's authenticated native HTTP, runtime restart/retirement, public owner facade and mounted app workflow with controlled transport. No Svelte component was changed.
- Risk/priority: high confidentiality/data-loss risk; prioritize fail-closed acknowledged content, authorization before execution, interrupted cleanup and atomic mutation/snapshot rollback. Avoid unrestricted HTTP SQL until owner-wide budgets exist.

## Answer

All five findings verified and fixed. Regression tests were observed red for truncation (`200` ensure), write-only RETURNING (`200` disclosure), interrupted cleanup (`NotFound`), row/byte boundary writes (committed unreadable vault), and unsupported amplifying SQL (subquery accepted). Public-owner tests initially failed for the missing existing-only open and native-limit interfaces.

### Exact fixes

1. **Acknowledged corruption:** `DatabaseRuntime::ensure` and ordinary ready-resource opens use public `ofdb_sql::Database::open_existing`, which delegates to the owner's existing non-creating native open. Creating opens remain for initialization/selected fresh copies. Restart HTTP tests first commit data, close all owners, replace persistence with zero bytes or junk, then assert both ensure and SQL execution return 503 without replacing bytes. Generic missing/corrupt/persisted-file behavior is tested at the owner facade. No internal Redb dependency was added to `of`.
2. **RETURNING:** classify all translated Insert/InsertValues/Update/Delete RETURNING as requiring read **and** write. Real authenticated HTTP tests deny write-only and read-only tokens before mutation, including the no-op `content=content` disclosure. Each denial returns only the fixed forbidden body and a subsequent authorized SELECT proves the data unchanged. Authorized RETURNING and ordinary write-only mutations still work.
3. **Interrupted retirement:** internal catalog lifecycle lookup includes initializing/deleted entries while public get/list remain ready-only. Selected-copy registration cannot expose initializing entries. Retirement takes the open lock while establishing the local fence/capturing an issued owner, then retires issued owners or removes unacknowledged staging/published files safely. Restart tests cover staging-only, valid published and corrupt published initialization; cleanup retries complete, public visibility stays absent, and the resource fence survives owner activation. Selection reconciliation no longer silently completes an unknown-to-ready-only-lookup initializing cleanup.
4. **Atomic snapshot:** collection mutations send mutation + full `SELECT id, content FROM secrets` in one public SQL batch with read/write actions. Validate the returned second result and record identities before publication. Acknowledged writes do not require another post-commit read. Rejected/uncertain responses still reconcile without replay, preserve existing safe/unsafe-draft semantics and never report false success. Mounted controlled row (1,000 → 1,001) and aggregate-byte boundary cases prove rollback/readability and a successful subsequent repair. Native HTTP independently proves aggregate-byte rollback; the existing native 1,001-row batch rollback remains green.
5. **Allocation bounds:** parsed HTTP policy rejects joins, subqueries/EXISTS, aggregates/grouping/HAVING, DISTINCT, computed/repeated projections, LIKE and unsupported DDL **before transaction admission**. Generic single-table CRUD, CREATE TABLE, predicates, ordering/limits/offsets and authorized RETURNING remain documented. Every HTTP transaction sets the public native read budget to 65,536 encoded entries/8 MiB, plus separate cumulative decoded scalar-row accounting with the same limits. Borrowed native bytes are admitted before owned key/value decoding or full-range accumulation. Decoded accounting includes row/value storage, empty/defaulted fields and payloads before scan accumulation/mutation encoding. Plain scans, LIMIT 1 and unmatched predicates cannot bypass it. Owner exhaustion aborts the batch and maps to redacted 413. Existing stricter 1,000-row/1 MiB wire checks remain pre-commit. A held-owner-transaction HTTP test proves rejected shapes do not wait for execution; real compressed-content HTTP scans exercise bounded failures. Native tests prove oversized get/range/mutation reads fail **before value decoding/allocation** and that limits cannot be reset. No timeout is used as an execution-budget substitute.

### Neighbor repository handoff

`ofdb` was initially clean on `main` at `849420cabca766bdfaee1e066f875ce01f2468a9`. Authorized work is isolated on its own branch `implement/password-manager-sql-review`:

- **`69813e339ef5d4d5bba3dd896488f1d2b0e379a9`** — `Expose existing-only SQL opens and native transaction read budgets`.

The `of` changes require that owner commit. No `ofnet` or `offs` source was changed. Native byte admission lives in the owner; HTTP policy/catalog identity and application behavior stay in `of`. Ordinary unbudgeted clients remain supported; remote/non-native limit configuration fails closed as unsupported. A new public SQL `ErrorKind::LimitExceeded` preserves the category without matching error messages.

### Validation actually run

Commands used the isolated review manifest/worktree. Native tests use `RUST_MIN_STACK=67108864` where needed; the explicit 2 MiB sync-worker regressions retain their original worker sizes.

Passed:

- `cargo test -p idp-unified --test owner_boundaries --no-default-features sql_http_tests`: **18 passed**, one subprocess entry intentionally ignored by the outer harness and executed by the crash test.
- Same target with `residency_http_tests`: **7 passed**, preserving filesystem behavior.
- `cargo hack test -p storage-service --feature-powerset --all-targets`: complete, **25 passed**.
- `cargo test -p storage-server --lib --all-features`: **95 passed**, one helper ignored; includes real replication and explicit production-sized-worker tests. An introduced stack regression was isolated against a passing untouched baseline and fixed by keeping synchronous native range admission/materialization out of nested async poll frames; no stack-size increase was made.
- `pnpm --filter password-manager test`: **67 passed** (63 mounted cases); config tests **2 passed**. Dependency builds, production app build and Svelte type check (**0 errors/warnings**) pass. Changed TS/fixture/test Biome check and Rust format/diff checks pass. A parallel-loaded run had one existing SDK test timeout; the isolated full app rerun passed all 67.
- Strict Storage service/server library Clippy passes. Owner facade all-feature library Clippy, new owner integration-test Clippy and B-tree/native-engine all-target Clippy pass.
- Owner focused all-feature suites `existing`, `read_limits`, `local_sql`, `retirement`, `sync_stack` pass, including unchanged 2 MiB staged sync. B-tree all-target tests (including pre-allocation counter regression) and native kernel all-target tests pass.

Broader checks/limits, **not** claimed green:

- Storage server powerset passes all-feature tests, then fails at no features (2/14) on unchanged `crates/storage-server/src/lib.rs:51`, ungated `device_client` re-export (`E0432`). No full server powerset claim.
- Owner SQL/B-tree/native-engine powerset timed out at **180 s**, reaching SQL combination 6/781 on its final attempt. No complete owner powerset claim.
- Full owner all-feature/all-target tests expose two failures also reproduced on untouched owner base `849420c`: `scan_layout::native_conflict_checks_reuse_raw_table_catalog_within_budget` exceeds its fixed 18 s machine-local timing threshold; `scoped_sync::candidate_outgoing_export_correlates_all_selected_rows_once` observes 2 callbacks instead of 1. These unrelated baseline tests were not edited or silently reclassified as passing. Partial broad-suite passes do not establish full owner-suite success.
- Strict owner all-target facade Clippy with only redb/sql hits unchanged feature-dependent unused imports in `crates/sql/tests/client.rs`; focused new tests and changed libraries pass strict lint.
- `just crap --summary` ran in both repositories with **180 s** bounds. `of` timed out during instrumented workspace compilation; `ofdb` timed out during workspace coverage (9/32). No completed CRAP score or coverage result is claimed.
- Interrupted Rust builds produced stale incremental linker errors; package-scoped build artifacts were cleaned and focused tests rerun successfully. No source workaround was added.

### Contract and qualification limits

The HTTP contract is intentionally narrower than unrestricted owner SQL. Budgets may reject history-heavy/catalog-heavy operations even when their projected response would fit. Native encoded admission occurs before owned reads; a codec must decode one document/row before its logical size is known. These bounds are not a hard process-memory/decompression/CPU/concurrency/wall-clock sandbox. Existing oversized replicated/history-heavy resources may require trusted native/operator recovery. Local cancellation does not prove rollback.

No deployed Installation/browser/OS-provider qualification, production-password-vault claim, secure erasure, global serialization or mesh convergence is inferred. Tests use non-sensitive records. Deployment browser sign-in/CRUD/reload/Node restart/denial/recovery remain manual as documented in the app README.


### P1 follow-up: selected-open acknowledgement bypass

The initial review fix did not close `open_selected`: protocol-first opening could
still create and cache an empty owner for missing/zero-length ready content. This
follow-up was implemented on the same review branch **after first merging
`integration/password-manager-sql` at `a59f0c3c5562b5fb061266c32dd0e01e57bd9065`**
(fast-forward). The extended regression was observed red: protocol-first selected
open succeeded after acknowledged content was removed.

- Selected SQL opens now use public `Database::open_existing`. Creating an empty
  SQL copy requires explicit durable local initialization intent, not file absence,
  catalog readiness or `generation > 0`.
- Fresh selected admission records local SQL initialization intent before catalog
  publication. Completed retirement records fresh intent while still fenced,
  before copy-generation advancement. Trusted reselection still has to clear the
  local fence; Tombstones, initializing catalog entries and retired owners remain
  denied. Existing catalog entries without fresh intent fail closed.
- SQL/KV selected admission uses consistent SQL-then-KV lock ordering so KV-first
  registration cannot race SQL acknowledgement. Staged SQL initialization is shared
  with ensure; already-published content is only validated. Successful selected,
  ordinary and ensure SQL opens consume fresh intent durably before cache insertion.
  The intent survives an interrupted fresh bootstrap without authorizing replacement
  of an acknowledged copy on a later restart.
- The native authenticated HTTP regression now commits data, closes every owner,
  removes/truncates/corrupts content, restarts, invokes `open_selected` **first** and
  then requires ensure/SQL execution to return 503 without replacing bytes. It covers
  both original and retired/reselected copy generations (six cases).
- Two additional service tests cover KV-first fresh admission interrupted before SQL
  initialization, restart/reopen with persisted records, and consumption of pending
  fresh intent when ensure or ordinary opening acknowledges published content.
  Existing retirement/reselection, Tombstone and owner-fence regressions remain.
- No `ofdb`, `ofnet` or `offs` source or branch was changed. The existing public owner
  API from `69813e339ef5d4d5bba3dd896488f1d2b0e379a9` is sufficient.

Follow-up validation: Storage service feature powerset/all targets **27 passed**;
Storage server all-feature library **95 passed**, one subprocess helper ignored;
strict changed service/server library Clippy passes. Full native SQL HTTP suite
**18 passed**, one subprocess helper intentionally ignored by the outer harness;
its selected-first corruption case covers all six original/reselected damage cases.
Rust formatting and `git diff --check` pass. `just crap --summary` was rerun with
a **180 s** bound and timed out during instrumented workspace compilation
(GTK/Tauri dependencies, 883/987); no completed CRAP/coverage result is claimed.
No Svelte/TypeScript code changed, so app build/type checks were not rerun for this
Rust lifecycle-only follow-up. No deployed qualification is claimed.
