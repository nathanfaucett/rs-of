# Review fix: confidential-client refresh revocation

## Scope and assessment

- Base: MAIN checkout `of`, branch `integration/ui-driven-node-bootstrap`, commit `f1ff45a0a19415151696fcfce20103713745ce93`.
- Authorized code scope: `crates/idp-service/src/oauth2/service.rs` and `refresh_tests.rs`. Only `service.rs` required a change; the existing test remains unchanged. No tickets or other scopes edited.
- Complexity: one repository-lookup substitution. Risk: security-sensitive client authentication; preserve verification rather than bypassing it. Priority: restore valid confidential-client refresh revocation.
- TDD skill invoked. User-agreed seam: existing `oauth2::service::refresh_tests::refresh_issuance_rotation_reuse_and_revocation`, exercised unchanged before and after the fix.

## Root cause and fix

`revoke()` loaded `find_client_registration`, which provides client metadata without the local secret verifier, then passed that client to `authenticate_client_for_token_endpoint`. Consequently, valid confidential-client credentials were rejected.

Restore `find_client_by_client_id(&claims.client_id)` only in this secret-authentication path. Metadata-only bearer validation and application-ID lookups remain unchanged. No authentication checks are removed or weakened.

## Red / green evidence

The identical command was executed serially before and after the one-line fix:

```sh
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-service --features replica --lib refresh_issuance_rotation_reuse_and_revocation --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --nocapture --test-threads=1
```

- **Red:** exit 101; 0 passed, 1 failed, 58 filtered out. Panic at `refresh_tests.rs:482:10`: `revoke replacement: ErrorResponse { error: InvalidClient, error_description: Some("invalid client credentials"), error_uri: None }`. Build 2m42s; test 6.81s.
- **Green:** exit 0; 1 passed, 0 failed, 58 filtered out. Build 52.98s; test 9.60s.
- Existing coverage includes refresh issuance, rotation, reuse rejection, missing client authentication rejection, successful confidential-client revocation, idempotent revocation, rejection of the revoked replacement, and validity of a separate refresh token.

Additional checks passed:

```sh
rustfmt --edition 2024 --check --config skip_children=true crates/idp-service/src/oauth2/service.rs
git --no-pager diff --check
```

The initial direct rustfmt check omitted the edition and failed because rustfmt defaulted to Rust 2015; rerunning with the verified workspace edition 2024 passed. No file was rewritten by either check.

## Resource receipt and limits

Used the task's sole granted Rust build token, one invocation at a time, MAIN's existing target directory, offline/locked resolution, one build job, incremental disabled, receipt33 selective profile overrides, and the existing 16MiB stack. Every terminal invocation had `timeout_ms=200000`; no scenario, observation, client, or other bounds changed. No dependencies, lockfiles, or build configuration files changed. No broad feature-powerset, CRAP, or workspace validation was run for this narrow review fix.

Both Rust test invocations completed; no build remains running. Rust build token released at completion of this task.
