# Final model and dependency integration

Branch: `backend-simplification-final-model`  
Worktree: `.scratch/worktrees/final-model`  
Base: `52ac21c7b`

## Scope and risk

High priority, high risk: shared Rust type identity, signed join serialization, and feature isolation. Complexity is concentrated in contract generation and independent runtime dependencies. No CRAP score was obtained. Setup source, desktop, local seed, and persistence code are unchanged. No merge back is made.

## Changes

- OAuth mutation contracts now have a canonical, non-generated source in `of-model/src/oauth_user.rs`. The integration base had already removed the service duplicates. Generation now reuses these definitions instead of overwriting their unknown-field validation.
- Hosted `NodeJoinRequest` and `NodeJoinApproval` moved from Device service to `of-model/src/node_join.rs`. Generated Device wire types reuse them. Validation, signature operations, transport, and owner behavior stay in Device service through runtime extension traits. The contract test checks the exact signed request field order.
- Generated setup wire contracts have one source in `of-model/src/setup.rs`. Four owner/client namespaces re-export the same definitions. Removed their duplicate files. Generation preserves canonical definitions.
- SDK resource handles and sync payloads moved to `of-model::storage::{sdk,sdk_sync}`. SDK modules now export these contracts. SDK, HTTP, and server session status use one `SyncStatus`. Direction uses the existing server enum, including Push/Pull/Bidirectional; this does not add directional transport behavior. Time/random ID constructors require `std`; payloads and supplied-ID constructors build with alloc only.
- Added `independent` features to owner servers, Device service, and server-common. CLI enables independent behavior. Local owner builds no longer enable setup. Device independent join/history modules are not compiled in the local feature graph.
- Canonical OpenAPI serving and the system-trust HTTP client no longer require setup. Setup keeps its unchanged implementations. IdP permission-client construction uses server-common directly.
- Full OpenAPI export requires `unified-server/independent-contracts`. Updated Cargo example metadata, package scripts, qualification script, and Just command. Added tests for independent operations, all published paths, and the typed OAuth update body.
- Fixed two source-document omissions: Storage's active `/sync/connect` route was missing from OpenAPI registration; IdP user update declared `Object` instead of `UpdateUserInfoRequest`.
- Regenerated all four canonical JSON documents from full source registrations, including independent operations. No published path was removed.

## TDD evidence

- Dependency contract tests first failed because unified included setup and the `independent` feature did not exist. Both now pass.
- Alloc model contract checking first failed on absent canonical modules and distinct status types. The contract tests now pass.
- Generator contract test first failed because extraction recreated setup and join definitions. It now passes.
- Full OpenAPI test first failed on the missing `/sync/connect` path. It now passes, including the named OAuth update request schema.

## Passed validation

Builds used the worktree target directory or the existing root target cache. Compiler warnings remain.

- `cargo check -p of-model --no-default-features --lib`.
- `cargo test -p of-model --no-default-features --test canonical_contracts --test shared_contracts`: 5 new and 3 existing tests.
- `cargo test -p of-model --no-default-features --features std,utoipa --test canonical_contracts --test shared_contracts`: 6 new and 4 existing tests.
- `cargo check -p unified-server --no-default-features` and `cargo check -p unified-server`.
- `cargo check -p idp-server -p device-server -p storage-server --bins` with owner defaults (CLI and independent).
- `cargo test -p device-service --features std --lib hosted_join::setup_peer_tests`: 1 test.
- `cargo test -p server-common --no-default-features`: 2 tests.
- `cargo test -p unified-server --features independent-contracts --example export-openapi`: 2 tests.
- `cargo run -p unified-server --features independent-contracts --example export-openapi -- target/final-model-openapi`.
- `cargo clippy -p of-model -p server-common --all-features --lib`: completed with warnings.
- `node --test scripts/model-build-contracts.test.mjs scripts/shared-rust-models.test.mjs`: 4 tests. Adding `scripts/generated-independent-client.test.mjs` passes all 8 tests.
- `python3 scripts/check_server_dependencies.py`.
- Literal `cargo tree -p unified-server --prefix none --format '{p}'`: no setup entry. Independent owner feature graphs retain setup.
- Changed Rust files formatted with Rustfmt. `git diff --check`.
- Source diff against base confirms no changes under `crates/setup`, `apps`, `crates/embedded-db`, or `crates/idp-server/src/local_seed.rs`.

## Remaining gaps — do not mark tickets 01 or 06 fully resolved

1. **Strict single source remains impossible under unchanged setup source.** Setup directly defines `IndependentCreateIntent`, `IndependentCreateRequest`, `IndependentOwnerPublic`, `IndependentPeer`, and `Challenge`. Its private `Registration` also repeats Challenge's wire fields. Setup imports generated clients, which depend on of-model. Making of-model import setup creates a cycle and removes the alloc-only boundary. Replacing setup definitions with model imports changes setup source. Neither was done; no conversion shim was added.
2. **Independent join runtime identities remain split.** `device-service/src/independent_join.rs` still defines `IndependentJoinRequest` and `IndependentJoinApproval` using setup's intent/peer types and their runtime validation. Generated clients now share their model wire definitions, but those are not the runtime identities. Hosted join extraction is complete; independent join extraction is not. Other pre-existing HTTP/domain representations (for example generated JWKs) are not all consolidated. A shared crate is not proof of a single definition per concept.
3. **Management ownership drift is exposed, not hidden.** Full source export contains `/devices/{device_id}/sync-selections` and `/resources/{resource_id}/sync-targets`, with 20 Management operations rather than the old artifact's 17. `node --test scripts/owner-contracts.test.mjs scripts/generated-independent-client.test.mjs scripts/shared-rust-models.test.mjs` has 9 passes and 1 failure: Management must not publish Device authority. The source registers these routes on the base commit. Canonical JSON now reflects that source instead of masking it. Runtime route cleanup is left to the validation/runtime owner.
4. **Device test inventory is stale.** `node --test --test-name-pattern='current composed Device schema' scripts/generated-device-client.test.mjs` fails with `50 !== 48`. Both the base JSON and regenerated JSON contain 50 operations. The broader generated-client test command timed out after 30 seconds; it is not a pass.
5. **SDK runtime build remains unverified.** Direct `cargo check --manifest-path crates/storage-sdk/Cargo.toml --no-default-features` fails because this retained crate is neither a workspace member nor excluded. Its manifest also retains a path to the removed storage-sync-client. Shared SDK model contracts are tested; the SDK runtime is not repaired or claimed to build.
6. **PostgreSQL is not implemented by this change.** Existing ofdb network transport is not a PostgreSQL backend or PostgreSQL wire protocol. No PostgreSQL acceptance claim is made.
7. **No no_std runtime claim.** Alloc-only model compilation and contract tests pass. Owners, server-common, HTTP, TLS, and async runtime code still require std. A no_std runtime was not implemented.
8. **Full validation is incomplete.** `cargo hack test --feature-powerset --workspace --all-targets` timed out after 200 seconds during compilation. `just crap` timed out after 60 seconds. No score or full-workspace test pass is claimed. Live independent setup, browser/desktop, full client regeneration, PostgreSQL, and local persistence behavior were not qualified here.
