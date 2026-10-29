# 03: Move Join onto the same host store

**What to build:** Unified Join uses the host-local SQL store and admission/workflow separation established for Create. Operators can join the exact Installation, recover and retry an interrupted operation, restart the completed Node offline, and explicitly reset it without separate Join file journals.

**Blocked by:** 02 — Move host configuration and Create setup into SQL.

**Status:** resolved

- [x] Reuse the existing host store, transaction handling, temporary credential handling, and durable retirement model from ticket 02. Do not introduce a second store, repository framework, or parallel operation history.
- [x] Persist the exact canonical Join operation, trusted peer, authenticated issuer, Node choice, stable allocated identities, credential references, attempts, and acknowledgements in host metadata. Preserve authenticated discovery and source approval semantics.
- [x] Bind durable operation evidence before acceptance or privileged provisioning. Exact retries use retained choices and identities; conflicting Create/Join requests or peer/issuer substitution refuse.
- [x] Authorized nonsecret recovery observes without resuming work. Preserve cancellation, interruption, approval/rejection handling, and the distinction between cancellation and rollback.
- [x] Commit Join completion and admission retirement together after required verification/publication. Remove the temporary code afterward; cleanup failure cannot restore authority. Completed-operation checks read host metadata rather than admission-owned historical payloads.
- [x] Completed restart requires only existing local state, verifies the retained endpoint binding, and works without contacting the original Join peer. Current readiness and normal authorization remain separate from setup completion.
- [x] Preserve attempted-before-external-effect and acknowledged-after-effect ordering. Provider, owner, and network failures never silently replace identities, repair missing evidence, or claim distributed rollback.
- [x] Update Join reset and native/shared recovery to use exact retained host references, preserve continuation evidence until secret cleanup succeeds, and leave shared or remote owner data untouched.
- [x] Remove superseded Join journals, duplicated intent storage, parsing/publication code, and remaining obsolete Unified admission workflow-history handling. Retain genuine TLS/configuration artifacts and owner-specific inventories that are outside the consolidation.
- [x] Unsupported legacy layouts remain intact with an actionable refusal; do not add migration, dual reads/writes, or serving wrappers.
- [x] Extend existing Join lifecycle and native/shared tests for approval, interruption, exact/conflicting retries, recovery, cancellation, atomic retirement, missing/corrupt metadata, offline completed reopen, and interrupted exact reset. Run affected Create and independent-owner admission regressions for shared changes.
- [x] If ticket 01 has landed, include focused combined lifecycle coverage for SQL-backed metadata and provider-backed endpoint identity, especially stable reopen and cleanup failure. Ticket 01 does not block this slice; if it lands later, its focused validation must exercise the then-current SQL-backed Create/Join paths. No separate integration ticket is needed.
- [x] Update existing operations, setup, security, and native documentation and remove obsolete guidance in this ticket. Record focused checks and remaining qualification limits; do not access real providers/OS trust or run a full cargo-hack feature-powerset or CRAP testing campaign.

## Comments

- Claimed by implement/host-join in isolated host-join worktree at integration 87532cde3. Agreed spec seams: Unified lifecycle/Join and native reset, injected providers only. High-priority security lifecycle rewrite; main risks are identity substitution, premature retirement and lost reset references.

## Answer

Implemented on `implement/host-join`, isolated `host-join` worktree, based on integration `87532cde3`. Before final commit, `GIT_EDITOR=true git merge implement/host-state-consolidation` reported **Already up to date** at that same integration tip. No main/remote changes or push.

- The same private `host.redb` now owns canonical Join binding, exact authenticated peer/issuer, Node choice, stable allocations, credential/provider references and production attempts/acknowledgements. Binding and initial attempts commit together before privileged effects; credentials and owner allocations are retained before acceptance. Exact retries/cancel/recovery use retained facts, never replacement identities. Recovery does not resume work.
- Verified Join publication is acknowledged before completion. The host retirement transaction requires acknowledged initialization and verified publication, and commits completion with admission retirement. Leftover temporary code has no authority, including after offline reopen. Current local readiness still verifies endpoint/owner binding independently of completion.
- Deleted Unified Join JSON publication/readback, duplicated Join admission/checkpoint storage, pending/attempted file writers/parsers and native manifest-reconstruction code. `HostedSetupOwners` has only the Unified implementation; its obsolete file workflow was removed, not retained behind a fallback. Independent-owner workers/admission/inventories are unchanged.
- Native/shared reset uses SQL-owned exact references and keeps SQL/reference evidence through failed provider deletion. Reference recovery validates existing SQL, never recreates missing workflow/admission; an exact operator backup is required. Missing/corrupt SQL and legacy journals fail closed intact. Domain/remote/shared owners retain their own data authority.
- Updated operations, setup-workers, security and native lifecycle documentation. High-priority security rewrite; principal risks were fresh-state misclassification, identity substitution, early retirement and lost reset references. Existing low-volume host transactions are reused; no extra database/framework/dependency, distributed transaction or legacy migration was introduced.

### TDD and focused evidence

The spec already confirms Unified lifecycle/Join and native/reset seams, plus the existing lower-level transaction fault seam. The first changed Join lifecycle test **failed** because Join had no SQL store; it passed after SQL persistence replaced the journal. The new Join transaction test **failed** because retirement accepted unverified Join state; it passed after completion/retirement validation. Existing provider/partial-state/native fixtures then exposed their obsolete file assumptions; these now observe logical SQL/reference state. Tests never compare binary database snapshots; operator DB bytes are used only for exact backup restoration, while assertions compare logical state. Literal corrupt artifacts use descriptive byte equality assertions.

