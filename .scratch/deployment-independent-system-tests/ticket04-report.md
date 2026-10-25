# Ticket04 report — authorization, learned denial and interrupted Join

## Result and ownership

**Resolved for the bounded applicable migrations in the documented Linux
same-machine mock-provider layouts.** Final post-integration validation passed
**four actual-process executions**: both layouts for permission/resource/native
existing-connection denial, and both for interrupted/rejected/exact-retry Join,
learned consumed/revoked refresh history and source-offline reopen. This does not
claim migration or qualification of live post-Join peer reconciliation.

- Worktree: `/home/nathan/code/rust/aicacia/of-system-tests-04`.
- Branch: `refactor/system-tests-04`, base **`af62ed7bc`**.
- Read spec, issue04, coverage inventory, ticket03 report, system fixture README,
  issue protocol and domain map. Claimed ticket04; loaded **tdd** and **ponytail**.
  Used the previously approved seams without adding a new seam interview.
- Split before implementation into sequential scenario-family children:
  **04a permission/resource denial**, **04b learned/native denial**,
  **04c rejection/interrupted exact retry**. Each has its own result in `issues/`.
- Implementation commits: **`e95ec02e2`**, **`d4c24ed14`**, **`769c1c88c`**.
- After implementation, ran `git -C ../of-system-tests-04 merge --no-edit
  refactor/deployment-independent-system-tests` with `GIT_EDITOR=true`.
  Result: **Already up to date**; integration tip **`af62ed7bc`**. The final four
  executions, lint/format checks and this report follow that merge check.
- Explicit owned paths only were staged/committed. No edits to ticket01's external
  files or **`coverage.md`**; this report supplies ticket04's additive inventory.
  Main's unrelated staged AD setup-simplification was not touched/stashed/staged.
  No main/integration merge and no push.

Complexity/risk was bounded by scenario-family sequencing rather than creating a
framework. The main risks are test runtime, real N0 network discovery, multi-owner
cancellation differences and accidentally treating denial as outage/rollback.
There is **no new production feature, visibility, hook, endpoint, security policy,
bootstrap SQL, credential copying or dependency-boundary exception**. The approved
Management socket correction is consumed as already merged, not changed here.

## Shared behavior and concrete clients

`tests/system/authorization.rs` shares one assertion body across actual Unified
and independent local-owner processes:

- Ordinary generated IdP login and User info; generated Management evaluation for
  **the specific public Device ID**, exact request echo and exact actor,
  `serviceSubject` and login `serviceClientId` audit identity.
- Another User subject and another valid Application scope are refused with 403.
  An IdP session is not Management authority. A well-formed ungranted Device
  restriction-read request returns **allowed:false**, echoes the exact request
  and actor, and the actual Device operation returns 403. A distinct granted
  Device listing still succeeds.
- Generated Storage creation/read, anonymous refusal (existing common assertion),
  read-only delete refusal, wrong-audience refusal and a **database-kind** token
  refused on the **filesystem** resource. The authorized read after denial returns
  the unchanged resource; no private row is asserted.
- Existing `tokio-tungstenite` plus typed `storage-model` requests write/read actual
  bytes over the process's Storage WebSocket. Independent WSS explicitly verifies
  its fixture CA with Rustls. Generated IdP User deletion then causes a subsequent
  write on the **same retained connection** to error/close, a new connection to
  return 401, and catalog access to return 401. Another owner's health still
  succeeds. This is subsequent-operation authorization, **not** an injected
  response-time fence or an assertion that denied responses imply rollback.

`tests/system/recovery.rs` supplies concrete setup differences to the existing
shared Join body in `create.rs`; `JoinAdmission` has only the two actual generated
setup contracts, not a generic host trait/framework:

- Source creates three distinct ordinary refresh grants, consumes one and revokes
  another using generated clients. Source denial must be `invalid_grant`.
