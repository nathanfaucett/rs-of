# Owner-client cutover — actual OpenAPI regeneration and app contract qualification

Date: 2026-10-19. Base: `e0508952fb0f3d3a33297a6054cd6546eda3bfa7` on `integration/ui-driven-node-bootstrap`, main `of` checkout, Ubuntu Linux x86_64.

## Scope and disposition

Regenerated the actual current IdP and Management owner contracts through the existing staged OpenAPI generator workflow. No generated client was hand-edited. The owner schemas are retained as `idp-owner-openapi.json` and `management-owner-openapi.json` beside this receipt. Complexity is low/moderate; the important risks are stale ownership, incomplete router exports, wrong audiences/prefixes and accidentally changing the separate setup contract.

- IdP exports **38 operations**: User/Client/OAuth contracts, active-User lookup and independent signer management, not Device authority. Generation produces `SuperRoutesActiveUserApi`; obsolete `SuperRoutesDevicesApi` and Device models are removed.
- Management exports **37 operations**, including Device discovery, enrollment, lifecycle, transfer and endpoint lookups, alongside its existing policy operations. Generated Device methods/types now satisfy the app's moved imports and calls.
- `scripts/generate-openapi-client.mjs` and its fixture inventories now require the actual owner operations/groups. The stale-transfer rejection test belongs to Management, not IdP. `scripts/generated-clients.test.mjs` asserts the ownership split and exercises escaped owner paths, configured bearer transport, Management transfer and IdP active-User lookup. The optional live qualification runner's counts and ownership assertions are also updated; that live runner was not executed.
- Both client packages built successfully, including TypeScript ESM/declarations and browser bundles. The existing `postopenapi` hook formats package metadata and generated sources with the current formatter; this accounts for formatting churn. It also reformats the copied setup subtree. All **12 setup TypeScript files transpile to identical JavaScript** before/after, and separate setup-client contract tests pass. No setup schema was regenerated or public setup behavior changed.
- No app source edits were necessary. The previously excluded real Management Device client test now passes against actual regenerated source; no mocks/shims were added to manufacture missing methods.

This is client/contract qualification, not a fresh whole-HTTPS/native lifecycle run. [Receipt33](33-management-authority-cutover.md) records the parent's integrated Rust/native/HTTPS results. Full tickets 02/03/04 remain open. Independent-owner Create/Join workers, relationship-credential delivery, deployment certification and remaining lifecycle work are not delivered by this receipt. `/management/node-joins/...` is host-composed source coordination, not an invented standalone-owner OpenAPI operation; existing app tests exercise that routing separately.

## Actual-router schema export

Management used the existing ignored `router::openapi_tests::export_management_openapi_for_client_generation` test. It requests `/management/openapi.json` through the actual router with isolated state, asserts the complete 37-operation contract and writes the requested public document. No HTTP server is launched.

IdP had no complete equivalent export procedure. The user explicitly approved a **temporary untracked integration-test fixture**, `crates/idp-server/tests/temporary_owner_openapi_export.rs`, and granted the sole Rust build token for export. The fixture used public `openapi_router`/`RouterState`, an isolated empty `db::Database`, the actual `OAuth2Service` and concrete repositories, and `PrivateKeyKeyringRepo::new_with_store` backed only by a `keyring_core::mock::Store` wrapper following existing tests. Its ephemeral Iroh endpoint used `presets::Minimal` and an in-memory generated key. It made an in-process `tower::ServiceExt::oneshot` request to `/idp/openapi.json`, bounded the response to 4 MiB, asserted the server base, OAuth token/active-User paths and absence of Device routes, and wrote the actual returned JSON. It closed the endpoint and removed its temporary database root. There was no identity seeding, real provider/trust installation or long-running HTTP server. The fixture and its empty tests directory were removed immediately after successful export; **no tracked Rust source is changed**.

Only the following two Cargo commands were run for this client slice. No other Rust build, test, lint or broad feature sweep was run.

### Management export

Working directory: `of`. Terminal bound: **200000 ms**.

```sh
OF_MANAGEMENT_OPENAPI_OUT=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/management-owner-openapi.json RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p management-server --lib router::openapi_tests::export_management_openapi_for_client_generation --features management-service/replica --config 'profile.dev.package.storage-server.debug=0' --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.management-server.debug=0' -- --ignored --exact --nocapture --test-threads=1
```

