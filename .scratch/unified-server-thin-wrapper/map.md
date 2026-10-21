# unified-server-thin-wrapper

Notes: Unified Host must remain thin wrapper per GLOSSARY.md; new routes violate service ownership.

Decisions-so-far:

- 01: Move both route modules in one vertical slice (merged per user)
- 02: Clean wrapper after removal
- 03: Verify composition intact

- Ownership confirmed by the user and current domain docs: Management owns hosted node joins; IdP owns hosted setup. No further ownership clarification is needed.
- Added public-boundary characterization coverage in `crates/unified-server/tests/composition.rs`: three supplied prefixes are preserved, absent setup/node-join paths remain 404, and owner-supplied setup/node-join paths are forwarded. This test passed; it does not prove production service ownership.

## Validation (partial attempt)

- PASS: `cargo test -p unified-server --test composition` (1 test).
- PASS: `cargo test -p unified-server --lib composes_service_routers_without_rewriting_prefixes` (1 test, unchanged).
- PASS: `cargo fmt --all -- --check`.
- PASS: `python3 scripts/check_server_dependencies.py`.
- FAIL: `cargo test -p unified-server --lib builds_three_prefixed_services_with_one_endpoint`: Node cannot resolve this worktree's `packages/management-client/esm/index.js`; the Rust fixture then fails at `runtime.rs:1626`. No production Rust change was made.
- INCOMPLETE: `cargo hack test --feature-powerset --all-targets`, bounded to 120 seconds, timed out during compilation.
- INCOMPLETE: `just crap`, bounded to 120 seconds, timed out during instrumented dependency compilation; no complexity result obtained.

## Fog / remaining work

- Tickets 01 and 02 are not implemented. `api_create.rs`, `node_join_source.rs`, their exports and the extra runtime merge remain unchanged. No claim of spec completion or preserved post-migration behavior is made.
- Server crates must not depend on other server crates (`scripts/check_server_dependencies.py`). `node_join_source.rs` currently depends on `IdpRuntime`, `OwnerProvisioner`, `UnifiedEndpoint`, `SecurityProtocol`, and issuer-discovery handling; these dependencies require ownership-aware separation, not a verbatim file move.
- `api_create.rs` contains a cross-owner worker as well as HTTP contracts/handlers, admission/cancellation state, readiness checks, and unavailable-owner routing. Its worker directly uses `UnifiedRuntime`, `LocalProvisioner`, `LocalJoin`, and credential/bootstrap/TLS types. Moving the file directly into IdP would create dependency cycles.
- Ticket 03 remains blocked by 02. Its new synthetic-router integration test verifies only the composition function, not that real service builders supply the moved features.
- Worktree is `of/.worktrees/of` on `implement/unified-server-thin-wrapper`. Ignored sibling links under `of/.worktrees/` preserve existing relative dependencies on ofdb/offs/ofnet. Tracker files were copied from the original working tree because `.scratch` is ignored by Git.
