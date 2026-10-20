# Receipt 42 — Final review fixes and independent signer-free normal operation

## Source, scope and controls

MAIN checkout `of`, branch `integration/ui-driven-node-bootstrap`, base `98dfa159d` (receipt 41). Read the spec and receipts 38–41 before implementation. Entry Rust diff SHA-256 (`git --no-pager diff --binary -- '*.rs' | sha256sum`) was `50964d97952fbebe7597f8307c3153deefbe469ee4572202ee9d1f200da67109`: all 16 dirty Rust integration files and the existing `evidence/review-transfer-lifecycle.md` deletion were preserved. No worktree, branch, external-repository, dependency, SDK/TS, real provider, real profile or privileged OS-store changes.

The final integration commit includes those pending fixes, this review slice, and only explicitly named evidence paths. Receipts 38/39 are preserved and added as historical provenance, not rewritten into passes. Receipt 40/41 already belong to earlier commits. The review artifact is relocated to `review-transfer-lifecycle.md` here; it was already revised before entry, so this is **not a byte-identical rename**. Old HEAD artifact SHA-256: `87cca3c2503f7773cbe2cdf2f99041f0593659de482447c857e8fe1efdeb5e84`; preserved relocated artifact: `f57ccda3c719685c454515d3f1456c4f13bf00619d8fea01d9f8e4aff8a27c3a`. Its historical validation hold remains historical; this receipt records current execution. No parent ticket/status is closed or edited.

Final staged Rust-only diff SHA-256, including the new test module (`git --no-pager diff --cached --binary -- '*.rs' | sha256sum`): `c7577858add3240b05c17e47fdffd21fee9ecf4e2ad9a4c265a723844e63c232`.

One exclusive granted Rust token, sequential builds only. All Cargo calls below: working directory repository root, tool `timeout_ms:200000`, offline/locked, one job, incremental disabled, 16 MiB thread stack and unchanged receipt-38 selective profiles. The first API command observed a build-directory lock wait; editor analyzers PIDs 17703/23931 were then paused with `kill -STOP 17703 23931`. No editor configuration changed. They are resumed at token release after commit; hooks are disabled explicitly for that commit so no unqualified hook build runs.

Complexity/risk/priority: three localized high-priority fixes, with security-sensitive failure ordering. Reuse existing routes, native verification client, retained reset journal, command runner and public runtime/repository seams. No new dependency, per-phase public protocol, authorization cache, warming request, acceptance retry, increased deadline or removed current-state check. Broad powersets, CRAP and workspace sweeps remain outside the explicit spec/task boundary.

## Review findings fixed

### HIGH — Listener verification is not current remote readiness

`crates/api/src/setup_server.rs` now uses an ordinary `OPTIONS` request on the existing status path to establish OS-native verified HTTPS/HTTP serving. Existing CORS may answer 200; a bare method router answers 405. Neither dispatches the readiness-bearing GET. No new route or public phase contract was added. Removed the obsolete startup-only readiness-body validator. The same native-root-only client, no redirects/proxy, one-second connect and five-second total request bound remain.

Current `GET /setup/status` still evaluates real owner readiness and reports 503/unavailable when it fails. Normal route authentication and authorization remain independent of status. The deterministic API child installs a delayed status handler before starting the listener: a six-second readiness check must not cancel the listener, and it must remain usable and recover without replacement.

The production fixture separately creates a completed independent deployment, stops it, and reopens the actual Storage host with an isolated TLS authority listener delayed for six seconds. It observes the restored normal router's 401 before the authority responds, observes the unchanged five-second status request timeout, verifies the Storage task stays alive, then observes real 503/unavailable when the authority is unavailable. Starting the real retained IdP/Management restores readiness on that **same still-running Storage host**. Worker evidence for all three owners stays byte-identical: no reseed. The deterministic API test establishes the pre-probe delayed-handler ordering; the production test establishes actual completed-host restoration and remote-authority behavior.

### MEDIUM — Interrupted reset cannot become fresh setup

`crates/api/src/independent_setup.rs` refuses ordinary prepare if either `setup-reset` or `setup-reset.json` exists, before existing/fresh domain handling or any admission/TLS creation. Only explicit exact reset continuation can remove that evidence.

Public prepare/reset regression covers both durable cleanup windows: after live-to-retired rename, and after retired-tree removal before journal removal. Ordinary prepare denies with no new setup root/admission/TLS; exact reset retry finishes without repeating trust removal; fresh prepare is allowed only afterward.

