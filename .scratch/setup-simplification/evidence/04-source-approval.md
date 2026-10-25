# Source approval validation

Branch: `setup-simplification-04`. Base/integration: `setup-simplification` at `dfeaf7dbe`.
Worktree: `of-setup-04` only. No changes to observation, startup, setup workers, or authorization contracts.

## TDD and scope

Used the existing rendered-flow DOM harness and actual `devices.svelte.ts` owner-client adapters (mocked HTTPS responses and owner-token/target configuration boundary, not mocked approval functions). The first test failed before `SourceApproval.svelte` existed. The partial-failure test then failed because refresh removed the decision error; retaining decision failure and the exact offer made it pass. Six rendered cases cover acceptance open/close and trusted-peer instructions, two-offer exact selection for unified/independent approval and rejection, 202 versus readiness, 200 source acknowledgement, persistent partial failure/exact retry, and 401/403 responses. Existing owner-client tests cover Management token exchange, insecure/missing auth refusal, target changes, exact IDs and independent routing. Existing Device policy and permissions code is unchanged.

## Reproduction prerequisites

`pnpm install --offline --frozen-lockfile`; `pnpm packages:build`; compile app Paraglide output before `pnpm check`.
The existing DOM harness requires external jsdom: this run installed `jsdom@26` locally under the ignored `node_modules/.cache/source-approval-test-tools` directory using npm `--no-save --no-package-lock`. Set `RESIDENCY_JSDOM_MODULE` to that directory's absolute `node_modules/jsdom` path for rendered tests. No production or workspace dependency was added.
Paraglide compilation warned that two CDN plugins could not be fetched, but generated enough output for the check to complete. Generated package/browser/bin churn was restored and is not part of this ticket.

## Passing focused evidence

### `pnpm exec vitest run --config vitest.residency.config.ts src/lib/common/components/sourceApproval.interaction.test.ts src/lib/common/components/residency.interaction.test.ts`

Exit: 0

```text
 Test Files  2 passed (2)
      Tests  9 passed (9)
```

### `pnpm exec vitest run --config vitest.auth.config.ts src/lib/common/state/ownerClients.test.ts`

Exit: 0

```text
 Test Files  1 passed (1)
      Tests  27 passed (27)
```

### `pnpm check`

Exit: 0

```text
svelte-check found 0 errors and 0 warnings
```

### `npx --yes @sveltejs/mcp svelte-autofixer ./src/lib/common/components/SourceApproval.svelte`

Exit: 0

```text
  issues: [],
  suggestions: [],
  require_another_tool_call_after_fixing: false
```

### `npx --yes @sveltejs/mcp svelte-autofixer ./src/routes/(auth)/devices/+page.svelte`

Exit: 0

```text
  issues: [],
  suggestions: [],
  require_another_tool_call_after_fixing: false
```

## Validation limitations (not claimed passing)

- Reused existing public HTTPS Join lifecycle seam with `RUST_MIN_STACK=67108864 cargo test -p unified-server --lib production_api_join_interruption_preserves_private_host_intent -- --nocapture`. It contains real source wrong-audience refusal, target Setup-capability refusal, forged approval, exact-offer approval/cancel/retry and retained identity checks. This run reached the 120-second tool bound during dependency compilation, before tests executed. No new approval runtime or Rust fixture was introduced; runtime/security qualification is not claimed.
- Required final `RUST_MIN_STACK=67108864 cargo hack test --feature-powerset --all-targets` reached the 120-second bound while compiling; no feature-matrix pass is claimed.
- Required final `RUST_MIN_STACK=67108864 just crap` reached the 120-second bound while compiling coverage dependencies (process killed); no complexity score is available.
- Broader `pnpm exec vitest run --config vitest.auth.config.ts`: 177 passed, 24 failed, six files failed/one passed. Failures included setup-client timeouts/exact request counts and owner-client Residency call expectations during the concurrent broad runs. These files were not changed. The owner-client suite passes independently as above. No baseline attribution or broad-suite success is claimed.
- No real OS trust-provider/native GUI, WAN, or two-physical-device completion qualification. HTTP response tests do not prove backend enrollment behavior. Source acknowledgement deliberately does not claim joining-owner readiness.
