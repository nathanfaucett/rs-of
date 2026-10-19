# Independent Create workers — API vertical slice

Date: 2026-10-19. Branch/worktree: `implement/bootstrap-independent`, `of/.scratch/worktrees/bootstrap-independent`. Continued the preserved uncommitted worker implementation from the failed transport session; no competing implementation or parent-worktree edits.

## Scope and provenance

Base integration: `bba9d49218fdc4cb0f91e0cc4df5e816c8dafa7b`. Preserved prerequisites: `1a6cebc65`, `d9fe9ea05`, `30ec2416f`; implementation parent: `30ec2416fab4dd649a1ae70f05841e99b18128f5`.

Changed-source provenance: SHA-256 of `git diff --cached --binary -- crates` against that parent is `214910f461e748f5b63669f394051e78224c725fc299ae0baeada605b7a91633`. This excludes this receipt and operational documentation, avoiding a self-referential digest. The final integration merge command reported `Already up to date`; integration tip remains `bba9d49218fdc4cb0f91e0cc4df5e816c8dafa7b`.

**Delivered: independent Create API workers, not the complete spec or tickets 02/03.** Independent Join, existing-app independent mutation wiring, generated independent setup SDK contracts, broader lifecycle reconciliation and deployment certification remain subsequent work. No ticket closure is asserted. Unified Join below is a regression, not independent Join qualification.

## Implemented ownership and lifecycle

- Each concrete owner worker checks that its setup host owns the configured domain root. Each independently admits the same exact canonical operation using its own protected host code. Wrong-owner codes, competing intents and wrong browser origins deny.
- `POST /setup/create` durably binds before returning 202. A bounded host-owned command channel and worker own provisioning independently of the HTTP response/browser. Recovery returns only the exact nonsecret operation; cancellation signals the exact active operation. Cancellation, registration and finalization share the lifecycle gate. Shutdown coordinates listener, worker and Management endpoint cleanup.
- IdP opens only its IdP owner, provisions the initial User/credential/root, independent token signer, public login Client and the confidential relationships actually consumed. Management opens only Management, creates explicit initial permissions and the one actual Management Device/endpoint with empty selections. Storage opens only its own resource roots and creates no resource, Device, signer or Node.
- All workers reuse their opened handles for seed and normal serving on their existing TLS listeners. Normal routers retain ordinary caller authentication, scope/audience checks and actor/action/target authorization; admission retirement is not a global normal-API enable flag.
- Consumer-local random secrets are protected by attempted-material fences and the existing secure provider. IdP sends an exact outstanding challenge only to its bound recipient over verified HTTPS; that recipient sends the secret directly to its bound IdP constructor. Codes are never forwarded. Public status/discovery/recovery never return relationship secrets, passwords or challenges. No verifier-import cryptography, copied endpoint key or cross-owner database open was added.
- Receipt evidence preserves exact registration retries without replacing consumer secrets. Challenges are transient, consumed before provisioning, and invalid after cancellation/restart/retirement. Complete-but-unretired restart reopens existing state and remains unfinished until authorized exact finalization. Missing attempted protected material refuses without regeneration.
- Completed IdP reopen verifies its retained signer, public login Client and confidential relationship metadata/root evidence locally, without peer discovery/import or reseeding. Management rechecks its retained approved endpoint and policy. Storage restores its retained service bindings and runtime without a separate serving command.

## Storage failure isolation and repair

The original independent HTTPS fixture reached all-owner ready and authenticated Management use, then the authenticated Storage catalog request hit the existing five-second client timeout. It was not an established deadlock: temporary nonsecret timings showed the first Storage identity/current-policy authorization taking about 3.6–3.9 seconds, followed by the mandatory response-time authorization recheck. No request or scenario timeout was increased.

Removed redundant current-state work rather than weakening checks:

1. Storage evaluates IdP identity and current Management owner policy concurrently, still requiring both and matching the subject to the current owner, on each authorization/recheck.
2. Management's selected-resources route resolves its approved endpoint/active owner once rather than immediately resolving the same Device/endpoint again. Caller authorization and endpoint resolution overlap, but the caller's authorization result is required before using/returning the endpoint result. Exact policy evaluation remains local and current.
3. IdP Principal construction reuses the active root already resolved in the same validation call instead of refetching that root and resolving it again. Public `find_principal` still performs its own active-root validation. Signer trust, signature, active Principal, client, audience and scope validation remain intact; no decision cache was added.
4. Preserved the earlier reqwest native-root correction for authenticated Management/Storage HTTPS consumers. Native verification and disabled redirects remain required.

The first concurrent Management implementation used `try_join!`, causing endpoint-unavailable errors to cancel caller introspection and replace the intended caller denial. Existing tests caught it (three failures with 503/error-ordering differences and mock-server BrokenPipe). Replaced it with `join!` followed by caller-result-first checking. All six policy regressions then passed. This is not an authorization bypass.

Temporary diagnostic logging was removed. The final independent fixture has no custom debug logger or per-request diagnostic prints. Existing shared HTTPS startup still prints only the listener and protected code retrieval path, never the code.

## Final validation results

Ubuntu Linux x86_64, offline/locked, one Rust build at a time, MAIN target directory, one build job, incremental disabled, 16MiB stack, selective profile below. Every Rust terminal invocation bounded at 200000ms. The independent parent fixture remains **90 seconds**; its client remains **five seconds**. No broad Cargo Hack/CRAP, real OS trust installation or real provider mutation was performed.