Exit 0: **1 passed**, 36 filtered out; build 0.42s, test 0.06s.

### Approved temporary IdP export

Working directory: `of`. Terminal bound: **200000 ms**. The command requires the temporary fixture described above; it is not a retained public export command.

```sh
OF_IDP_OPENAPI_OUT=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/idp-owner-openapi.json RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo test --offline --locked -p idp-server --test temporary_owner_openapi_export --config 'profile.dev.package.idp-server.debug=0' --config 'profile.dev.package.idp-server.opt-level=0' --config 'profile.dev.package.management-server.debug=0' -- --exact export_actual_idp_router --nocapture --test-threads=1
```

Exit 0: **1 passed**, no filtered tests; build 19.18s, test 0.04s. Fixture removed afterward. The retained schema makes subsequent client regeneration independent of reconstructing the temporary fixture.

## Literal generation and package build commands

Each terminal bound: **60000 ms**. Commands within an owner were sequential. Only the two named client packages were built; no app/workspace build or Storage regeneration was run.

Working directory: `of/packages/management-client`:

```sh
OPENAPI_SPEC_URL=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/management-owner-openapi.json pnpm run openapi
pnpm run build
```

Both exit 0. Generation uses the existing cached OpenAPI Generator **7.25.0**, not a fabricated schema or hand-modified SDK. Build runs the existing `clean`, `build:esm` (`tsc`) and `build:browser` (`rolldown -c`) scripts. Browser result: 125.50 kB, Rolldown 1.2.1, 163.02ms.

Working directory: `of/packages/idp-client`:

```sh
OPENAPI_SPEC_URL=/home/nathan/code/rust/aicacia/of/.scratch/ui-driven-node-bootstrap/evidence/idp-owner-openapi.json pnpm run openapi
pnpm run build
```

Both exit 0. Browser result: 135.99 kB, Rolldown 1.2.1, 181.45ms. The generator reported `Could not compute datatypeWithEnum from AuthorizationDetail, null`; generation and TypeScript compilation still succeeded. No warning suppression or generated-code repair was applied.

## Focused tests and checks

### Generator and generated owner/setup contracts

Working directory: `of`. Bound: **60000 ms**.

```sh
GENERATOR_TEST_BIOME=/home/nathan/code/rust/aicacia/of/node_modules/.bin/biome node --test scripts/generate-openapi-client.test.mjs scripts/generated-clients.test.mjs scripts/generated-setup-client.test.mjs
```

Exit 0: **191 passed / 0 failed / 0 skipped**, duration 13364.451144ms. Explicit Biome selection runs the four invalid-staged-TypeScript refusal tests rather than skipping them. Existing failure-atomicity, missing group/operation, prefix, setup separation and source-preservation checks remain. The generated-owner transport tests execute actual generated TypeScript in memory; generated setup tests consume the built separate package subpath.

### App owner, setup session and Installation page

Working directory: `of/apps/app`. Bound: **60000 ms**.

```sh
/home/nathan/.npm/_npx/85c515f6a611c15f/node_modules/.bin/vitest run --config vitest.auth.config.ts src/lib/common/state/ownerClients.test.ts src/lib/common/state/setupClient.test.ts src/routes/setup/installation/page.test.ts
```

Exit 0: **3 files passed; 103 passed / 0 failed / 0 skipped**, total 1.26s. Breakdown: owner clients 19, setup session 59, Installation page 25. No exclusion remains for `uses Management Device client routes and audience for list, rename and revoke`. It checks actual generated methods, Management paths/prefixes, PATCH JSON and fresh `idp-management` audience exchanges. Source-approval tests cover configured prefixed/unprefixed Management bases, changed-target refusal, native command audience and ordinary User bearer handling, not target setup admission.

The preceding owner-only invocation also passed all **19** tests, 1.36s. The former worktree `@aicacia/native-fetch` resolution blocker does not reproduce in this main checkout.

### App setup wire contract

Working directory: `of`. Bound: **30000 ms**.

```sh
node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
```

Exit 0: **24 passed / 0 failed / 0 skipped**, duration 183.101744ms. Discovery/admission assertions for independent owners do not prove independent Create/Join mutations work.

### Built public exports

Working directory: `of/apps/app`. Bound: **10000 ms**.