### MEDIUM — Linux distro uninstall is the inverse of the supported fallback

`crates/api/src/independent_reset.rs` preserves command spawn errors rather than reducing all outcomes to Boolean. Only an absent direct `trust` executable (`NotFound`) allows distro-only removal without first requiring `pkexec trust`. An existing tool's unsuccessful removal still requires successful native elevation; genuine spawn/removal/refresh errors propagate. An absent tool with no supported distro root refuses.

The fallback still checks the fingerprint-derived owned filename **and exact certificate DER** before removal, verifies the file is gone, and refreshes supported stores. Refresh is retried even when an earlier deletion already succeeded. Tests inject isolated roots and command execution: successful distro-only inverse, unrelated certificate preservation, mismatched contents refusal, removal failure, refresh failure and deletion-already-completed retry. No real OS store or provider command executes. Local TLS/journal evidence is retained on failures by the existing reset ordering.

## Spec 118 — Actual independent normal operation, not policy publication

New focused `crates/unified-server/src/independent_operation_tests.rs` uses the ordinary public setup APIs and real IdP/Management HTTPS services, then production `storage_server::build_http_runtime`, `RouterState`, `IdpClient`, `ManagementClient` and an isolated Storage root. It consumes existing protected service registrations; it does not inject a fake authorizer, create an autonomous Storage endpoint, copy an endpoint private key or enroll a Device per process. Existing fixture helpers are reused; the old ~80-second Create wrapper is not extended.

A Management-only approved endpoint is explicitly absent from the current public IdP signer-trust enumeration while a valid issuer signer remains. With a real OAuth token and its exact permitting Management policy, the new runtime performs actual `GET /storage/databases` and decodes the real empty catalog. Missing authentication denies. The same token is tested through policy false → true → false. A counted TLS-listener adapter proves only **one accepted TLS connection** for those requests, not merely reuse of a reqwest client. Response bodies are consumed for keepalive. After retained Storage reopen, policy denial persists; restoring policy permits the actual operation; endpoint revocation then denies on the same accepted TLS connection. A second retained Storage reopen preserves endpoint denial with that token. IdP worker evidence is unchanged.

All requests use the production five-second verification client and isolated native roots. The parent bounds the whole new child at 90 seconds. Existing unified signer-free authenticated exchange evidence remains separately scoped in [receipt 30](30-signer-and-endpoint-separation.md); it is not represented as a new unified run here. The new independent proof is normal Storage operation, not the earlier signer-free policy-publication-only checks.

## Red/green, failures and bounded retries

- Reset crash-window test: RED at `ordinary startup cannot replace reset evidence` (40.34s build, 0.05s test). Added prepare guard: GREEN (40.15s build, 0.11s test).
- Linux isolated inverse test initially RED at E0425 for the absent injected-root helper. After implementing it, all five reset tests GREEN (38.50s build, 0.25s tests), then included in final API suite.
- Deterministic delayed-readiness test: RED because the listener had finished after seven seconds (child 7.00s, wrapper 7.04s). New listener probe: GREEN in API suite. API suite passed twice, **37 passed / 3 explicit child ignores**, 7.95s and final 7.96s; ignored children are run by their wrappers.
- New production operation command had **five** 200000ms tool timeouts: three during dependency compilation and two after compilation while the scenario was running. These are not passes or 90-second child-deadline failures. Identical bounded retries are listed below. Two interrupted temporary fixture roots were removed after checking no test process remained.
- First completed composed operation run failed before setup, with `setup HTTPS status verification failed` for all owners, then readiness assertion after 30.05s (build 2m36s). CORS answers OPTIONS with 200, unlike the bare router's 405. Accepting those two existing non-readiness responses fixed that protocol assumption; no GET readiness/auth checks were relaxed.
- Preliminary completed operation fixture passed at **47.46s child / 47.55s wrapper**. Added explicit connection counting and endpoint-revoked retained reopen. Final-source fixture passed twice: **49.37s / 49.46s**, then **50.04s / 50.14s**. No failed completed final-source operation run is omitted.
- Storage local-authorizer command had one compilation timeout; identical retry passed (1m36s build, 6.33s tests).
- No test filter matched zero tests in this receipt. No scenario HTTP or parent deadline was increased.

## Final focused results