- Real production Join history clients (native Unified / independent WebSocket
  history exchange) import into independently owned target state. Target refresh
  denies consumed/revoked grants; an **unconsumed imported grant succeeds**, rotates
  to a different grant, then denies replay. This positive control distinguishes
  learned denial from missing history or an unavailable owner.
- Learned denial and target-local replay denial persist after all source processes
  stop and the target reopens. Source login Client ID is used for learned grants;
  no target password verifier or source credential is copied to make them work.
- Exact active Join resubmission preserves the pending request. Cancellation with
  absent host/worker capability returns 401. Authorized cancellation is only a
  signal: observe non-readiness, actually stop/reap the processes, relaunch retained
  commands, and compare protected recovered intents. Source pending release and
  stale-offer approval denial are separately observed with bounded waits.
- Exact retry preserves Node, public signer and endpoint request. Anonymous Device
  rejection is denied; ordinary authorized rejection leaves the target non-ready.
  Rejected offers cannot be approved. Another exact retry retains the request and
  completes only through ordinary approval, then exercises the existing joined
  owner/resource and source-offline retained behavior.
- Independent Device rejection does **not** cause IdP/Storage waiting workers to
  become `unfinished` automatically. Cancel the remaining active workers and
  reopen retained processes before retry. Independent stale-offer refusal is
  documented **409**; Unified is **404**. These differences remain explicit.

`tests/system_create.rs` adds two ignored opt-in denial entries and strengthens the
existing two Join entries, avoiding a duplicate source/target setup per subcase.
README documents nine available entries and the four migrated executions.

All HTTP supported operations use generated owner clients/models. Only native
protocol framing uses its existing typed transport. Existing private provider,
OS-trust substitution, bounded requests/polls, explicit origins, private roots,
16 KiB drained child logs and kill/reap teardown are reused. Assertion failure
messages never intentionally print bearer/refresh tokens; token comparisons use
boolean assertions rather than value-dumping equality assertions.

Four already-used workspace/dependency clients are added **dev-only** at the root:
`futures-util`, `rustls`, `storage-model`, `tokio-tungstenite`. Final lockfile adds
only root dependency edges, **no package/version additions**. Rustls is needed to
supply explicit per-fixture WSS roots without unsafe global trust changes.

## Inventory reconciliation and exact dispositions

Ticket03 already covers common fresh Create, correlated readiness, authorized
resource use, retained restart and approved Join/source-offline reopen. Ticket04
adds the missing negative/recovery assertions, not a deployment cross-product.
The following is additive to ticket01's untouched `coverage.md`.

