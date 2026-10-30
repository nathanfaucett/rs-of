# 02: Switch the password manager to automatically provisioned SQL storage

**What to build:** A User signs in and immediately manages secrets in the database explicitly provisioned by the password-manager application. Load, save, delete, Refresh, and reopen-after-reload work without filesystem/database selection or manual creation. Recovery and session teardown retain existing data-loss and confidentiality safeguards.

**Blocked by:** 01 — Provide explicit authenticated SQL database access.

Status: resolved

- [x] Put default database resolution, application-chosen logical identity, explicit creation, and schema initialization/checks in an application-owned startup module. Use the generic identity/lifecycle contract from ticket 01; neither the SDK nor collection adapter chooses or creates the default.
- [x] Initialize/check schema repeatably and transactionally, including its version state. Use one secrets table containing a secret ID and serialized validated record. Keep ordering in the existing TanStack live query; do not normalize nested fields or add speculative SQL indexes/filtering.
- [x] Replace filesystem persistence with a focused SQL collection implementation receiving an already resolved database and narrow structural Storage SQL interface. Keep the official TanStack Svelte binding, collection mutations, optimism, authoritative reconciliation, Refresh, and cleanup. Do not build a generic SQL collection framework or couple the adapter to the concrete SDK.
- [x] Preserve Secret schema validation, favorite/name ordering, additional URIs and match modes, custom fields, unedited notes, timestamps, and existing history. Changing the password appends its previous value to history; validation rejects invalid drafts before writes.
- [x] Deliver load/save/delete/Refresh together; remove manual filesystem selection/open/create controls and obsolete app-specific filesystem wiring, fixtures, and documentation. Preserve generic filesystem capabilities and other consumers; do not retain compatibility wrappers or a second write path.
- [x] Distinguish preparing, loading, empty, ready, malformed, unavailable, deleted, and denied states. Offer explicit safe provisioning retry without silently resurrecting a deleted database or replacing inaccessible acknowledged data with an empty database.
- [x] Report mutation success only after persistence and authoritative reconciliation. Reconcile rejected writes, retain editable drafts where safe, and identify uncertain outcomes without blind replay or false success. Failed reconciliation hides untrusted records and requires explicit recovery.
- [x] Refresh reloads external additions/updates/deletions, closes stale drafts on success, and cannot conflict with active writes. Failed or malformed snapshots never publish a partial or falsely empty vault. No polling, automatic reconnect, or offline mutation outbox.
- [x] Immediately clear records/drafts and dispose dependent queries/collections on sign-out, identity change, route teardown, or SDK retirement. Ignore late provisioning/read/write/Refresh outcomes. Preserve the current editor through same-identity renewal; distinguish Storage failures from terminal IdP failures.
- [x] Reuse mounted application workflow tests with the real SDK, OIDC/Storage clients, collection, and official binding against controlled transport. Cover first sign-in, automatic provisioning, ready-empty state, returning visits/reload, schema checks, explicit provisioning retry, CRUD/metadata/history/order, external changes/Refresh, malformed records, rejected/uncertain writes, and session changes during pending operations.
- [x] Document that old filesystem records remain untouched and are not imported or used as fallback. Do not add implicit migration, dual-write, or resource deletion. Keep Device selection and replication permissions separate from app provisioning.
- [x] Document that SQL does not add end-to-end encryption, secure erasure, production vault qualification, or concurrent-edit protection. Keep decrypted secrets out of browser persistence and logs; use non-sensitive test records only.
- [x] Run focused tests, type checks, production build, and applicable formatting/lint checks. Qualify actual IdP exchange, automatic creation, SQL CRUD, reload, Node restart, denial, and recovery when a running Installation is available; explicitly report unavailable manual qualification. Test replication separately only if claimed.

## Comments

- Claimed on `implement/password-manager-sql-02` in the requested isolated worktree; confirmed exact base `7ba7d0564159bc26ed2f5cca44e1d8f258878629`, initially also the integration tip.
- Complexity: moderate (startup plus one consumer-specific collection). Risk: high for confidentiality and falsely successful/replayed writes; fail-closed lifecycle/reconciliation are release gates. Priority: complete the end-to-end cutover, not a read-only intermediate.
- The spec/user-confirmed test seam is mounted application behavior using real SDK/OIDC/Storage clients, TanStack collection and official Svelte binding against controlled transport; native authenticated HTTP independently qualifies the application SQL request sequence.

## Answer

Implemented the complete application cutover on `implement/password-manager-sql-02`, based exactly on `7ba7d0564159bc26ed2f5cca44e1d8f258878629`. No implementation scope remains from this ticket; unavailable deployed qualification and broader-check limits are explicitly recorded below.

### Delivered

