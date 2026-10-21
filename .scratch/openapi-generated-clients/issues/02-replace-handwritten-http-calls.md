# 02: Replace handwritten HTTP calls with generated clients

**What to build:** All project-owned Rust and JavaScript/TypeScript API consumers use generated HTTP operations from the shared contracts, while retaining existing authentication, transport safeguards, domain checks, and direct in-process owner execution.

**Blocked by:** 01: Check in complete OpenAPI specs and generate both clients.

**Status:** resolved

- [x] Migrate every project-owned outbound API operation identified by the inventory, including OAuth token acquisition, IdP identity/discovery calls, Management policy/replication calls, Storage resource calls, public Setup, independent handoffs, and Node-join HTTP operations where present.
- [x] Generated operations own paths, parameter encoding, request serialization, and response decoding. Application-owned code handles only necessary credential caching, transport configuration, domain validation, and error mapping.
- [x] Preserve service-token caching and renewal plus all required issuer, audience, token-type, expiry, scope, and domain-response checks. Successful deserialization does not grant authority.
- [x] Preserve native-root TLS verification, applicable HTTPS restrictions, redirect/proxy policy, timeouts, and bounded streaming response consumption. Preserve operation-bound Setup admission, recipient binding, secret handling, and fail-closed behavior.
- [x] Keep Unified Host owner calls local and current-state based. Do not introduce loopback HTTP, a remote authorization dependency, new universal client traits, or domain ownership changes.
- [x] Exercise migrated operations through generated SDKs against actual composed routes, with correct topology prefixes, expected statuses, malformed responses, missing/wrong credentials, and denied permissions. Retain focused transport-failure and local-owner checks.
- [x] Delete replaced handwritten operations, duplicate wire definitions, and obsolete helpers; do not retain compatibility wrappers or fallback implementations. Leave generic fetch infrastructure and non-HTTP protocol implementations intact where still needed.
- [x] Run focused tests and builds for changed consumers and update the inventory with evidence of migrated or explicitly out-of-scope calls. No application UI redesign or unrelated behavior change.


## Answer

Implemented on `implement/openapi-consumers` in the isolated `of-openapi-consumers` worktree, based on `cee87ccff`. The parent worktree was not modified. All production operations in `docs/openapi-inventory.md` are migrated or explicitly classified at their actual boundary; no project-owned handwritten HTTP operation remains as a compatibility fallback.

Generated operations own endpoint paths, encoding, request serialization and response decoding. Existing credential caches, renewal, verified TLS/redirect/deadline policy, stricter per-operation response limits, token/domain checks and current-state in-process owner calls remain. Independent history and browser Storage handshakes use generated request builders while retaining their separate WebSocket/binary protocols. Both applications explicitly select the generated project-owned OIDC mode; arbitrary external-provider/native-protocol support remains separate.

Removed replaced request/response helpers and duplicate wire structs. Corrected generation for closed object schemas, exact Rust success statuses, complete OAuth form grant literals, composed JWK variants and the Storage owner-base root path. Corrected the introspection scope schema annotation to its actual serialized string. No framework or compatibility layer was added.

Risk/complexity/priority: high security risk and high priority because this touches credential recipients and authorization boundaries; implementation breadth is substantial, but operation plumbing stays in existing generators and SDKs. Approved seams were actual composed routers, offline generation and transport boundaries. Red/green regressions demonstrated oversized request rejection, exact cleanup status, actual JWKS decoding, foreign OIDC recipient refusal and the actual Storage upgrade URL.

### Passing evidence

Use `RUST_MIN_STACK=67108864` for Rust qualification, consistent with the existing coverage recipe.

- `node scripts/qualify-openapi-clients.mjs`: passed. Two byte-identical state-free exports match committed contracts; both SDK families regenerate without drift and compile; 31 generated/generation-failure JavaScript tests pass; five generated Rust transport/actual-owner tests pass; built host-composed Node-join qualification passes. The built Storage consumer now lists, creates and opens a real socket against actual routers, and actual IdP JWKS variants decode through the generated Rust SDK.
- `cargo test -p unified-server --lib production_independent_join_and_source_offline_reopen -- --nocapture`: passed, including actual verified HTTPS owner handoffs, source enrollment, generated history handshake/public import and source-offline reopen (72.55 seconds).
- `cargo test -p management-service --features replica hosted_control_plane::tests`: 16 passed, including current local owner checks and resource identity validation.
- `cargo test -p storage-server --lib client::tests`: 20 passed, including renewal, bounds, malformed/unauthorized evidence, endpoint canonicality, exact cleanup acknowledgement and local inspector behavior.
- `cargo test -p lidp --lib`: 17 passed, one parent-only child test ignored; the parent executes that child. Native peer revocation and lifecycle/readiness/trust checks pass.
- The isolated existing IdP `independent_permission_http_binds_exact_request_and_fails_closed` test passed (30.48 seconds), including wrong certificates, forged/mismatched decisions, redirects, bad credentials and upstream loss. Temporary diagnostic output was removed from source after isolating the broader batch failure.
- App `pnpm exec vitest run --config vitest.auth.config.ts`: 200 passed. Storage package `pnpm exec vitest run`: 18 passed. OIDC package: seven passed.
- IdP, Management, OIDC and Storage ESM/declaration builds pass. App `pnpm run check` passes with zero diagnostics after building the existing Svelte-headless package and generating existing Paraglide outputs. All five changed Svelte modules pass the Svelte autofixer with no issues/suggestions.
- `cargo hack test --feature-powerset --all-targets`: passed all 10 root-package configurations. This is the required root command, not a claim of a workspace-wide powerset; configurations without a matching binary target report the expected no-op.
- `cargo clippy --workspace --lib -- -D warnings`, `cargo fmt --all -- --check`, and `python3 scripts/check_server_dependencies.py`: passed.

### Validation limits

- The broad Unified library run first encountered the existing small-stack limit; its larger-stack rerun exceeded the 1,100-second bound and was stopped. It was not repeated after the final user instruction. Full Unified/workspace acceptance is not claimed.
- The broad multi-package IdP batch reported three missing/unconfigured signer-fixture failures and a permission-evaluation 503. The permission test passes in isolation; the three signer tests are outside the migrated consumer implementation. Batch results are not claimed green.
- All-target workspace Clippy fails in existing tests (`items_after_test_module` in IdP OAuth config/Management Device/selection policy and `unused_io_amount` in existing HTTP fixtures). Library Clippy is clean; those unrelated test lints were not changed.
- `just crap` was attempted and stopped after 180 seconds while compiling coverage dependencies; no complexity/coverage result is claimed.
- Password Manager `pnpm run check` still reports its missing `$lib/paraglide/runtime` and missing local Vitest declarations. Its `project.inlang/settings.json` is absent, so its existing localization-generation prerequisite cannot run. Its five Storage tests pass in the app qualification and the migrated module has no reported type error.
- No production deployment, native GUI or cross-platform acceptance is implied.

### Audit exclusions

The Setup listener uses a framework OPTIONS request solely for native TLS observation, not an owner operation. Generic native/deep-link fetch bridges, explicit external OIDC providers, embedded test fixtures, browser navigation and non-HTTP WebSocket/Iroh/SQL/KV/filesystem protocols remain separate. The updated inventory records these distinctions and the actual operations at every migrated location.
