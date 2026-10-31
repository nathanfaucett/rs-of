# Ticket 01 validation

Worktree: `/home/nathan/code/rust/aicacia/of-owner-setup-surface`
Branch: `unified-owner-lifecycle-01`
Base: `unified-owner-lifecycle` at `2d1105a6e5a6e14d7e05a9cacdec376959fd0e3a`.

## Scope and decisions

- The full owner-relative surface is `/setup/owner/{status,admission,create,join,intent,cancel,public,challenge,register,history}`. A standalone owner uses its origin; composition adds the configured owner prefix once.
- Both `independent` path segments are removed, including from worker handoffs. There are no compatibility routes or generated operation aliases.
- Consolidate the previously separate neutral admission/status description and worker description into one `owner_setup` operation group (`OwnerSetupApi`), with `OwnerSetupStatus`. Delete obsolete generated modules/models and the redundant neutral OpenAPI builder.
- This uses the existing surface-named namespace without colliding with the still-existing host-coordinated `/setup/...` contract. Replacing that lifecycle and wiring all workers into Unified Host belongs to ticket 02; no temporary host rename is introduced here.
- Complexity is low (route/operation renaming, no lifecycle algorithm change); reach/risk is moderate because generated clients, callbacks, probes and callers must move atomically. No dependency or compatibility layer is added.

## Environment and command notation

Commands below ran in the worktree unless another directory is stated. Rust commands used `CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target` to reuse build artifacts. The editor invoked external-worktree commands through `env -C /home/nathan/code/rust/aicacia/of-owner-setup-surface`; this wrapper does not change their arguments. Generation ran serially. The cross-layout suite itself used `--test-threads=1`.

## Red/green

1. `cargo test -p api owner_setup_surface_uses_the_owner_prefix_in_both_layouts -- --exact --nocapture` selected zero tests because the function is module-qualified; this is not red/green evidence.
2. `cargo test -p api owner_setup_surface_uses_the_owner_prefix_in_both_layouts -- --nocapture` failed before implementation: the new root owner status returned 503 instead of 200.
3. The same command passed after changing the authoritative route definitions.
4. The retained test now covers all three owners, empty and composed prefixes, exact status documents, invalid/valid admission (401/204), and 404 for all ten previous routes after the normal owner router is published. Existing lifecycle tests were moved to the renamed paths rather than duplicated.

## Passing validation

| Exact command | Result |
| --- | --- |
| `pnpm install --offline --frozen-lockfile --ignore-scripts` | Cached dependencies installed; incidental tracked node_modules launcher changes were restored. |
| `cargo run -p unified-server --example export-openapi -- openapi` | Canonical documents exported from authoritative Rust registrations. |
| `pnpm --filter @aicacia/idp-client run build:esm` | Bootstrap SDK prerequisite built. |
| `pnpm openapi` | All four TypeScript/Rust SDK pairs generated and checked; no manual generated edits. Repeated successfully with SHA-256 comparison of canonical documents and both SDK trees: byte-for-byte unchanged. |
| `pnpm sdk:build` | All four SDK packages built. |
| `node --test scripts/generated-*.test.mjs` | 32 passed. |
| `node --test scripts/owner-contracts.test.mjs` | 3 passed. |
| `node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs` | Passed. |
| `pnpm --filter @of/app exec vitest run --config vitest.auth.config.ts src/lib/common/state/independentSetup.test.ts src/lib/common/state/ownerClients.test.ts` | 44 passed. |
| `pnpm --filter @of/app exec vitest run --config vitest.residency.config.ts src/lib/common/components/independentSetup.interaction.test.ts` | 9 passed. |
| `cargo test -p api -- --test-threads=1` | Final run: 44 passed, 2 intentionally ignored child entries; 1 doctest passed. |
| `cargo test -p management-server -p storage-server --lib openapi` | 8 passed. |
| `cargo test -p unified-server --test composition` | 2 passed. |
| `cargo clippy -p api --all-targets --no-deps -- -D warnings` | Passed. |
| `cargo fmt --all -- --check` | Passed. |
| `git --no-pager diff --check` | Passed. |
| `cargo build -p idp-unified --example system-test-service -p idp-server --bin identity-database` | Passed on retry after an initial 200-second compilation timeout. |
| `cargo test -p idp-unified --test system_create unified_retained_restart -- --ignored --exact --test-threads=1 --nocapture` | Passed. |
| `cargo hack test --feature-powerset --all-targets` (in sibling `offs`, its normal target directory) | Passed; no filesystem code changed. |