| Existing body / assertion family | Disposition and concrete reason |
| --- | --- |
| `api_join_tests::run`, Interrupted branch: exact public pending request after reopen, ordinary rejection, non-ready rejection, new exact retry retaining request | **Replaced after both shared layouts passed.** Removed the 35-line public rejection/retry block. Shared recovery compares the entire public request, rejects through generated Device clients, checks non-readiness/stale offer refusal, then exact-retries/approves in both layouts. |
| `https_bootstrap_tests::production_api_join_interrupted_retry_and_rejection` | **Narrowed/renamed** to `production_api_join_interruption_preserves_private_host_intent`. Still checks private `BootstrapConfig` Join purpose, retained issuer/peer, TLS/admission, exclusive host guard and internal retained-offer release. Only its replaced public rejection/request-comparison block is removed, not the private case or fixture. |
| Remaining `api_join_tests::run` common wire assertions, Offline, Readiness and Finalization cases | **Retained.** Exact raw 202/header/body behavior, forged/unknown fields, hostile Origin, capability retirement, current-readiness effects and simulated crash before admission retirement are different contracts. The finalization case restores pre-retirement admission bytes; shared fixtures must not simulate that via private writes. |
| `full_node_tests::two_completed_full_hosts_reconcile_consumption_and_revocation` | **Partial shared replacement, retain whole mixed test.** Initial-history consumed/revoked denial, usable unconsumed grant and local replay are now shared, including source-offline retention. Its *post-Join* reconnect uses the live runtime endpoint and `SecurityProtocol::synchronize` with local owner databases/member context. There is no public process synchronization trigger. Do not read keys/DBs or add a production trigger; retain bidirectional live reconciliation and local race assertions here. |
| `full_node_tests::two_completed_full_hosts_learn_restrictions_on_retained_sockets` | **Retained private/owner scenario, not falsely counted as shared synchronization.** Privately grants reserved Installation restriction permissions, partitions two live security owners, mutates remote Device restriction, reconnects owner histories, denies existing sockets, restores restriction and verifies unchanged bytes. Public Management role routes deliberately refuse the nil Installation namespace; no public way exists to seed these precise grants or initiate post-Join reconciliation. Shared native User-deletion denial is additional coverage, not a substitute for learned Device restriction. |
| `full_node_tests::two_completed_full_hosts_join_partition_reopen_and_learn_denial` and `full_node_probe::single_full_host_refresh_replay_and_restriction_put` | **Retained mixed/diagnostic.** Independent secret inventories, no copied private material, JWKS signer contents, per-host local refresh race and private policy grants are not replaced by black-box retained endpoint/resource/grant assertions. The probe is not promoted to qualification. |
| `permission_http_tests::{normal_permission_http_administration,normal_identity_and_infrastructure_lifecycle}`, `run_permission_http_checks`, and `shared_identity_management_writes_change_idp_authorization` | **Partly shared; owner-specific remainder retained.** Same-actor/audience/exact decision denial now shared. Existing fixture manually seeds multiple Users, per-Application administrators, confidential trusted/untrusted evaluator relationships and private Installation grants. Published IdP API does not expose arbitrary test User creation, and public Management role administration refuses Installation namespace. Client/key/infrastructure lifecycle and cross-actor service-delegation matrix remain owner tests; do not widen public APIs or copy fixture SQL into processes. |
| `runtime::builds_three_prefixed_services_with_one_endpoint` | **Retained mixed Unified wiring/security.** Injected owner handles, custom signer bindings/permission authorizer, ALPN routing and private context grant removal/re-addition qualify the composition. Public process addresses, actor/resource denial and Join are covered separately, not a replacement for those internal wiring assertions. |
| `native_api_tests::native_local_tokens_bound_api_actions_resource_kind_and_existing_socket_operations` | **Partly shared, retain owner fixture.** Filesystem kind/action/audience denial and existing-socket User deletion now have process coverage; the old test also checks both filesystem/database catalog matrices, exact Application namespace and private resource bytes after denial. It is not wholly replaced, so neither its test nor fixture is removed. |
| `native_api_tests::{native_local_token_read_response_denies_learned_revocation_before_transmission,native_local_token_authorized_write_survives_revoked_response_and_next_operation_denial,native_catalog_create_response_denies_learned_user_deletion_without_rollback,native_catalog_all_result_kinds_recheck_after_execution_and_retain_mutations}` | **Retained private fault injection.** Paused session/catalog gates intentionally mutate authority between execution and response; committed writes may remain while the response is denied. Process tests cannot promise that exact scheduling boundary and do not equate cancellation/denied acknowledgement with rollback. |
| `native_api_tests::independent_idp_service_loss_denies_response_and_api_without_cached_fallback` | **Retained layout-specific availability/fault case.** Intentionally loses the remote IdP at a paused response boundary; Unified cannot model that separately stopped owner through the same wiring. |
| `device_authority_tests::device_http_and_storage_share_isolated_authority_and_recheck_revocation` | **Retained owner authority test.** Direct isolated Device owner repository mutations and request-time rechecks are not replaced by public User deletion. No private Device engine accessor is exported to the shared fixture. |
| `security_sync_tests::authenticated_equal_peers_exchange_scoped_history_without_local_verifiers`, all `security_candidate_tests` and `security_history_tests` | **Retained protocol/storage/fault tests.** Scoped table manifests, signer/verifier exclusion, candidate corruption, conflicted bindings/tombstones, overwrite attacks and private admission fences require direct histories/databases and authenticated owner contexts. Native actual-process Join proves its public observable import result, not those formats/fault boundaries. |
| `node_join_bootstrap_tests::{authorized_node_bootstrap_imports_both_contexts_and_reopens_offline,owner_cutover_node_bootstrap_reopens_offline}` | **Retained private cutover/receipt recovery.** Constructs prepared owner bindings and mutates/inspects receipt and credential boundary state. Shared public Join/reopen does not replace that failure-injection contract. |
| `hosted_setup_integration_tests::{retired_join_reopen_refuses_mismatched_or_missing_intent_without_owner_mutation,partial_credentials_reopen_preserves_recovery_for_create_and_join}` plus local provisioning/TLS/publication checkpoint tests | **Retained private recovery.** Missing/mismatched retained intent, partial credential inventories, damaged CA/admission and publication checkpoints must remain existing-only fail-closed checks. Shared process interruption does not create corrupted private files or claim equivalent crash-stage coverage. |
| `owner_composition_tests::join::{production_independent_join_and_source_offline_reopen,production_independent_join_legacy_record_reset}`, reset helpers and `remote_identity_process_tests::independent_processes_share_remote_identity_and_consumer_reset_preserves_rows` | **Retained deployment-specific/mixed.** Remote identity attachment, legacy cleanup references, reset refusal and retained remote rows are explicit independent-owner persistence/reset contracts. They are not independently owned target Join history qualification. |
| Ticket01 external Create exact retry and raw wire tests; generated JS qualification | **Untouched.** Ticket01 owns those edits/validation. Common Create/retained readiness already qualified by ticket02/03 is reused, not duplicated. Generated Rust is the selected shared consumer; existing JS checks are preserved without multiplying SDK coverage. |

