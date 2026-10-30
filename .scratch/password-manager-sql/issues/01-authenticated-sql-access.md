# 01: Provide explicit authenticated SQL database access

**What to build:** An authenticated application can explicitly create/reopen its intended Database Resource and execute parameterized SQL through Storage, the Storage client, and the session-guarded SDK. Concurrent creation and uncertain-response retries resolve the intended resource under a documented generic identity contract. No application defaults or automatic provisioning live in the SDK or backend.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Establish and document one generic resource-identity/creation contract before implementing it: application-chosen logical identity, authenticated namespace scoping, concurrent requests, lost-response retries, restart/interrupted creation, deletion/recreation, and disconnected-Node behavior. Distinguish request idempotency from logical identity; do not infer cross-Node uniqueness from a local lock or introduce a central writer.
- [x] Carry explicit creation/reopen behavior through the existing Resource Catalog/runtime, authenticated Storage interface, generated contracts, Storage client, and SDK. Names remain non-unique display labels; do not use list-then-create or browser preferences as the authoritative identity mechanism.
- [x] Authenticate through the IdP and exchange the verified User access token for `storage:databases` with required actions. Preserve the canonical User/Application namespace across exchange; reject browser namespace overrides. Keep Application URI, internal Application ID, OAuth Client ID, and audience distinct.
- [x] Expose one bounded HTTP SQL execution operation supporting parameterized statements and atomic batches using the existing public SQL facade and transactions. Define wire values/results and redacted errors; reuse existing owner facilities and appropriate request/result limits. Do not add per-table CRUD endpoints, browser transaction sessions, or a new query WebSocket protocol.
- [x] Enforce current namespace, resource lifecycle/local-copy fences, and action permissions at execution. Authorize parsed/translated operations rather than SQL string prefixes; read-only tokens cannot execute DDL, mutations, or mixed write batches. Failed batches roll back atomically.
- [x] Preserve SDK session-generation/cancellation behavior, suppress obsolete results, and reject retained capabilities after retirement. Same-identity renewal remains usable. Authentication restore and obtaining Storage access cause no database creation or schema initialization.
- [x] Preserve existing filesystem operations and their correct audience. Keep secret schemas, default selection, collection ownership, Device selection, and replication policy out of the SDK/client/database operation.
- [x] Never log tokens, secret SQL parameters/results, or credential-bearing response bodies. Cancellation or timeout does not prove a sent operation rolled back; do not automatically replay arbitrary mutations.
- [x] Verify behavior through authenticated Storage requests backed by real persistence where practical: creation concurrency/retry/restart, interruption recovery, Tombstones/unavailable-copy handling, namespace isolation, same-Application multi-Client access, wrong audience/actions, spoofing, atomic rollback, and denial after deletion/fencing. Test disconnected-Node behavior against the chosen contract without overstating guarantees.
- [x] Extend existing client/SDK tests for exchange, service prefixes, explicit creation/execution, cancellation, retirement, and late results. Keep generic SQL integrity tests with the persistence owner; no testing-only abstraction or duplicate persistence implementation.
- [x] Update owner/client contracts and run focused validation plus applicable final feature-combination and complexity checks. This slice is verifiable independently of the password-manager cutover.

## Comments

- Claimed on `implement/password-manager-sql-01` in isolated worktree `of-password-sql-01`; base confirmed at integration commit `fbd121ddd`. Agreed test seams: authenticated Storage HTTP with real persistence and public Storage client/SDK lifecycle interfaces (per spec).

- Before implementation: selected namespace-derived UUID v5 logical identity with durable initializing/ready catalog lifecycle; permanent Tombstones, fail-closed acknowledged missing copies, equal offline identity without global serialization. Exact wire/lifecycle/bounds documented in `crates/storage-service/docs/api.md`. Complexity/risk: medium cross-layer implementation, high authorization/data-loss risk; prioritize HTTP persistence/rollback/fence tests before client wiring. No neighboring repository changes required.

## Answer

Implemented on `implement/password-manager-sql-01`, initially based exactly on
`integration/password-manager-sql` at `fbd121ddd`. All source changes are in the
isolated `of-password-sql-01` worktree. No neighboring repository was modified.

### Ticket-02 consumption contract

- `sdk.storage()` returns explicit guarded `ensureDatabase(request, signal?)`,
  `createDatabase(name?, signal?)`, `listDatabases(signal?)`,
  `getDatabase(id, signal?)`, `deleteDatabase(id, signal?)`, and
  `executeSql(id, request, actions, signal?)` in addition to unchanged filesystem
  capabilities. The Storage client forwards generated contracts; the SDK guards
  the captured session generation. Authentication/restoration/access do not
  provision anything. No application key, default, schema or Device policy is
  supplied by either package.
- `ensureDatabase({creationKey, name?})` calls `POST /storage/databases/ensure`
  with database `write` and returns `{id, name}` on 200 for creation or reopen.
  The application chooses a stable case-sensitive key (1–128 UTF-8 bytes).
  Names are non-unique labels, retained from first local acceptance, not lookup
  keys. Unkeyed `createDatabase` remains intentionally fresh per call.
- Logical ID is UUID v5 in the URL namespace over compact UTF-8 JSON
  `["of:database:creation:v1", verifiedSubject, canonicalApplicationUuid, creationKey]`.
  A canonical Application UUID is not its URI, OAuth Client ID or token audience.
  Namespace comes only from validated authorization. Same-Application Clients
  share it. Same namespace/key yields the same identity across independent Nodes,
  not globally serialized creation/writes or deployment/replication qualification.
