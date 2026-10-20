# Receipt 41 — Actual Management cleanup contract and generated SDK

## Scope, source and controls

MAIN checkout `/home/nathan/code/rust/aicacia/of`, branch `integration/ui-driven-node-bootstrap`, base `f288fc100` (`Batch signer trust conflict checks per operation`). Read `independent-transfer.md` and receipts 38–40 before execution. Inventory changes from the earlier transfer implementation are already present; no inventory weakening or invented route was needed.

The existing **16 dirty Rust files** and deleted `evidence/review-transfer-lifecycle.md` remain parent-owned, untouched and excluded from this commit. Before and after export/generation/test qualification, the literal `git --no-pager diff --binary -- '*.rs' | sha256sum` result is:

```
50964d97952fbebe7597f8307c3153deefbe469ee4572202ee9d1f200da67109
```

No Rust source, app source, dependency manifest/lock, provider, OS trust, real profile or external repository was changed. Sole Rust token was used only for the existing bounded current-router export, then **explicitly released after regeneration**. No other Rust build/test, broad sweep, powerset, CRAP or privileged operation ran. No long-running server, temporary fixture, hand-edited generated code or `.bak` file was used.

Complexity/risk/priority: small tooling/SDK cutover, high-priority contract reconciliation. Critical risks are losing the cleanup ID/current binding, treating pending cleanup as resource approval, wrong bearer/owner prefix, and manufacturing a successful acknowledgement from denial. Reuse the current exporter and generator; no new dependency or framework.

## Actual complete Management export

Exactly one Cargo command was run, working directory repository root, tool bound **200000 ms**, offline/locked, one job, incremental disabled, **16 MiB** stack, receipt-38 selective profiles:

```sh
OF_MANAGEMENT_OPENAPI_OUT=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/management-owner-openapi.json RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib router::openapi_tests::export_management_openapi_for_client_generation --config 'profile.dev.package.unified-server.debug=0' --config 'profile.dev.package.unified-server.opt-level=0' --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --include-ignored --exact --nocapture --test-threads=1
```

**PASS:** 1 passed, 39 filtered; build **3m 00s**, test **0.06s**. The existing exporter constructs isolated state, requests the actual composed `/management/openapi.json` router in process, asserts the complete contract and removes its temporary database. No listener or real provider is needed.

Updated artifact `management-owner-openapi.json` SHA-256:

```
974b52d369aaa230618a95ba3051d1d9ae292a281272ec6d01f4bc9ea067284c
```

Current inventory: **38 unique operations**. The artifact delta is only the new `OwnerCopyCleanup` schema, optional/nullable selection-response `pendingCleanup` and its description, and actual `POST /replication/owner-cleanup`. Owner-relative paths remain unprefixed; server base is `https://management.example/management`.

## Generation, SDK build and exact model reconciliation

Working directory `packages/management-client`:

```sh
OPENAPI_SPEC_URL=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/management-owner-openapi.json pnpm run openapi
pnpm run build
```

Both **PASS**, bounded respectively at 200000/120000 ms. Existing staged generator plus normal formatting hook produced source; TypeScript ESM/declarations and browser bundle compile. Rolldown result: **128.52 kB**, **142.28 ms**. The existing script inventory accepted `SuperRoutesReplicationApi: ownerCleanup replicationAdmission selectedResources`; no unrelated owner was regenerated.

Generated changes are confined to:

- `packages/management-client/src/apis/SuperRoutesReplicationApi.ts`: `OwnerCleanupRequest`, request options, raw and ordinary `ownerCleanup` methods.
- `src/models/OwnerCopyCleanup.ts` and model re-export: all five required camelCase strings `cleanupId`, `deviceId`, `endpointId`, `oldOwnerSubject`, `newOwnerSubject`.
- `src/models/SelectedResourcesResponse.ts`: `pendingCleanup?: OwnerCopyCleanup | null` and generated nested serializers; existing Device/endpoint/current-owner, `adminAllowed`, and resources remain required.

This exactly matches the current Rust wire shape. Pending response fixture uses the current new owner, `adminAllowed:false`, empty resources and the exact cleanup tuple. Absence serializes without `pendingCleanup`; explicit null stays null; a pending object retains all five fields. Generated field-presence helpers are not security validation. Service-only caller/audience/scope/current-binding authorization is still enforced by Rust; the SDK does not confer it.

