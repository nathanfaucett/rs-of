# 02: Move host configuration and Create setup into SQL

**What to build:** A fresh Unified Host reaches awaiting-choice setup and completes Create using one private host-local `ofdb-sql` database for configuration and workflow state. Its temporary setup-code file contains credential material only. Exact retries, authorized recovery, cancellation, completion, restart, and reset retain their current guarantees without Create's overlapping file journals.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Reuse the existing SQL engine in a dedicated host-local database opened before domain provisioning. Do not add SQLite, expose this database through domain SQL/replication, or merge owner database lifecycles.
- [x] Keep a minimal versioned representation of host configuration and Create operation binding, stable allocated identities, credential/provider references, attempt evidence, acknowledgements, completion, and retirement. Preserve owner-specific domain authority and protected inventories.
- [x] Fresh host creation, awaiting-choice restart, and exclusive process ownership work end to end. Missing/corrupt retained databases, unsupported schema, and unsupported legacy layouts refuse intact before any implicit database creation, credential generation, or repair.
- [x] Separate setup credential handling from workflow history for Unified Create. Preserve existing header authorization, protected local code retrieval, and authorized nonsecret intent recovery. Do not add code expiry, rotation, single-use semantics, or a new sign-in flow.
- [x] Bind the exact canonical Create operation and required initial attempt evidence in a host transaction before privileged mutation or acceptance. Reject conflicting requests and retain identities and choices across exact retries and interruptions.
- [x] Preserve cancellation/finalization serialization, current owner readiness checks, and completed Create retry correlation followed by normal Initial Administrator authentication. Cancellation must not claim rollback.
- [x] Commit Create completion and admission retirement together after required owner publication and verification. Delete the temporary code afterward; a leftover code after cleanup failure or restart never authorizes setup. Missing live code refuses without regeneration, while retired reopen requires no code.
- [x] Keep external effects outside host write transactions: commit attempted evidence, perform owner/provider work, then commit acknowledgement. Do not claim atomicity across owner stores or providers.
- [x] Update Create reset and native/shared reference recovery to use authoritative SQL metadata. Retain exact references through interrupted cleanup and do not delete shared owner data.
- [x] Delete replaced host configuration and Create journal parsing/publication code in this slice. Leave Join-specific persistence for ticket 03, with existing Join behavior still working; do not add legacy-layout adapters or parallel authoritative copies of converted facts. Ticket 03 will reuse the new host store and admission handling.
- [x] Preserve independent-owner admission behavior when shared code changes. Do not move independent-owner journals or domain databases into Unified storage.
- [x] Extend existing host lifecycle tests for fresh Create, awaiting-choice reopen, exact/conflicting/concurrent requests, authorized recovery, cancellation, completion/retirement atomicity, leftover-code denial, completed reopen, missing/corrupt state, creation interruption, and reset. Use fault seams only where required to prove an observable safety guarantee.
- [x] This ticket does not depend on provider-backed endpoint persistence. Use whichever endpoint implementation is authoritative when it lands, without a new compatibility abstraction.
- [x] Update affected setup, security, operations, and native documentation. Run focused lifecycle/admission regressions and necessary affected compile/format/lint checks; record results and limits. No full cargo-hack feature-powerset run or CRAP testing is required.

## Comments

- Claimed by implement/host-sql in isolated host-sql worktree; base confirmed at 2f3f57c8f (integration tip). Own SQL/config/Create/admission; preserve Join for ticket 03 and endpoint implementation for ticket 01.


## Answer

Implemented on `implement/host-sql`, continuing checkpoint `63a96f6c3`. Merge `53cbddfc3` incorporates integration `e36ed4ac2` (ticket 01 endpoint provider). All previously resolved merge content and the user's logical-configuration/binary-assertion test fixes were preserved.

### Ownership and safety

- The dedicated private `host.redb` uses existing `ofdb-sql`, before domain provisioning, with a versioned host record and transactional updates. Configuration, bootstrap references, canonical Create operation, stable allocations, owner attempts/acknowledgements, completion and retirement are SQL-owned. No new SQL engine was introduced (the existing `ofdb-sql` crate is reused), no remote route/replication was introduced, and domain databases/inventories remain owner-owned.
- Unified admission stores only the temporary code marker/material. Durable SQL binding and allocation precede accepted Create/owner mutation; completion and retirement commit together after owner publication and current verification. External provider/owner work stays outside host transactions. Leftover code cannot authorize live or restarted setup, and completed exact retries use normal administrator authentication.
- Fresh-root checks and lifecycle locks remain. Missing, empty, corrupt or unsupported-schema SQL, missing live code, interrupted code creation and legacy file layouts refuse without replacement authority. Superseded configuration/Create/credential JSON publication was removed rather than adapted. Existing lock directories and TLS files are not workflow mirrors.
- Reset captures SQL-owned exact references, preserves them through provider failure and deletes protected material before data. Reference recovery never reconstructs missing SQL history/admission; an exact operator database backup is required. Native tests now verify this boundary and compare logical settings/public references rather than redb physical bytes.
- Endpoint integration fixes retain standalone Join's existing journal behavior, require the provisioning directory on endpoint reopen, and reject missing provider-reference fields. No Join persistence consolidation or ticket 03 implementation was undertaken. Independent admission/provider behavior remains covered.

Continuation assessment: high-priority lifecycle/security change; greatest risks were accidental fresh creation, identity replacement, premature retirement and lost reset references. The small SQL store removes publication machinery; fault tests target those risks without introducing distributed transactions or compatibility layers.

### Validation evidence

Commands run from the isolated `host-sql` worktree. Providers were injected mocks; HTTPS wrappers used isolated native-root environments, never live providers or OS trust installation.