- `apps/password-manager/src/lib/storage/startup.ts` owns the permanent `password-manager:default` key, explicit ensure, validated resource identity, empty-catalog schema initialization, and strict version checks. The schema/version batch is atomic and repeatable; existing incomplete/incompatible version state is never reset or migrated. Retry repeats the same key, never an unkeyed create or replacement resource.
- `src/lib/collections/secrets/collection.ts` is app-owned SQL persistence with a narrow structural execute capability and an already resolved database. It retains TanStack mutations, official Svelte live queries, optimistic settlement, complete validated snapshots, authoritative reconciliation, explicit Refresh and terminal cleanup. It has no SDK/provisioning dependency. Parameterized statements carry UUID IDs and JSON text; app-generated UUIDv7 IDs satisfy the public owner's row identity requirements.
- The mounted page prepares automatically and distinguishes preparing/loading/empty/ready/errors. Deleted, fenced, unavailable, malformed and denied resources never become an empty replacement. IdP retirement synchronously clears the editor/query/collection; late results cannot enter replacement contexts. Ordinary same-identity renewal retains the editor. Storage errors do not indiscriminately end IdP authentication.
- Full CRUD, required-field checks, favorite/name order, additional URI/match/custom-field/notes/timestamp/history preservation, previous-password history, Refresh and safe drafts are retained. Success waits for persistence/reconciliation. Pre-commit rejections reconcile without success; ambiguous/post-commit-denial/timeout outcomes reconcile without replay, and close unsafe drafts. Failed reconciliation hides records and requires explicit recovery.
- Removed the app's filesystem selection/create/open flow, filesystem collection dependency/aliases, obsolete filesystem fixtures/tests and unused filesystem utility. Generic Storage filesystem capabilities and generic filesystem collection remain unchanged; their regression tests pass.
- App/SDK/collection READMEs describe the cutover and current ownership. Old filesystem data is untouched, with no import/fallback/dual-write/deletion. No encryption, secure-erasure, concurrent-edit protection, production-vault or replication qualification is claimed. Device selection remains separate. Decrypted secrets remain memory-only and failures/logging are redacted.

### Validation actually run

All commands used the isolated manifest/worktree; Rust compilation reused the existing shared target cache. No neighboring repository source was edited.

Passed:

- `pnpm --filter password-manager test`: **65 tests**, including **61 mounted workflow cases** with real SDK/OIDC/Storage clients, SQL collection and official TanStack binding. Initial automatic-opening and malformed-resource tests were observed red before implementation/fix. Cases include fresh callback/restore/reload, schema retry/version rejection, lifecycle/error/recovery, metadata/history/order, rejected/uncertain writes, held persistence/reconciliation, 30-second deadline, sign-out/identity/teardown races and late completion suppression, same-identity renewal, and no secret browser persistence/logs.
- `pnpm --filter password-manager exec node --test svelte.config.test.mjs`: **2 passed**.
- `pnpm --filter @of/sdk test`: **52 passed**; Storage client: **34 passed**; generic filesystem collection: **59 passed**.
- `cargo test -p idp-unified --test owner_boundaries sql_http_tests --no-default-features`: **13 passed**, one subprocess entry ignored by the outer harness and explicitly run by the crash test. The new native test exercises the application's actual schema/CRUD request sequence, repeat initialization, authenticated HTTP and persistence across a local runtime restart. Existing tests independently cover real IdP exchange, namespace isolation/multi-Client continuity, rollback, response denial, identity/retry/deletion/fences/unavailable content and bounds.
- Production app build; Svelte type check (**0 errors/warnings**); prerequisite client/SDK/headless builds; changed TypeScript/config source Biome lint/format; Svelte autofixer (**no issues/suggestions** for page/editor/workflow); `cargo fmt --all -- --check`; `git diff --check`.
- Svelte lint runs skip only Biome's template-unaware unused-import/variable rules; `svelte-check` and autofixer independently validate the components. Ordinary TypeScript lint has no warnings.

Limits, not claimed green:

- `cargo hack test -p idp-unified --feature-powerset --all-targets` timed out at **180 seconds**, compiling the first all-features combination (1/10). No completed powerset result is claimed.
- `just crap --summary` timed out at **180 seconds** compiling instrumented workspace dependencies (including desktop GTK/WebKit). No completed CRAP score/coverage result is claimed. It measures Rust, not this TypeScript application.
- Strict `cargo clippy -p idp-unified --test owner_boundaries --no-default-features --no-deps -- -D warnings` finds **6 existing diagnostics** in unchanged `owner_composition_tests/{review_transfer_restart_tests,create,operation,remote_setup}.rs` (collapsible if), `tests/support/identity.rs` (needless borrow), and `node_bootstrap_interruption_tests.rs` (drop non-Drop). None are in the added native SQL workflow test; unrelated code was not changed to silence them.
- No configured deployed Installation/browser credentials were supplied. Actual deployed browser sign-in/CRUD/reload, Node restart, denial/recovery and browser/OS/provider trust qualification remain manual using the README checklist. Native fixture restart is not that deployment qualification. No cross-Node mesh/catalog convergence, secure erasure, global write serialization or concurrent-edit protection is inferred.

### Review correction and follow-up

[03: review fixes](03-review-fixes.md) makes each collection mutation and its authoritative full snapshot SELECT one atomic read/write SQL batch. Returned snapshots are validated before publication; snapshot row/byte rejection rolls back the mutation and leaves a repairable readable vault. Rejected/uncertain responses still reconcile without replay or false success. Mounted row/aggregate-byte boundary cases and independent native aggregate-byte rollback are covered; actual validation and qualification limits are in ticket 03.