No whole remaining mixed test has assertion-equivalent replacements. The
`assert_offers_released` helper still has private retained-offer callers in HTTPS
bootstrap tests; it is not obsolete. Likewise the existing runtime owner accessors,
credential/fault stores and native fixtures still serve the retained tests above.
No new public hook was introduced to preserve them and none was deleted blindly.

## TDD and validation history

The existing test seam was approved in the spec. Each bounded slice first added
its consumer call and ran the focused target: missing `denial_scenario`, learned
grant helpers, retained socket helper, rejection/release helpers each produced
observed compiler-red results before the minimal concrete implementation. This
is test migration: **no production behavior was invented or patched to make the
new assertions pass**. Behavioral failures during fixture assembly were preserved
and corrected at the test-contract/lifecycle boundary:

1. The first two focused compilations hit **200s tool deadlines** rebuilding
   worktree-path dependencies (`-j2`). They were not test passes; a later 600s
   bounded run reached the intended missing-function compiler red.
2. Initial permission attempts tried public role administration for the reserved
   Installation namespace. Both layouts correctly refused it. Removed that
   unsupported fixture strategy; use an already-ungranted, well-formed action and
   retain private permission-mutation tests instead of weakening owner admission.
3. Restriction evaluation with `id:null` was a malformed action/target tuple and
   refused; corrected the test to discover the actual public Device ID. A mistaken
   `Device.id` compile error was corrected to use generated Device listing.
4. Independent rejection first timed out waiting for all workers to automatically
   become unfinished: IdP and Storage remained `joining`, Device was `unfinished`.
   Corrected the lifecycle fixture to cancel waiting workers explicitly and reopen
   retained processes. A first stale-offer assertion incorrectly expected Unified
   404; used independent's documented 409 instead. No production change.
5. The narrowed old private test first failed before the removed block at
   `SetupHost::open` with `Unavailable` (23.04s after build). Focused rerun passed
   (70.18s). Root cause is not established; do not claim a fixture change fixes this
   intermittent private reopen failure.