| Command | Observed result |
| --- | --- |
| `cargo test -p unified-server --no-default-features --test bootstrap_config --test setup_host --test host_sql --test host_sql_interruption --test host_sql_transactions` | Initial post-merge: 17 passed. Final expanded run: **20 passed** (10 + 6 + 1 + 1 + 2). Covers fresh/reopen, process exclusion, missing/empty/corrupt/unsupported/legacy state, code interruption, exact/conflicting/concurrent Create, authorized recovery, cancellation, stable allocations, atomic completion/retirement, failed cleanup/leftover-code denial, completed authentication and SQL reset references. |
| `cargo test -p unified-server --no-default-features --lib local_provisioning_tests:: -- --test-threads=1 --skip https_bootstrap_tests` | **32 passed**, including all nine provider lifecycle/fault tests, owner-readiness/loss checks and stopped same-Node recovery. |
| `cargo test -p unified-server --no-default-features --lib setup_host_tests -- --test-threads=1` | **5 passed**, one ignored child entry point; isolated HTTPS children were explicitly launched and passed. |
| `cargo test -p unified-server --no-default-features --lib hosted_setup_integration_tests -- --test-threads=1` | **3 passed**, one ignored child entry point; actual HTTPS Create child passed. Includes partial credential faults, retired Join refusal and completed Create/leftover-code behavior. |
| `cargo test -p unified-server --no-default-features --lib config::tests -- --test-threads=1` | **5 passed**. |
| `cargo test -p unified-server --no-default-features --features cli --lib cli::tests -- --test-threads=1` | **5 passed**, including retained UI-origin/configuration behavior. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_completion_and_offline_reopen -- --test-threads=1` | **1 wrapper and its isolated child passed**. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_interruption_preserves_private_host_intent -- --test-threads=1` | **1 wrapper and its isolated child passed**. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_complete_unretired_finalization -- --test-threads=1` | **1 wrapper and its isolated child passed**. Join's existing separate completion/retirement checkpoint remains supported. |
| `cargo test -p unified-server --no-default-features --lib production_setup_status_tracks_current_local_revocation -- --test-threads=1` | **1 wrapper and its isolated child passed**. |
| `cargo test -p lidp --lib setup_reset -- --test-threads=1` | **6 passed**, including interrupted deletion and lost database refusal/exact operator restore. |
| `cargo test -p lidp --lib native_host::tests -- --test-threads=1` | **14 passed**, including corruption classification, missing state, exclusive ownership and stopped recovery. |
| `cargo test -p lidp --lib native_shared_create_reopen_and_provider_safe_reset -- --test-threads=1` | **1 wrapper and its isolated child passed**. |
| `cargo test -p api --no-default-features setup_admission` | **15 passed**, preserving independent-owner admission. |
| `cargo test -p idp-service --lib provider_isolation_tests -- --test-threads=1` | **3 passed**, including independent Device provider isolation. |
| `cargo clippy -p unified-server --no-default-features --lib --no-deps -- -D warnings` | **Passed** after fixing the changed Create admission condition. |
| `rustfmt --check --edition 2024 --config skip_children=true <changed Rust files>` | **Passed**; exact changed-file set obtained from `git diff --name-only 3b75fc9db -- '*.rs'`. |
| `git diff --check` | **Passed**. |

### Red/green observations and limits

Existing regression fixtures first failed on removed JSON journals. Their Create/configuration fault injection now targets SQL; Join retains its journal. Provider regressions exposed standalone Join incorrectly requiring SQL, reopen bypassing a lost provisioning directory, and missing endpoint-reference fields being accepted; all nine provider checks pass after the corresponding fixes. Desktop corruption first reported Retry instead of Repair; retained database-open corruption now reports invalid data and all 14 desktop tests pass. The completed Create fixture now tests SQL retirement overriding leftover code rather than manufacturing the deleted completion-before-retirement window.

- A cold test-listing chain timed out at 180 seconds during `idp-server` compilation. Native cold-build chains also reached 240-second bounds; the individual native suites above subsequently completed. An initial independent-provider build chain reached its bound; its final separate run passed.
- `cargo test -p unified-server --no-default-features --lib local_provisioning_tests::https_bootstrap_tests -- --test-threads=1` reached its **240-second bound**, so the whole HTTPS suite is **not** claimed passing. Its first hosted cancellation/exact-retry child passed; legacy admission fixture failures were then corrected, and the three affected Join wrappers plus current-readiness wrapper passed individually as listed above. No timeout bound was widened.
- `cargo clippy -p unified-server --no-default-features --lib -- -D warnings` is blocked by existing generated-client diagnostics: `of-management-client` (66), `of-storage-client` (48), `of-device-client` (73), `of-idp-client` (97), including enum naming, needless returns and empty doc comments. Generated clients were not changed. The changed host crate passes the no-dependency lint run above.
- Shared test support emits three pre-existing dead-code warnings (`seed_permissions`, `infrastructure_token`, `run_lifecycle_http_checks`). They were not suppressed or removed.
- No full feature-powerset, `just crap`, full-node/SDK qualification, real provider/reboot qualification, live trust operations or ticket 03 work was performed. These are validation/scope limits, not claims of production-platform certification. No known ticket 02 functional gap remains in the exercised seams.

### Combined review follow-up

[Ticket 04](04-combined-review-fixes.md) closes the verified combined-review gaps and supersedes the corresponding earlier safety/fixture claims: retained Create SQL prevents empty reset after directory loss; omitted Join endpoint references refuse; stopped recovery includes authoritative SQL; Join no longer caches workflow state or duplicates canonical peer/Node choices. Focused red/green results and exact final commands/limits are recorded there. This does not expand platform or full-matrix qualification.
