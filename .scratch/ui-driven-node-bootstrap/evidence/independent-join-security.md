# Independent Join and Create security qualification

Date: 2026-10-19. Worktree: `of/.scratch/worktrees/bootstrap-independent`; branch: `implement/bootstrap-independent`.

## Provenance and scope

Continued the substantial preserved uncommitted independent Join implementation after the previous transport failure. Did not discard it, reset state, change branches, or edit the parent worktree. Starting HEAD was integration merge `2e52fe7f2`, containing independent Create commit `79fb84c1e`. Explicit final `GIT_EDITOR=true git merge --no-edit 2e52fe7f2` reported `Already up to date`.

SHA-256 of `git diff --cached --binary -- Cargo.lock crates` against `2e52fe7f2`: `8bda4312abaafd3243d09f845b36dd45766e59c95e60af61861d5774d56b7a58`. This includes all preserved and new Rust/dependency changes, excluding documentation and this receipt to avoid self-reference.

Delivered: independent owner API Create/Join workers, owner-local public history import, source ordinary-User approval, concrete Create security fixes, and bounded HTTPS/authenticated-use/offline-reopen qualification. Existing-app independent mutation wiring and generated clients remain integration-owner work. No ticket closure or full-spec acceptance is asserted.

## Security fixes and red/green evidence

### Lost pre-domain initialized record

Extended the real HTTPS Create fixture at its existing stopped Management interruption: its public Client and provider-backed endpoint/consumer secret exist, but its domain directory does not. Removing `worker.json` previously allowed `into_create` to succeed. The new assertion failed with `lost pre-domain initialized record must refuse before exact retry` (child 0.16s), establishing the reported defect.

`IndependentCreateControl::write_record` now durably writes `initialized.json` before the first worker record can precede provider mutation. Host reconstruction refuses an initialized owner with no worker record, even without a domain store. Record reads and writes also refuse lost initialized evidence, rather than falling back to newly generated IDs. An interrupted fence-before-record write deliberately fails closed; no provider attempt is permitted after that failure. The HTTPS regression verifies no missing record regeneration and unchanged original Client secret/endpoint key, then restores the fault-injected record solely to continue the fixture. Existing worker attempt flags still govern individual protected material; this fence does not replace them.

### Browser CORS on normal routes

The preserved uncommitted code already contained a separate normal-fallback CORS layer; the new focused fallback/authentication test passed before further changes. Do not claim a newly reproduced CORS failure on that preserved state. Simplified it to one configured-origin CORS layer outside the whole setup/normal composition. Setup retains its own exact-Origin guard; ordinary OAuth/Bearer authorization remains unchanged.

The actual HTTPS Create fixture now submits a normal password token request with the configured Origin and asserts its readable CORS response. Authorization/content-type preflight succeeds for that Origin. Hostile-Origin preflight and token requests cannot obtain a matching allow-origin response. Existing unauthenticated/invalid normal protected requests still deny. CORS is browser read policy, not authentication: a hostile nonbrowser Origin does not gain access, but need not cause a blanket normal-route 403.

### Forged challenge versus cancellation/finalization

A focused admitted-recipient test blocked its secret loader while submitting a forged exact-public-values/chosen-nonce challenge. Before the fix, authorized cancel exceeded the 250ms assertion because the lifecycle gate was held; the test failed with `forged callback must not hold lifecycle gate` (0.28s). It passes after the fix.

The recipient captures its active cancellation token and secret loader under the gate, then releases the gate for protected provider work and outbound verified HTTPS. Cancellation interrupts waiting. Before receipt/public progress writes it reacquires the gate and revalidates the captured cancellation, shutdown, admission retirement, exact operation, active state and exact public registration. IdP's outstanding-capability consumption/accepted registrar path remains serialized with its own cancellation/finalization.

The production HTTPS Create fixture additionally runs an arranged delayed IdP callback over native-root-verified TLS. It deliberately holds the outbound registration reply open; authorized HTTPS cancel must complete within 250ms, the forged caller gets 403, status stays incomplete, no receipt/registered progress appears, and the cancelled command cannot finalize. The arranged server returns 204 when released, making this a cancellation-race test rather than a claim that a forged capability authenticates to a real IdP. The later full real-worker exchange still qualifies authentic capability registration. A post-finalization callback also returns 403 with byte-for-byte unchanged completed worker evidence.

## Preserved independent Join implementation

- Each target owner admits the same exact Join under its own host code. Initial discovery verifies the source's pinned Management endpoint signature over nonce/issuer/owner bases. Target admission never authorizes source approval.
- Source Management exposes ordinary authorized User acceptance, pending offers and exact approval/rejection. Pairing authenticates the joining endpoint. Device ID, token-signer member/key IDs and endpoint are distinct, operation-bound identities; topology does not create a Device per process.
- Source owner public-history streams are capability-bound to the exact signed approval and recipient. IdP imports only public identity/signing/history projections; Management imports only its public policy/Device history. Private password/client-verifier state and resource content are excluded. The existing candidate validation is shared from Management rather than maintained as an independent second implementation.
- Workers retain generated identities, approval/import evidence and consumer-local service secrets for exact retries. Completed reopen/finalization verifies retained owner state locally, without contacting the source or reseeding. IdP can verify existing source tokens from public trust without a source password verifier or private signer.
- The production fixture provisions an already-existing source explicitly; it does not qualify Join merely against a newly created source setup worker. It rejects an offer, cancels/stops target work, recovers exact source issuer/intent, rejects conflicts, retries identical endpoint/signer bindings, requires ordinary User approval, reaches authenticated Management and empty Storage use, denies source-password login at the target, stops the source, and qualifies completed-but-unretired finalization plus authenticated target reopen with the source offline.