| Check | Result |
| --- | --- |
| API complete library | 37 passed / 3 explicit wrapper-child ignores, 7.96s |
| New independent operation + delayed authority, final source twice | 1 wrapper passed each, children 49.37s and 50.04s |
| Unchanged current/historical independent Join, paired twice | 2 passed each, 129.32s and 131.30s suites; historical children 63.92s and 66.25s |
| Independent Create and historical-record Create/reset | 2 passed, 149.57s suite; historical child 76.16s |
| Actual independent transfer and original-owner/replay checks | 3 passed / 1 explicit child ignore, 26.58s; actual HTTPS child 24.26s |
| Management complete library | 38 passed / 2 explicit ignores, 13.39s; build 2m42s |
| Management selection-policy replica tests | 14 passed, 14.40s; build 2m19s |
| Storage local authorizer | 4 passed, 6.33s |
| Storage Management client | 10 passed, 0.08s |
| Storage delayed SQL-frame/filesystem-content release | 2 passed, 37.60s |
| Strict affected-library Clippy | PASS twice, 17.51s then final 0.36s, `-D warnings` |
| Scoped rustfmt, diff whitespace, server dependency boundary | PASS; no direct server-to-server dependency edges |

Current/historical Join fixtures and their first normal authenticated Storage catalog request were **not changed in this review slice**. The preserved entry delta in that file remains only the historical-reset wrapper/flag. Final file SHA-256: `a8ece2728f3152b0792260dd4e75285ba682460e1dfca8faeb881c4e3d2c6be3`; diff against base SHA-256: `a321daf55dcb7fb08f7cd7bc4491e2fbc7a5c2746c12884f4fbe95a648b3a0ae`. No extra first-catalog probe, retry or warming was introduced. Passing repetitions are finite qualification, not a universal latency guarantee.

## Literal bounded Cargo commands

Each line below was invoked literally from `of` with tool bound 200000ms. Counts include failures/timeouts. Non-Cargo preparation (`pgrep`, analyzer pause or rustfmt) sometimes preceded the listed Cargo command in the same shell; builds were never parallel.

```sh
# Reset crash window: twice (RED, GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib interrupted_cleanup_refuses_prepare_until_exact_reset_finishes --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Linux inverse: once (RED compile)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib distro_only_trust_removal --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Reset group: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib independent_reset::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Startup: once (RED)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib listener_survives_readiness_slower --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# API complete: twice (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# New operation fixture: nine invocations (five tool timeouts, CORS failure, three passes)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_signer_free_operation --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Current + historical Join: twice (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_join --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Current + historical Create/reset: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent_create --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Management library: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Management policy: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-service --features replica --lib replica::selection_policy::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Storage authorizer: twice (compile timeout, GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib local_replication_authorizer:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Storage client: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib management_client::tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Delayed release: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p storage-server --lib delayed_http_policy_cannot_release --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Transfer: once (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib independent_transfer_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
# Strict affected libraries: twice (GREEN)
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo clippy --offline --locked -p api -p idp-server -p management-server -p management-service -p storage-server -p unified-server --lib --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- -D warnings
```

Final static commands (20000ms bound for the initial formatting/boundary call; the final combined static/Clippy call used 200000ms):

```sh
rustfmt --edition 2024 --check --config skip_children=true crates/api/src/independent_reset.rs crates/api/src/independent_setup.rs crates/api/src/setup_server.rs crates/idp-server/src/independent_reset.rs crates/management-server/src/independent_reset.rs crates/management-server/src/router/routes/replication.rs crates/management-server/src/runtime.rs crates/management-service/src/replica/mod.rs crates/management-service/src/replica/schema.rs crates/management-service/src/replica/selection_policy.rs crates/storage-server/src/idp_client.rs crates/storage-server/src/independent_create.rs crates/unified-server/src/api_create_tests.rs crates/unified-server/src/independent_create_tests.rs crates/unified-server/src/independent_join_tests.rs crates/unified-server/src/independent_reset_tests.rs crates/unified-server/src/review_transfer_restart_tests.rs crates/unified-server/src/independent_operation_tests.rs crates/unified-server/src/lib.rs
git --no-pager diff --check
python3 scripts/check_server_dependencies.py
```

## Qualification boundary and handoff

The three concrete review findings and missing independent actual-operation proof are implemented and final focused qualification passes. Pending receipt-38/39 integration changes are included coherently, rather than discarded or claimed as already committed. No frontend/SDK tests were rerun or source changed; receipt 41 remains that boundary. No full-workspace/native-platform/provider certification is claimed. TLS/provider fixtures use isolated roots and injected/mock commands only. Tickets and parent scratch statuses remain open for parent adjudication. Rust token is released after commit and analyzer resume; no further Rust command follows release.