Commands run from this worktree (`--test-threads=1` where shown). All providers were injected mocks; HTTPS fixtures use isolated native-root child environments, not live provider or OS trust installation.

| Command | Result |
| --- | --- |
| `cargo test -p unified-server --no-default-features --lib local_provisioning_tests::join_ -- --test-threads=1` | 2 passed: exact continuation/exclusion, wrong issuer, missing/corrupt SQL, no regeneration, restored stable endpoint. |
| `cargo test -p unified-server --no-default-features --lib old_one_two_and_four_relationship_create_and_join -- --test-threads=1` | 1 passed; malformed historical operation state refused logically unchanged. |
| `cargo test -p unified-server --no-default-features --test host_sql --test host_sql_interruption --test host_sql_transactions --test setup_host --test bootstrap_config` | 21 passed: Create regression, process ownership, retry/cancel/reopen, missing/corrupt/schema/code faults and transactional retirement. Transaction suite rerun after final completion guards: 3 passed. |
| `cargo test -p unified-server --no-default-features --lib endpoint_provider_tests:: -- --test-threads=1` | 9 passed, including SQL-backed Join stable reopen, missing material and exact reset/deletion-failure continuation; final rerun passed. |
| `cargo test -p unified-server --no-default-features --lib hosted_setup_integration_tests -- --test-threads=1` | 3 passed, 1 ignored child entry point; actual isolated HTTPS Create child launched/passed. SQL Join partial credentials, missing attempts, peer/issuer mismatch, missing operation/owners refuse before mutation. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_completion_and_offline_reopen -- --test-threads=1` | Wrapper and isolated HTTPS child passed. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_interruption_preserves_private_host_intent -- --test-threads=1` | Wrapper/child passed, including final stable-allocation assertions across cancellation/restart/retry. |
| `cargo test -p unified-server --no-default-features --lib production_api_join_atomic_retirement_leftover_code_offline_reopen -- --test-threads=1` | Wrapper/child passed, including final rerun after removal of obsolete hosted file workflow. Replaces the impossible completed-but-unretired crash fixture with leftover-code denial after atomic retirement. |
| `cargo test -p unified-server --no-default-features --lib hosted_join_cancellation_exact_retry_and_offline_reopen -- --test-threads=1` | Wrapper/child passed; source approval/rejection, cancellation, exact continuation and offline current-readiness checks. |
| `cargo test -p unified-server --no-default-features --lib production_setup_status_tracks_current_local_revocation -- --test-threads=1` | Wrapper/child passed. |
| `cargo test -p lidp --lib setup_reset -- --test-threads=1` | 6 passed; retained Join references, failed deletion, missing SQL refusal and explicit exact operator restore. |
| `cargo test -p lidp --lib native_host::tests -- --test-threads=1` | 14 passed. |
| `cargo test -p lidp --lib native_shared_create_reopen_and_provider_safe_reset -- --test-threads=1` | Wrapper and isolated child passed. |
| `cargo test -p api --no-default-features setup_admission` | 15 passed; independent admission regression. |
| `cargo test -p idp-server --no-default-features --lib hosted_setup:: -- --test-threads=1` | 7 passed, 1 ignored generation entry point; final rerun passed after deleting obsolete file handling. |
| `cargo clippy -p unified-server -p idp-server --no-default-features --features unified-server/cli --lib --no-deps -- -D warnings` | Passed, including changed CLI feature gate. |
| `rustfmt --check --edition 2024 --config skip_children=true <git-diff changed Rust files>`; `git diff --check` | Passed. |

### Limits and intermediate failures

- Initial cold lifecycle build, two cold native build invocations and a cold independent-check chain reached 240-second bounds. A mixed finalization/Create chain passed the Join child but timed out waiting for the native build lock before Create checks; the separate final Create command passed. The individual focused checks above subsequently completed; bounds were not increased.
- `cargo test -p unified-server --no-default-features --lib local_provisioning_tests:: -- --test-threads=1 --skip https_bootstrap_tests` reached its 240-second bound. Its two obsolete Join provider fixtures failed before conversion; final focused provider tests pass. The entire broader local suite is **not** claimed passing.
- Intermediate converted fixtures failed on missing SQL admission creation evidence and native restore imports; corrected, with the final hosted/native suites above passing. An intermediate trait-signature compile error after removal of the last file fallback was corrected and revalidated.
- Three existing shared-support dead-code warnings remain (`seed_permissions`, `infrastructure_token`, `run_lifecycle_http_checks`). Lint is scoped with `--no-deps`, not generated-client/dependency qualification.
- No full cargo-hack/feature-powerset, CRAP, full test suite, live provider/reboot, real OS trust, WAN or production-platform qualification. No known functional gap remains in the exercised ticket seams; mock persistence is not platform persistence certification.

### Combined review follow-up

[Ticket 04](04-combined-review-fixes.md) closes the verified combined-review gaps and supersedes the corresponding earlier safety/fixture claims: retained Create SQL prevents empty reset after directory loss; omitted Join endpoint references refuse; stopped recovery includes authoritative SQL; Join no longer caches workflow state or duplicates canonical peer/Node choices. Focused red/green results and exact final commands/limits are recorded there. This does not expand platform or full-matrix qualification.