6. First strict Clippy found two needless borrows and late initialization; fixed
   locally. Final strict targeted Clippy passes. An initial WSS feature selection
   temporarily introduced a second webpki-roots version in an intermediate commit;
   switched to the existing native-roots TLS feature with explicit connector.
   Final lockfile has no new packages.

All Cargo commands use:

```text
--manifest-path ../of-system-tests-04/Cargo.toml
--target-dir target
-j2
```

They were invoked with working directory `/home/nathan/code/rust/aicacia/of`, so
`target` is the shared cache `/home/nathan/code/rust/aicacia/of/target`.
No Cargo process was active when initial validation started; this agent never ran
two Cargo commands concurrently. Used Cargo's normal cache lock and small focused
commands, not broad builds or a second cold target cache. No permanent background
server was started by terminal; the test fixture owns and reaps its actual services.

| Focused command (suffix after `cargo <subcommand>` and common options) | Observed result |
| --- | --- |
| `test -p idp-unified --test system_create permission_resource_denial -- --ignored --test-threads=1 --nocapture` | Both layouts passed 82.86s; with native retained connection checks passed 80.96s; final strengthened actor/target/kind version passed **81.35s** before final combined execution. |
| `test -p idp-unified --test system_create join_and_source_offline_reopen -- --ignored --test-threads=1 --nocapture` | Learned-grant slice **2 passed, 251.60s**; completed cancellation/rejection/retry **2 passed, 254.95s**; added missing-capability cancellation checks **2 passed, 259.86s**. |
| `test -p idp-unified --test system_create independent_join_and_source_offline_reopen -- --ignored --exact --nocapture` | Independent lifecycle correction **1 passed, 148.82s**. Earlier failed attempts described above are not counted as passes. |
| `test -p unified-server --lib production_api_join_interruption_preserves_private_host_intent -- --nocapture` | Narrowed private regression: first reopen failed, focused rerun **1 passed, 70.18s**, plus its isolated child passed. Compilation for this single filtered lib test took 8m40s on first run; not a broad lib-suite execution. |
| `build -p idp-unified --example system-test-service -p idp-server --bin identity-database` | Passed; repeated final prerequisite build **0.38s**. |
| `test -p idp-unified --test system_create -- --ignored --test-threads=1 --skip create_and_use --skip retained_restart --skip independent_local_owner_create_and_restart --nocapture` | **Final post-integration: 4 passed, 0 failed, 5 intentionally filtered unchanged entries; 400.61s**, 7.22s compile, 600s tool bound. Both Join/recovery and both permission/native-denial executions passed. |
| `clippy -p idp-unified --test system_create --no-deps -- -D warnings` | Final post-integration **passed**, 0.68s cached. |

Also passed final `cargo fmt --manifest-path ../of-system-tests-04/Cargo.toml
--package idp-unified --package unified-server --check`,
`python3 scripts/check_server_dependencies.py` with Python cwd explicitly set to
this worktree, and `git diff --check`.

## Limits and follow-up boundaries

Final qualification is the four selected actual-process cases and the focused
private-case rerun, **not** the full nine-entry lifecycle suite, full workspace,
platform matrix, graceful-shutdown certification or arbitrary network reliability.
Production N0 discovery/relay remains a real Unified prerequisite even on one
machine. OS credential storage/trust installation remain explicitly substituted
as in ticket03; production password costs, WAN/proxy/mTLS and other OSes are not
qualified. Existing bounded HTTP/poll deadlines and process isolation are retained;
native connection and response waits added here are bounded at 30s, unfinished
polling at 90s and source-offer release at 60s. Failures preserve private log roots.

The retained live post-Join reconciliation and reserved Installation-policy test
cases are **not shared**. They remain valuable, enabled tests with the concrete
reasons above; no new production synchronization control or policy mutation API is
authorized by this ticket. Do not infer that initial-history learned denial or
local User deletion proves live remote Device-restriction reconciliation.

No CRAP, coverage, feature-powerset, full-workspace test, main merge or push was run.
