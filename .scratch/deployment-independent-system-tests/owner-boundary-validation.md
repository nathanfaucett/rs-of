# Moved owner boundary validation

Validated on `refactor/boundary-owner-tests` after fast-forwarding to integration
`9cd44cf6d` (which includes the owner-test migrations). No source, fixture,
production-owner, or boundary changes were required.

## Compilation and dependency boundary

From the owner-tests worktree, using the shared target directory and `-j8`:

```sh
cargo check -p idp-unified --no-default-features --test owner_boundaries -j8 --offline --target-dir ../of/target
cargo test -p idp-unified --no-default-features --test owner_boundaries --no-run -j8 --offline --target-dir ../of/target
cargo check -p device-service --no-default-features --features replica --test owner_cleanup -j8 --offline --target-dir ../of/target
cargo tree -p idp-unified --no-default-features --edges normal,dev --prefix none --format '{p}' --offline
```

All compilation commands passed. The root target's normal/dev dependency tree
contains no `unified-server` dependency. All moved tests in `owner_boundaries`
compile without root default features; Device cleanup explicitly enables only
its required `replica` feature.

## Runtime results

Executed the freshly checked test executable
`../of/target/debug/deps/owner_boundaries-f196021ee7efba92` with
`RUST_MIN_STACK=67108864`, `--exact`, and `--nocapture`.

| Test | Result | Runtime | External process bound |
| --- | --- | --- | --- |
| `node_join_bootstrap_tests::authorized_node_bootstrap_imports_both_contexts_and_reopens_offline` | Passed | 168.50 s | 600 s |
| `native_api_tests::generated_rust_and_built_javascript_use_actual_owner_routes` | Passed | 12.14 s | 120 s |

The bootstrap completed both public-context imports, local token issuance,
source update and exact retry, revocation denials, source disconnection,
offline owner reopen, and offline issuance/local revocation. The earlier
120-second incomplete run was not a test failure; the adequate-bound run
completed successfully without code changes.

## Actual-owner JavaScript qualification

The Rust test continued to invoke `scripts/qualify-built-openapi.mjs` against
its real IdP, Storage, and independent-setup listeners. No skip, substitute
server, alternate assertions, or new artifact-root environment variable was
introduced.

Existing built artifacts from the main checkout were used read-only through
temporary worktree symlinks at these otherwise missing paths, each targeting
the same relative path in `../of`:

- `node_modules`
- `packages/idp-client/esm`
- `packages/management-client/esm`
- `packages/device-client/esm`
- `packages/storage-client/esm/generated`
- `packages/storage-client/node_modules/@aicacia`

The tracked Storage `esm/index.js` was not replaced and matches the main
checkout's built entry point. The initial artifact setup omitted the last
package-local workspace link; Node reported `ERR_MODULE_NOT_FOUND` for
`@aicacia/idp-client` imported by Storage. Adding that existing package-local
link resolved the environment failure. The final run printed:

```text
Built package exports and generated operations qualified against actual owner routers
```

All temporary links were removed in a `finally` block. No main-checkout files
or staged changes were modified.

## Scope

Only this validation report is committed. No CRAP, feature powerset, full
library suite, or other expensive suite was run. Unified wrapper cleanup and
full-node validation remain with their respective owner.