```sh
node --input-type=module -e 'import assert from "node:assert/strict"; import * as idp from "@aicacia/idp-client"; import * as management from "@aicacia/management-client"; import * as setup from "@aicacia/idp-client/setup"; assert.equal(typeof management.DefaultApi.prototype.listDevices, "function"); assert.equal(typeof management.DefaultApi.prototype.transferDevice, "function"); assert.ok(management.DeviceState); assert.equal(idp.DeviceState, undefined); assert.equal(idp.DefaultApi.prototype.listDevices, undefined); assert.equal(typeof idp.SuperRoutesActiveUserApi.prototype.activeUser, "function"); assert.equal(typeof idp.SuperRoutesReplicaSignersApi.prototype.enrollReplicaSigner, "function"); assert.equal(typeof setup.JoinRequestToJSON, "function"); console.log("Built public package exports: Management Devices, IdP active User/signer, separate setup passed");'
```

Exit 0. These checks import built package exports, not source aliases.

### Setup preservation after the existing formatting hook

Working directory: `of`. Bound: **10000 ms**. Executed before committing, with HEAD still the recorded base.

```sh
node --input-type=module -e 'import {createRequire} from "node:module"; import {execFileSync} from "node:child_process"; import {readFileSync} from "node:fs"; import assert from "node:assert/strict"; const ts=createRequire(new URL("./packages/idp-client/package.json", import.meta.url))("typescript"); const files=execFileSync("git",["--no-pager","ls-files","packages/idp-client/src/setup"],{encoding:"utf8"}).trim().split("\n").filter(p=>p.endsWith(".ts")); const normalize=s=>ts.transpileModule(s,{compilerOptions:{target:ts.ScriptTarget.ES2021,module:ts.ModuleKind.ESNext,removeComments:true}}).outputText; for(const p of files)assert.equal(normalize(readFileSync(p,"utf8")),normalize(execFileSync("git",["--no-pager","show","HEAD:"+p],{encoding:"utf8"})),p); console.log("Setup subtree preserved after formatter: "+files.length+" TypeScript files have identical transpiled JavaScript");'
```

Exit 0: **12 files**, identical transpiled JavaScript. This is a semantic preservation check, not a claim of byte-identical formatting.

### Syntax, whitespace and Rust-source scope

Working directory: `of`. Bound: **10000 ms**.

```sh
node --check scripts/qualify-openapi-clients.mjs
node --check scripts/generate-openapi-client.mjs
git --no-pager diff --check
git --no-pager diff --name-only -- crates apps/app
```

Exit 0; no whitespace diagnostics, no tracked Rust or app source changes. The optional live qualification runner was syntax-checked, not run against a new server. No full frontend typecheck, rendered GUI, actual OS trust installation, real provider or native/whole-HTTPS runtime qualification was attempted in this slice.

## Failure and retry history

- Initial inventory-only tests (`node --test --test-name-pattern='fixture inventory matches current Rust' scripts/generate-openapi-client.test.mjs`) failed the stale IdP/Management inventories. After correcting ownership, all four inventory comparisons passed.
- The first full generator test run then exposed an additional stale test requiring `transferDevice` in IdP. Moved that stale-contract refusal test to Management; no completeness guard was weakened.
- First combined script run after regeneration: **185 passed / 0 failed / 4 skipped** out of 189. Two additional actual generated transport tests were added, then the final run supplied `GENERATOR_TEST_BIOME` and passed **191/191** without skips.
- No export, generation, package build or final app-test failure occurred. No build deadlines, authorization requirements or test assertions were relaxed.

## Provenance

Tools: Node `v26.5.0`, pnpm `12.4.1`, Rust `1.99.1 (58cf8f9c3 2026-10-13)`, cached OpenAPI Generator `7.25.0`. Original backend source was the recorded base commit; no tracked Rust implementation was edited.

SHA-256 of actual exported schemas:

```text
930ee289cc6735ea234a107e335e1185606a1ee2b94e96fb4889810f5c34e1bb  idp-owner-openapi.json
3d0ce50173b3098e721d36d3d22bbdc48db9d9aee5a5e4cce2c8ead9dd7c4332  management-owner-openapi.json
```

Remaining feature blockers are outside this generated-client slice: independent-owner setup/relationship coordination and the remaining ticket lifecycle/qualification obligations. The prior pending generated Management SDK dependency for app Device contracts is resolved here; full-ticket acceptance is not claimed.
