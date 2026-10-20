# Transfer lifecycle review fixes

Base: `7c8fca4fe0cfdd70833449aa2231d53b1eb465d2`, on the existing integration branch. No branch switch, reset, or external dependency edits.

## Findings addressed

### HIGH: completed Management restart must retain its recovery API

`crates/management-server/src/independent_create.rs` separates retained Device/endpoint identity validation from current replication readiness. A completed worker still validates its exact operation, protected endpoint material, approved Device binding, current owner ID, and protected service credentials before restoring the ordinary authenticated router. Pending retirement or `admin_allowed = false` no longer aborts that restoration. Provisioning and readiness observations retain the full policy check, so readiness remains HTTP 503 until cleanup and ordinary policy recovery finish.

No unauthenticated recovery route or readiness bypass was introduced.

### MEDIUM: exact original owner return before retirement

Management completion and Storage cleanup accept equal old/new owners, while retaining the normal Device, endpoint, owner, cleanup-ID, conflict, cancellation, and service-authorization checks. An `A -> B -> A` return does not cancel retirement: Storage must still retire A's old SQL/KV/filesystem copies, recheck the binding, and acknowledge it. Completion leaves the policy restricted and unselected; activating A does not restore retired copies.

The preserved pre-existing dirty cleanup-history implementation retains completed history and generates a fresh cleanup ID for each later obligation. This review change does not replace or commit that pre-existing work. Pending cleanup retains its ID when only its destination owner changes; the destination remains part of acknowledgement validation.

## Regression coverage added (not executed)

- `independent_transfer_tests::original_owner_return_keeps_retirement_pending_after_stopped_reopen`: public Management repository boundary; exact original-owner return while pending, stopped pending/completed reopen, stale destination rejection, empty restricted policy, fresh ID on a later transfer, and same-owner stale-ID replay rejection.
- `independent_transfer_tests::independent_transfer_production_https`: existing authenticated HTTPS/real Storage-copy scenario now returns to the exact original owner during delayed acknowledgement, rejects the stale acknowledgement, completes via the Storage adapter, and checks old database/filesystem copies remain unavailable after activation and another stopped reopen. Existing unrelated-owner, service-scope, live-handle, cancellation, and fault assertions remain.
- `review_transfer_restart_tests.rs`, called by the existing `production_independent_create_and_offline_reopen` fixture: completed independent Management worker restarts with pending original-owner retirement, exposes authenticated routes despite readiness 503, rejects unauthenticated and stale cleanup, accepts the exact authenticated acknowledgement, stops and restarts again with `admin_allowed = false`, rejects unauthenticated policy mutation, accepts the ordinarily authorized administrator update, and reaches readiness 200 without resurrecting selections. The fixture reuses its existing explicit restriction permission grant.

TDD skill loaded; regression tests were written before their corresponding production edits. Red/green execution was intentionally deferred, not observed.

## Risk / complexity / priority

- Priority: HIGH restart deadlock and MEDIUM owner-cycle wedge, as reviewed.
- Complexity: small production change: split the existing identity/readiness checks and remove the invalid owner-inequality requirement; no dependencies, schema changes, or compatibility layer.
- Risk: security-sensitive recovery and retirement ordering. Existing authentication and all remaining cleanup binding checks are retained. New tests target these public boundaries, including stale evidence and stopped reopen.

## Validation and build hold

Executed successfully: targeted `rustfmt --edition 2024 --config skip_children=true` on the six changed Rust files, and `git diff --check`.

No Cargo build, check, test, clippy, diagnostics refresh, coverage, or `just crap` was run. The parent validation receipt records an unrelated dependency blocker at `ofdb/crates/btree/src/btree.rs:63`: E0599 due to missing `StreamExt`. This report was not reverified with a build, and that external user work was not edited. The other agent's no-build hold remains in effect. Compilation and runtime behavior of the new tests are therefore unverified.

Pre-existing dirty changes, including other hunks in selection policy and the Create fixture, remain outside this review commit. Full regression verification must use the integration working tree with those preserved fixes (or their eventual owning commits).

## Pending verification commands

Run from `of` only after the external blocker is cleared and the build hold is released:

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib independent_transfer_tests:: --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p unified-server --lib production_independent --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

Use a 200000ms tool bound per command and run one Rust build at a time. Broad Cargo Hack powersets, CRAP, full-workspace/platform sweeps and privileged native installation remain prohibited without renewed authorization, as specified in the spec. The production HTTPS wrapper tests launch their ignored child fixtures with isolated native TLS roots and protected credential providers; do not invoke the ignored children directly without their wrapper environment.