## Storage timing investigation and same-request reuse

Two qualification runs failed at `normal joined Storage` on the unchanged five-second HTTPS timeout (Join children 45.03s and 52.86s). A temporary owner-route timing run then passed in 54.34s, but observed Storage requests taking 4824ms and 4734ms. It showed cold service-token acquisition plus current introspection/policy validation and the mandatory response-time recheck, not a lifecycle deadlock. Timings are diagnostic evidence, not a latency SLA or fresh seed-only phase breakdown.

Removed a concrete redundant current-Client lookup: bearer verification already validates the token's live Client registration. It now carries that registration's Application ID with the verified principal/claims, so remote introspection and local Management/Storage owner adapters do not fetch the same Client again solely for its Application ID. Current signer/root, Client registration, scope/audience, Principal and policy validation remain. No policy/validation cache, timeout increase or response-time-check removal was added. The public-key test explicitly checks the returned Application ID against the fixture's registered Application; learned signer/member/client/Principal revocation tests remain green. Temporary route timing prints were removed before final runs.

## Final checks

Ubuntu Linux x86_64. Offline/locked. MAIN target directory, 16MiB stack, one build job, incremental disabled, receipt33 selective profiles. One Rust build at a time; every tool invocation bounded at 200000ms. No powersets/CRAP, real CA installation or real protected provider mutation. Fixtures use isolated native CA-file roots, mock trust commands and isolated providers. Parent scenarios retain their 90-second bounds and production HTTP clients their five-second timeouts.

| Check | Result |
| --- | --- |
| Final combined independent HTTPS Create + Join | 2 pass, total 79.30s; children Create 25.57s, Join 53.52s |
| Previous final combined run after redundant lookup removal | 2 pass, total 80.37s; children Create 24.83s, Join 55.32s |
| API independent admission/scope/CORS/exchange/cancel tests | 10 pass, 0.25s |
| Public-key/independent signer/local revocation tests | 8 pass, 69.72s; compilation 48.80s |
| Client creation/rotation/public-metadata disclosure regression | 1 pass, 3.82s |
| Management replication policy/caller authorization regressions | 6 pass, 1.60s |
| Storage IdP introspection/redirect/token-cache regressions | 8 pass, 0.08s |
| Unified Create/recovery/offline reopen | 4 pass / 1 deliberately ignored child; parent 15.65s, launched child 15.02s |
| Unified approved Join/authenticated Storage/source-offline reopen | 1 pass; parent 49.67s, launched child 49.63s |
| Strict library Clippy, seven affected crates | pass, 14.87s |
| Changed-file rustfmt check; diff whitespace check; `just boundary-check` | pass; no direct server-to-server dependency edges |

Earlier compilation-only invocations exhausted the 200s tool limit; three later compilation-plus-scenario invocations were cut off after a child started. Process checks found no remaining Cargo/rustc/fixture process before continuing. Identical commands were rerun under the same bounds; no scenario timeout was increased. A mistargeted `oauth2::refresh_tests::` filter ran zero tests and is not validation; the correct `public_key_tests::` filter ran all eight. The first service-test compilation found a preserved assertion calling `is_none()` on the String `client_secret_hash`; corrected to `is_empty()` before the successful disclosure/public-key runs. Earlier receipt's public-key fixture blocker is resolved by the preserved signer/Device-separated fixture changes, not by reintroducing Device tables into IdP.

No rendered GUI, app wiring/client generation, real trust/provider installation, WAN/container/multi-platform deployment, arbitrary-crash matrix, lost-provider recovery or full ticket/spec acceptance is claimed. Timing still depends on machine/provider costs; these two final passes are bounded same-host evidence, not universal deployment performance certification.

## Stable integration contract

See `docs/api.md#independent-join-worker-contract` for literal request/status shapes. Initial Join sends `purpose:"join"`, exact nonce/Node/three HTTPS bases, empty `administrator` and `issuer`, and `peer:"https://source.example/management#<canonical-pinned-endpoint-id>"`, with no password. Every target uses its own Setup code. Recovery is code-authorized `POST /setup/intent` with no body; 200 returns the same nonsecret tuple with authenticated source issuer, 204 means no bound intent. Retry/cancel use that recovered tuple. Retained empty-issuer retry adopts the retained issuer without rediscovery. Status is exactly `stage`, `state`, `ready`, singleton `owners`; it does not contain source offers or login credentials. Public login Client is discovered from IdP `/setup/independent/public` at `facts.login_client`. Source approval is under the source Management base and ordinary User authorization, not the IdP base or target Setup code.

## Literal Rust commands

Working directory: `/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-independent`. All below had `timeout_ms:200000`; repeated unchanged when compilation exhausted that bound. The `production_independent` filter runs both production parents, whose children are explicitly launched by their isolated-root harnesses.

```sh
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-service --features replica --lib public_key_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-service --features replica --lib client_secrets_are_disclosed_only_on_creation_and_rotation --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib router::routes::replication::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib idp_client::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib api_create_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_api_join_completion_and_offline_reopen --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p api -p idp-service -p idp-server -p management-service -p management-server -p storage-server -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

Red runs used the same full environment/profile prefix with filters `production_independent_create_and_offline_reopen` (unified-server) and `delayed_forged` (api). The already-green preserved CORS characterization used `independent_normal_fallback` (api). Join failure/diagnostic runs used `production_independent_join_and_source_offline_reopen` (unified-server).