The previous generated-source test compared JavaScript objects before JSON serialization and rejected the new optional `pendingCleanup:undefined` property. It was observed failing, then updated to assert undefined absence and the unchanged JSON wire policy. No production/generated code was changed to hide the optional field. A new built-package contract test covers schema inventory/shape, tuple preservation, pending/absent/null wire forms, exact owner transport, 204 acknowledgement and propagated 401/403/503 errors. Its first selections URL expectation was corrected against the actual route `/replication/devices/{endpoint_id}/selections`; no route or generated client was altered.

## Focused validation

All commands below completed within their stated tool bounds. No dependency installation occurred.

### Generator, source contracts and built SDKs

Working directory root, 120000 ms:

```sh
GENERATOR_TEST_BIOME=/home/nathan/code/rust/aicacia/of/node_modules/.bin/biome node --test scripts/generate-openapi-client.test.mjs scripts/generated-clients.test.mjs scripts/generated-management-client.test.mjs scripts/generated-setup-client.test.mjs scripts/generated-independent-client.test.mjs apps/app/src/lib/common/state/setupApi.test.mjs
```

**252 passed / 0 failed / 0 skipped**, **17.14s**. Includes real-Biome generator/failure-preservation cases, source owner-prefix/bearer/ownership checks, six new built Management contract tests, existing setup/independent SDK contracts and setup API tests.

Initial combined run had the optional-property test failure described above and an environment failure: MAIN lacked `packages/idp-client/esm/independent/index.js` required by the existing built-independent test. Ran only the unchanged IdP package's existing build, cwd `packages/idp-client`, bound 120000 ms:

```sh
pnpm run build
```

**PASS:** TypeScript/declarations and Rolldown **135.99 kB / 241.62 ms**. IdP was **not regenerated**, and it has no tracked source/package changes. The combined suite then passed without exclusions.

### App owner/setup state and routes

Working directory `apps/app`, 60000 ms:

```sh
/home/nathan/.npm/_npx/85c515f6a611c15f/node_modules/.bin/vitest run --config vitest.auth.config.ts src/lib/common/state/setupClient.test.ts src/lib/common/state/ownerClients.test.ts src/lib/common/state/independentSetup.test.ts src/lib/common/state/localhostBaseUrl.test.ts src/routes/setup/installation/page.test.ts 'src/routes/(auth)/layout.test.ts'
```

**158 passed, 6 files**, **2.60s**. Real generated Management source is used by owner consumers; existing setup/session, scope/recovery/cancel, login discovery, source audience and native-versus-remote selection regressions remain green. No frontend edits.

### Setup rendered interactions

Working directory `apps/app`, 60000 ms:

```sh
RESIDENCY_JSDOM_MODULE=/home/nathan/.npm/_npx/f2259fd4c4b894b7/node_modules/jsdom /home/nathan/.npm/_npx/85c515f6a611c15f/node_modules/.bin/vitest run --config vitest.residency.config.ts src/lib/common/components/independentSetup.interaction.test.ts
```

**4 passed**, **2.65s**: independent and bundled desktop Create/Join forms using HTTP/native boundary substitutes. These are frontend regressions, not a new native/TLS/provider qualification.

### Static checks and separately inherited Svelte errors

`node --check scripts/generated-management-client.test.mjs`, `node --check scripts/generated-clients.test.mjs`, scoped new-test Biome formatting, and `git --no-pager diff --check` pass.

Working directory `apps/app`, 120000 ms:

```sh
node_modules/.bin/svelte-check --tsconfig ./tsconfig.json
```

Exit **1**, **5 errors / 0 warnings in 2 files**, not a full app check pass. MAIN now has the generated Paraglide modules, so the historical 14 missing-module errors from receipt 35 were **not reproduced here**. Remaining inherited failures are the three missing Storage SDK exports (`ResidencyMode`, `ResourcesResidencyApi`, `StorageOwnerConfiguration`) in `ResourceResidency.svelte`, plus two missing Storage exports (`ResourcesResidencyApi`, `StorageOwnerConfiguration`) in `ownerClients.test.ts`. Storage was not built/regenerated or edited to fix these out-of-scope issues.

## Qualification boundary and handoff

Actual current Management contract export, generated SDK compilation and focused contract/frontend qualification pass. This does not rerun or supersede receipt-38/39/40 lifecycle, readiness, transfer, source-offline, provider or real HTTPS evidence. No ticket is closed. Parent Rust changes, Management policy/readiness/transfer fixes and the existing evidence deletion remain uncommitted parent work; only Management SDK source, focused script tests, the generated schema artifact and this receipt belong to this commit. Rust token has been released.

Commit hooks are explicitly disabled (`-c core.hooksPath=/dev/null`) to prevent automatic builds after token release; all scoped validation above ran explicitly. Final repeated combined suite: 252 passed / 0 skipped / 0 failed, 16.97s.