| Check | Final result |
| --- | --- |
| Independent production HTTPS Create/authenticated Management + Storage/stopped reopen | Parent 1 pass, **29.68s**; explicitly launched child 1 pass, **29.60s**; final compilation 2m02s |
| API independent admission/scope/challenge tests | **7 pass**, 0.19s |
| Management replication-policy/caller authorization regressions | **6 pass**, 1.43s |
| Storage IdP introspection/current-owner/redirect/token-cache regressions | **8 pass**, 0.07s |
| Strict library Clippy, seven affected crates | **Pass**, 15.06s |
| Unified Create/recovery/retirement/offline reopen regression module | **4 pass / 1 deliberately ignored child**; parent 15.43s, explicitly launched child passed in 14.78s |
| Unified approved Join/authenticated Storage/source-offline reopen regression | Parent **1 pass**, 47.89s; explicitly launched child 1 pass, 47.76s |
| Changed Rust files rustfmt check, `git diff --check`, `just boundary-check` | **Pass**; no direct server-to-server dependency edges |
| IdP public-key regression module | **1 pass / 7 fail**, 12.52s, inherited fixture issue described below |

The independent production fixture proves, through real native-root-verified HTTPS and real owner constructors:

- Separate HTTPS addresses/roots and singleton scopes; no Tauri or parent/child ownership of service processes is required by the API (the test itself uses a child to isolate native roots/provider state).
- Wrong-owner code, missing code, wrong Origin and conflicting exact operation denial; prompt 202 without waiting for recipient registration/domain completion.
- Accepted Management work survives its submitting response, nonsecret recovery does not resume it, active exact retry returns 202, unauthorized cancel denies, explicit cancel succeeds, cancelled challenge replay denies, and stopped retry retains its public Client and endpoint identity.
- All owners become ready, User password login and token exchange work, trusted Management lists exactly one actual Device, and authenticated Storage lists an empty resource catalog. Missing/invalid protected-route authentication still denies after completion.
- Consumer secrets are absent from public status/discovery bodies and durable worker records; password likewise absent. Secret values are read only through the isolated provider for these negative assertions and never printed.
- Stopped reopen requires no external peer/import. A fault-injected completed-but-unretired IdP snapshot remains unfinished, recovers its exact intent and finalizes only under the original owner code, preserving the endpoint and normal authenticated use.
- After shutdown, deleting one protected Storage relationship secret makes reopen return an error. Worker evidence remains byte-for-byte unchanged and the provider slot remains missing, proving refusal rather than replacement.

The focused challenge test uses the actual setup admission/router with an arranged outstanding challenge and a counting registrar callback. Operation, role, registration and capability substitutions deny; exactly one valid registration succeeds and its replay denies. This is **handler-level replay qualification**, not wire interception or a claim of production HTTPS challenge capture. The production HTTPS fixture separately exercises the complete real recipient-to-IdP exchange and rejects unsolicited/cancelled challenge requests.

Independent scenario totals include interruption/recovery, normal login/use, stopped reopen/finalization and fault refusal; they are not fresh seed-only timing claims. No separate independent schema/KDF/key/seed phase instrumentation was added. The unchanged unified regression separately observed 1ms acceptance, 20ms endpoint, 3720ms IdP, 3360ms Management, 150ms runtime, 8143ms ready and about 1450ms stopped reopen; those are **unified**, not independent, measurements. Mock provider/trust commands and build/profile costs are not real deployment performance certification.

## Failures and limits retained honestly

- Several invocations reached the 200s terminal bound during compilation, before scenario completion. One compilation finished at about 199s and the tool killed the just-started fixture. Checked that no Cargo/rustc/fixture child remained, then reran identical commands under identical bounds; no deadline increase.
- Initial Storage request failures above reproduced before removing repeated work. After the repair, an early fixture assertion mistook transient startup `unfinished` for terminal reopen failure; polling now waits within its existing deadline. A later public-discovery poll initially failed on connection-refused during listener startup; it now polls transport readiness under the same existing deadline.
- One edit introduced a malformed split method chain during added existing-only reopen verification; rustfmt/compiler rejected it. Corrected before successful final validation.
- An initial IdP test filter selected zero tests; corrected to `public_key_tests::`. The actual run has seven failures at **untouched `crates/idp-service/src/oauth2/refresh_tests.rs:103`**, before token verification: the fixture inserts into `devices` in the IdP store, but the Device table moved to Management. `git show bba9d4921:crates/idp-service/src/oauth2/refresh_tests.rs` confirms that same insertion at the base, and this file has no diff. The suite is not claimed green; no Device table was restored to IdP and no unrelated fixture was rewritten to silence it.
- No independent Join, app mutation/SDK generation, rendered browser GUI, real trust/provider installation, lost-provider recovery, arbitrary-crash matrix, WAN/container/headless/multi-platform certification or full-spec/ticket acceptance is established.

## Literal Rust commands

Working directory: `/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-independent`. Each invocation has a 200000ms tool bound. Commands were rerun unchanged when compilation exhausted the tool bound.

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_create_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib router::routes::replication::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib idp_client::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-service --features replica --lib public_key_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p api -p idp-service -p idp-server -p management-service -p management-server -p storage-server -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_completion_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

Other final checks: changed-file `rustfmt --edition 2024 --config skip_children=true --check`, `git --no-pager diff --check`, `just boundary-check`, and `GIT_EDITOR=true git merge --no-edit integration/ui-driven-node-bootstrap`.