- Durable initializing intent precedes SQL-owner initialization. Closed initial
  content is synced/published before ready acknowledgement. Retrying interrupted
  intent is safe; ready missing/corrupt content is never replaced by empty data.
  Tombstones permanently consume a key (`410`); local-copy fences are `409`;
  unavailable acknowledged content/authorization is `503`. Deliberate replacement
  needs a different explicitly chosen key. Unaware partitioned Nodes may continue
  until they learn the Tombstone. Losing the entire catalog is operator recovery,
  not something ensure can infer or repair.
- `executeSql(id, {statements:[{sql, params?}]}, actions, signal?)` calls
  `POST /storage/databases/{id}/sql`. Actions are explicit `("read"|"write")[]`:
  parsed SELECT needs read, DDL/mutations need write, mixed batches need both.
  Parameter values are tagged null/boolean/integer/float/text/uuid/blob. Integers
  are decimal i64 strings. Store serialized application records as text. Owner
  Type/JSON result values are unsupported rather than silently coerced.
- Results are `{results:[{columns:string[], rows:SqlValue[][]}]}` in translated
  statement order, with no affected-row-count promise. One public owner
  transaction commits the entire batch; errors/oversized results roll it back.
  Schema/version initialization and checks belong to the application and may
  share that atomic batch. Results are complete, not silently truncated.
- Limits: 256 KiB body, 1–32 input and translated statements, 16 KiB SQL and 128
  parameters per entry, 1,000 total result rows and 1 MiB encoded results. These
  are request/batch/result bounds, not a query CPU/scan-isolation guarantee; the
  current owner materializes results before result validation.
- Authorization is rechecked before transaction admission and after commit,
  outside owner transactions; catalog/fences are rechecked during execution and
  before commit. Post-commit denial suppresses the response without undoing
  admitted writes. `DatabaseOperationError.status` is exposed without provider
  bodies. Signals cancel unsent work; retirement rejects retained capabilities
  and obsolete results. Sent work is never automatically replayed and local
  cancellation does not prove remote rollback. Same-identity renewal stays usable.

Exact contract and limitations: `crates/storage-service/docs/api.md`; client/SDK
usage: `packages/storage-client/README.md`, `packages/sdk/README.md`. Canonical
`openapi/storage.json` and generated Rust/TS clients were regenerated together.

### Validation actually run

Passed (all commands bounded):

- `cargo test -p idp-unified --test owner_boundaries sql_http_tests --no-default-features`
  with the isolated manifest/shared build target: **12 passed**, one subprocess
  entry ignored by the outer harness (explicitly executed by the crash test).
  Real sign-in/exchange, authenticated HTTP/native persistence, concurrent/retry
  creation, restart, forced process interruption, namespace isolation,
  multi-Client continuity, generated scalar roundtrip, rollback/schema retry,
  bounds, learned Device denial, deletion/fences/unavailable copies, independent
  identity and post-commit response denial are covered.
- Same target with `residency_http_tests`: **7 passed**, preserving filesystem
  behavior. `cargo test -p storage-server --lib`: **95 passed**, one ignored;
  includes existing real SQL/KV/filesystem protocol and HTTPS owner regressions.
- `cargo hack test -p storage-service -p of-storage-client --feature-powerset --all-targets`:
  passed (Storage service **24 tests**; generated client combinations also pass).
- Storage client Vitest: **34 passed**. SDK Vitest: **52 passed**. Storage client
  ESM/browser build, SDK build and prerequisite client builds passed.
- Canonical OpenAPI export and pinned-generator Storage regeneration, including
  generated Rust check and TypeScript generation checks, passed. The initial
  generator download timed out; reusing the already-installed identical 7.25.0
  jar resolved it without changing generator versions or generated sources by hand.
- `cargo clippy --no-deps -p storage-service -p storage-server --lib -- -D warnings`:
  passed. `cargo fmt --all -- --check`, focused Biome check and `git diff --check`:
  passed.

Broader checks/limits, not claimed green:

- Full applicable `cargo hack test -p storage-service -p storage-server -p of-storage-client
  --feature-powerset --all-targets` timed out at **180 s** during the first server
  build. The separate no-feature server powerset slice fails with existing
  `E0432` at `crates/storage-server/src/lib.rs:51`: ungated `device_client` export
  while its module is network-gated. This unchanged baseline defect was not fixed.
- Strict all-target Clippy first fails in unchanged generated `of-idp-client`
  code (97 diagnostics). `--no-deps` all-target linting also finds existing server
  test lints (15 diagnostics, including unused read amount/range loops/manual
  async functions). Changed service/server libraries pass strict linting.
- `just crap --summary` ran with the isolated working directory and **180 s**
  bound; it timed out during instrumented workspace dependency compilation.
  **No completed CRAP score or full coverage result is claimed.**
- Native tests use isolated local roots and actual IdP/HTTP/SQL owner paths, not
  a deployed browser Installation. No GUI/OS-provider/trust qualification or
  cross-Node mesh/catalog convergence is claimed. The forced subprocess stop
  is not an exhaustive deterministic power-loss fault-injection campaign.

Ticket 02 remains the application cutover: choose the app key, initialize/check
its versioned schema transactionally, and implement its SQL collection/UX. Existing
filesystem resources and generic filesystem consumers remain untouched.

### Review correction and follow-up

[03: review fixes](03-review-fixes.md) supersedes the initial Answer's SELECT-only read classification and post-materialization-only bound description. Ready resources now reopen through the public existing-only SQL owner API; RETURNING requires read/write; retirement includes initializing intents; HTTP SQL is an explicitly restricted generic CRUD subset with pre-allocation native read admission and decoded-row accounting. The neighboring owner commit, exact regression results and remaining validation/qualification limits are recorded in ticket 03.
