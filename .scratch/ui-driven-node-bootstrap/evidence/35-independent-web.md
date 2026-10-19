# Receipt 35 — Independent web Create/Join and bundled HTTPS frontend

## Scope and provenance

Branch `implement/bootstrap-web`, worktree `.scratch/worktrees/bootstrap-web`, based on integration `f1ff45a0a`. Checkpoint `55f20fa80`; integration/native handoff `b09eee6cb` merged as `104f2555d`. Authoritative contracts: `docs/api.md` Independent Create/Join worker sections, `evidence/independent-join-security.md`, and `apps/app/src-tauri/NATIVE-SETUP.md` from that integration merge.

This slice owns frontend, generator scripts, the new generated independent SDK subpath, and minimal actual public-handler OpenAPI annotations/export. It does not implement workers, change service credential delivery, edit native Rust, or qualify OS trust/provider behavior. Full tickets remain open.

Complexity/risk/priority: high-priority security-boundary reconciliation. Reuse current setup admission and shared HTTPS worker, not a new framework or one Node per owner. Critical risks: wrong owner/code/password destination, unrelated ready states, recovered tuple races, stale scope, accidental native mutation, and confusing asynchronous source acceptance with completion.

## Delivered flow and pointers

- `apps/app/src/lib/common/components/IndependentSetup.svelte`: actual three-owner form, transient codes/password, explicit Create/Join/recovery/observation/cancel, per-owner partial outcomes, readiness navigation. Deployment OS CA trust and allowed UI Origin remain prerequisites; no certificate-warning bypass advice.
- `state/independentSetup.svelte.ts` and `state/independentSetupApi.ts`: one stable exact operation over three distinct canonical HTTPS bases. Each capability travels only in its own owner's Authorization header. Only IdP Create receives the transient password. Join sends empty administrator and initial issuer, no password; peer is exact `https://source/management#pinnedendpoint`. Backend authenticated discovery owns issuer resolution.
- Recovery checks every current scope/tuple, permits unbound awaiting-choice owners, reads public evidence for retired/ready owners rather than sending their codes, and revalidates completion against the newly recovered tuple. All ready evidence must match intent, issuer and all target bases. Three unrelated ready flags are insufficient. Cancel is per-bound-owner signaling, not rollback/cleanup completion; closing a page does not cancel host work.
- `state/setupApi.ts` and `state/oidc.svelte.ts`: independent public login discovery uses `/setup/independent/public` `facts.login_client` UUID, only after all-owner public evidence matches. Unified inline login metadata is preserved.
- `state/devices.svelte.ts` and `(auth)/devices/+page.svelte`: source acceptance/approval uses selected Management HTTPS with fresh ordinary User `idp-management` audience on every platform. Acceptance PUT 204 reads back; approval 200 means unified approved, 202 means independent accepted, not completion. Independent offers preserve separate Device/member/endpoint identities and exact target/source tuple.
- `state/idpClient.svelte.ts`, `managementClient.svelte.ts`, `setupClient.svelte.ts`: selected remote bases are not replaced by bundled local APIs. Bundled desktop uses the same HTTPS setup client and form; explicit native code retrieval checks exact bundled target both before and after retrieval. Remote selection cannot retrieve a local capability or reset the local Node. Exact bundled Tauri origins are allowed only for bundled selection; remote/web setup still requires HTTPS UI Origin.
- `routes/setup/installation/+page.{ts,svelte}` and `components/NativeSetupRecovery.svelte`: removed native synchronous Create/Join/cancel/recover/source mutation calls. Native remains explicit observation, protected code retrieval, prerequisite retry/trust/reset/reference/peer-revocation support. Public Node readiness leads to local Device marker completion, not authentication enablement. Failed native observation/startup remains an actionable recovery view.
- `state/localhostBaseUrl.svelte.ts`: `/idp/setup/status`, no obsolete `/lidp/setup/status` fallback.
- `(auth)/+layout.ts`: removed final obsolete native `get_setup_intent`; selected IdP HTTPS OIDC metadata supplies issuer comparison. Discovery unavailable/malformed denies access while retaining the session. Remote desktop auth regression verifies no unrelated native calls.

## Actual OpenAPI export and generated SDK

`crates/api/src/independent_openapi.rs` publishes annotations on existing handlers, not invented routes. Exactly seven owner-relative browser paths:

```
/setup/status
/setup/admission
/setup/create
/setup/join
/setup/intent
/setup/cancel
/setup/independent/public
```

Internal challenge/register/history exchange is deliberately excluded. Independent setup is separate from unified generated setup (`@aicacia/idp-client/independent`, generator service `idp-independent`). Operations: `admission cancel create join ownerPublic recover status`. Main IdP generation preserves both setup subtrees. No existing IdP/Management/Storage generated source was regenerated or hand edited; the independent subtree was produced by the existing generator workflow. The app consumes actual generated request/intent serializers (`operationNonce` -> `operation_nonce`). No new dependencies.

Retained export: `independent-setup-openapi.json` beside this receipt. SHA-256:

```
f755cbab0ca67e9ae75008c36d9afa4439d786c99b4fd891a5f7fecb6ee74de5
```

Sole Rust token was granted for this bounded export and explicitly released after generation. No further Rust builds were run. No temporary mock exporter was needed: existing public-handler annotation/export is sufficient. Literal export command (worktree root, 200000 ms tool bound):