`cargo test -p unified-server generated_` also exited successfully but selected zero tests; it is not counted as behavioral coverage.

## Broader gates: failures and limits, not passes

- `pnpm openapi:check`: final completed run passed both deterministic exports, both complete regeneration/digest comparisons, SDK builds and generated/owner contract tests, then failed the two existing cold-cache generator assertions requiring `cargo fetch --locked`. The unchanged generator currently calls fetch without that flag. `node --test scripts/generate-openapi-client.test.mjs` reproduced those same assertions in the untouched integration worktree. An earlier 600-second check attempt timed out during regeneration under concurrent heavy builds; its temporary staging directory was removed.
- `node --test scripts/generate-openapi-client.test.mjs scripts/generated-*.test.mjs`: initially also exposed the expected Device operation-count refresh (50 to 48, because neutral admission/status are no longer duplicated). Both Rust and JavaScript inventory expectations were updated; generated tests now pass. The unrelated cold-cache assertions above remain.
- `cargo test -p idp-server -p management-server -p device-server -p storage-server --lib openapi`: Device passes all 3 after refreshing its inventory. IdP passes 7, ignores 1 explicit export helper, and fails its unchanged raw router inventory (40 operations versus expected 39). Cargo stops before Management/Storage, which were run separately and passed.
- `cargo test -p idp-unified --test owner_boundaries owner_composition_tests -- --test-threads=1`: 4 passed, 4 failed, 6 ignored. Create/remote setup fail the existing normal-router CORS assertion; its exact failure was reproduced on the untouched integration branch using `cargo test -p idp-unified --test owner_boundaries production_independent_create_and_offline_reopen -- --test-threads=1`. The delayed-authority operation case also failed. No unrelated CORS or lifecycle changes were made.
- `cargo test -p idp-unified --test system_create -- --ignored --test-threads=1 --nocapture`: first run passed independent Create, Join/source-offline reopen, local-owner Create/restart, permission denial and retained restart, plus Unified Create and permission denial. Unified Join failed its documented N0 Trusted Peer discovery network preflight; the overall 600-second limit interrupted the final Unified restart. That restart passed separately as recorded above. A second overall attempt passed independent Create but failed independent Join on a Management process exit and timed out in the next scenario under concurrent heavy builds. This is not a green nine-test deployment qualification.
- `pnpm --filter @of/app exec vitest run --config vitest.auth.config.ts`: final serial run is 178 passed / 10 failed, exactly matching the untouched integration baseline: eight auth-layout denial/redirect assertions plus two existing session/PKCE assertions. Initial fresh-worktree attempts also lacked generated SvelteKit configuration; `pnpm --filter @aicacia/svelte-headless exec svelte-kit sync` and `pnpm --filter @of/app exec svelte-kit sync` resolved that prerequisite. A heavily loaded intermediate run had additional timeout failures; the final serial run is the meaningful comparison.
- `cargo hack test --feature-powerset --all-targets` (of worktree): fails compiling unchanged `tests/unified_identity_composition.rs` / `tests/support/identity.rs`: missing `super::client_onboarding` and missing `UnifiedEndpoint::open_fresh`.
- `cargo hack test --feature-powerset --workspace --all-targets`: attempted separately; timed out after 200 seconds compiling a feature combination, so full workspace qualification is incomplete.
- `just crap`: attempted with the shared target directory; timed out after 600 seconds during instrumented workspace compilation. No CRAP score was produced.
- `cargo clippy -p api --all-targets -- -D warnings`: dependency linting failed on 95 generated IdP SDK lint errors. The focused `--no-deps` check passed without suppressions or manual generated-code edits.
- `cargo test -p api`: passed repeatedly, but a final parallel run intermittently failed the existing admission-lock-release test (43 passed / 1 failed / 2 ignored). The final serial run passed all 44 runnable tests.

No full-suite, production network, real credential-provider, OS trust installation, or GUI certification is claimed. Ticket 01's route/SDK implementation is complete; baseline and environmental qualification failures are retained explicitly rather than fixed outside scope.
