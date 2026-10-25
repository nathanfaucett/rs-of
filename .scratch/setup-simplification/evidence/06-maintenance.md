# Ticket 06: separate maintenance evidence

## Scope, risk and seams

- Worktree `of-setup-06`, branch `setup-simplification-06`, based on integration `23e9006ca`. Claimed before implementation; read AGENTS, tracker, spec, ticket 01 and its evidence. Used TDD and both Svelte skills. Spec-authorized rendered-page/native-adapter seams; no interview or new test framework/dependency.
- Medium presentation complexity, high destructive-operation/security risk, high priority. Keep proven native reset/revocation implementation unchanged. Ticket 05 owns startup lifecycle; tickets 02/03 own Create/Join. Only minimal Installation parent wiring changed.
- Added `/maintenance`, retaining existing recovery component as its dedicated presentation. Removed inline onboarding controls and duplicate maintenance runtime retry. Settings links directly to maintenance. Repair remains reachable, ordinary errors retain prerequisite/observation/runtime retry.
- Reset discloses local consequences, requires `RESET` for every attempt, shows progress and failure, preserves exact retry, and separately refreshes surviving public revocation targets. Advanced exact-reference recovery remains distinct/collapsed and cannot automatically reset. Remote/browser route has no local mutation controls; native adapter also guards selection.
- Removed ambient OIDC bearer fallback: peer revocation requires explicit transient User bearer, cleared before native invocation. Issuer pinning, per-owner exchanges, both-owner acknowledgement, provider-first deletion, retained locks/journals and durable data-only cleanup remain existing native responsibilities; no runtime/security implementation rewritten.

## Red → green

1. Rendered reset test failed on missing local-consequence disclosure in the old inline presentation. It now verifies disabled reset before confirmation, failed provider response, cleared confirmation, explicit second attempt and local-complete/remote-pending wording.
2. Remote-selection refusal test exposed initial asynchronous target-read failure overwriting the operation's actionable error. Target-load errors now preserve an existing operation message; test passes. Failed target refresh after successful cleanup reports uncertain revocation status, not failed local deletion.
3. Added rendered coverage for exact-reference conflicts without reset, missing explicit bearer without ambient-session lookup, partial owner failure retaining exact target, cleared bearer, both-owner acknowledgement, and browser/remote route exclusion with return navigation.
4. Updated existing setup rendered expectations to assert `/maintenance` navigation and absence of destructive/recovery/revocation buttons rather than the superseded inline summary.

## Validation

Commands are relative to worktree except where specified. All DOM tests use the existing residency environment, with `RESIDENCY_JSDOM_MODULE` pointing to the already installed disposable jsdom under sibling ticket 04's `node_modules/.cache/source-approval-test-tools/node_modules/jsdom`. This is test tooling only; no sibling source/profile/trust changes and no dependency/lockfile changes committed.

- `pnpm install --offline --frozen-lockfile`: passed.
- `pnpm packages:build`: passed. Generated tracked Storage/browser/bin artifacts restored afterwards.
- `CI=1 ... pnpm exec vitest run --config vitest.residency.config.ts --maxWorkers=1 --reporter=dot`: **21 passed, four files**, including seven maintenance cases and existing Create/Join/source-approval/residency interactions.
- `pnpm exec vitest run --config vitest.auth.config.ts src/routes/setup/installation/page.test.ts --maxWorkers=1`: **28 passed**.
- Broad auth suite is **not consistently green**: latest complete run **179 passed / two failed** in unchanged `setupClient.test.ts` (Join request/serialization expectations, including expected one fetch but observed two). Earlier loaded/concurrent runs included unrelated auth/Storage failures/timeouts; one bounded rerun timed out. No claim of broad-suite qualification or independently established baseline for those failures; no unrelated tests changed.
- `pnpm build`: passed, static app generated. Initial check needed generated localization; build supplies it. Final `pnpm check`: **zero errors, zero warnings**.
- Svelte MCP autofixer on maintenance component and new route: no issues/suggestions. Installation/root layout: no issues, suggestions on existing polling/theme effects (lifecycle timer cleanup and external theme/body synchronization remain effects, not derived values).
- `git diff --check`: passed.
- `cargo test --manifest-path .../Cargo.toml -p lidp --lib setup_reset`: **timed out compiling** at 200 seconds, twice; no current-tip native test result. The chained peer-revocation command was not reached. Existing provider-refusal, missing/conflicting reference, interrupted data-only cleanup, original Join IDs and issuer/partial-peer tests remain unchanged, not claimed rerun green.
- `timeout 60s cargo hack test --manifest-path .../Cargo.toml --feature-powerset --all-targets`: timed out compiling; full matrix unqualified.
- `timeout 60s just --justfile .../justfile --working-directory ... crap`: timed out compiling coverage dependencies; no CRAP score. Initial invocation omitted required `--justfile` and was corrected.
- No changed Rust source; no new Clippy result. No real profiles, secure stores, OS trust/elevation, native GUI or remote erasure tested. Injected UI adapters qualify presentation, not actual native lifecycle safety.

## Integration

`git merge setup-simplification` before completion: **Already up to date**, integration tip `23e9006ca`. No main-worktree edits; its pre-existing Cargo.lock modification left intact. Worktree-generated Cargo.lock changes restored. Native lifecycle implementation untouched. Operations and native handoff document the separate maintenance path and qualification boundaries.