```sh
OF_INDEPENDENT_SETUP_OPENAPI_OUT=/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-web/.scratch/ui-driven-node-bootstrap/evidence/independent-setup-openapi.json CARGO_TARGET_DIR=/home/nathan/code/rust/aicacia/of/target RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p api --lib independent_openapi::tests -- --include-ignored --nocapture --test-threads=1
```

Result: **2 passed**, 32 filtered, build 38.26 s; tests 0.00 s. This qualifies schema export/path inventory, not worker integration or native runtime.

Generation and package build, cwd `packages/idp-client`:

```sh
OPENAPI_SPEC_URL=/home/nathan/code/rust/aicacia/of/.scratch/worktrees/bootstrap-web/.scratch/ui-driven-node-bootstrap/evidence/independent-setup-openapi.json pnpm run openapi:independent
pnpm run build
```

Both passed. Final repeated package build also passed (`tsc`, Rolldown 135.99 kB, 95.58 ms). Generated contract dependency is delivered here, not deferred to hand editing or a future parent regeneration. Management/Storage SDK source is untouched; unrelated existing Storage package-export blockers remain below.

## Test evidence

TDD used at the setup session/HTTP, source-owner client, route-load, and rendered form seams. Red observations included missing independent adapter, singleton IdP inline-login assumption, recovery completion-switch race, independent acceptance 204, desktop remote-target overwrite, bundled native Create gate, and the last auth-layout removed command. A focused auth regression failed with `Unexpected IPC command: get_setup_intent` before replacing that call. Old native page expectations were replaced with shared HTTPS/native-observation expectations, not skipped.

Final state/route command, cwd `apps/app`:

```sh
/home/nathan/.npm/_npx/85c515f6a611c15f/node_modules/.bin/vitest run --config vitest.auth.config.ts src/lib/common/state/setupClient.test.ts src/lib/common/state/ownerClients.test.ts src/lib/common/state/independentSetup.test.ts src/lib/common/state/localhostBaseUrl.test.ts src/routes/setup/installation/page.test.ts 'src/routes/(auth)/layout.test.ts'
```

**158 passed, 6 files**, 1.15 s: setup session 62, owner clients 21, independent session 16, localhost 7, installation page 24, auth layout 28. Covers web without Tauri, partial errors, exact retry/recovery, scope drift, cancellation, issuer/readiness mismatch, HTTP destinations/audiences and remote desktop selection.

DOM command, cwd `apps/app`:

```sh
RESIDENCY_JSDOM_MODULE=/home/nathan/.npm/_npx/f2259fd4c4b894b7/node_modules/jsdom /home/nathan/.npm/_npx/85c515f6a611c15f/node_modules/.bin/vitest run --config vitest.residency.config.ts src/lib/common/components/independentSetup.interaction.test.ts
```

**4 passed**, 1.67 s. Real Svelte form events for independent Create/Join and bundled desktop Create/Join; exact request bodies, owner codes, IdP-only password, transient clearing, correlated readiness navigation, native protected code retrieval, and no removed native commands. HTTP/native are boundary substitutes; this is not a real browser/desktop/TLS/provider acceptance run.

Generator and built SDK command, worktree root (after final package build):

```sh
GENERATOR_TEST_BIOME=/home/nathan/code/rust/aicacia/of/node_modules/.bin/biome node --test scripts/generate-openapi-client.test.mjs scripts/generated-clients.test.mjs scripts/generated-setup-client.test.mjs scripts/generated-independent-client.test.mjs apps/app/src/lib/common/state/setupApi.test.mjs
```

**245 passed, 0 skipped, 0 failed**, 19.03 s. Includes generator inventory/failure-preservation tests, built normal/setup/independent contracts and 24 setup API tests. Independent SDK tests verify actual schema path inventory, exact owner-relative transport, code isolation, serializers, empty Join admin/issuer, no Join password, public requests without capabilities and 202/204 semantics.

Cached `svelte-mcp svelte-autofixer` run on IndependentSetup, NativeSetupRecovery, installation and Device pages, independent/setup/devices/IdP/Management/localhost/OIDC state modules: no issues or suggestions. Scoped Biome formatting used without changing generated source. `git diff --check` passed.

## Inherited blockers and qualification limits

Literal cwd `apps/app` command:

```sh
node_modules/.bin/svelte-check --tsconfig ./tsconfig.json
```

Exit **1**, **19 errors / 0 warnings in 15 files**, unchanged inherited categories:

- **14** missing generated Paraglide modules: `$lib/paraglide/runtime`, `$lib/paraglide/server`, `$lib/paraglide/messages`; hooks, locale, ConfigForm, app/auth/config/policy/signin/terms routes.
- **5** missing built Storage SDK exports: `ResidencyMode`, `ResourcesResidencyApi`, `StorageOwnerConfiguration` in ResourceResidency (3); ResourcesResidencyApi and StorageOwnerConfiguration in ownerClients tests (2). Focused tests resolve workspace Storage source; installed package export typing remains stale.

No new setup/auth/source diagnostics. Full app build/whole-repo checks are not claimed. No broader Rust/native builds, live workers, physical peer discovery, real OS trust/CA/provider changes, actual native GUI Create/Join, or new full HTTPS integration qualification. Parent/native receipt qualification remains separate. Full tickets stay open.

Temporary dependency symlinks used solely for local tests/build are removed before commit. Incidental pnpm rewrites of tracked legacy Storage executable shims are restored, not included. No `.bak` files or secret browser storage added.
