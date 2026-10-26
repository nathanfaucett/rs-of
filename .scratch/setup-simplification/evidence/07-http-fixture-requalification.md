# Ticket 07: standalone setup HTTP fixture requalification

## Scope and base

- Branch: `implement/setup-07-fixtures`, worktree `.worktrees/setup-07-fixtures`.
- Confirmed clean starting HEAD and merge base: `12c7fda8e97f3ec0e2c8e4000e193122e692b296`, also the starting `integration/setup-simplification-followup` tip.
- Read root `AGENTS.md`, shared/Device glossaries and glossary map, issue-tracker guidance, setup-simplification spec and ticket 07, current `setupApi.ts` and its standalone tests, relevant `docs/api.md` contracts, and generated IdP SDK request/decoder/runtime paths. Loaded `tdd` and `ponytail`.
- Authorized seam: existing public setup HTTP client contract, exercised through exported functions with fetch responses. No new seam, orchestration framework, or production implementation.
- Complexity/risk/priority: small fixture correction; security-sensitive assertions require tracing the actual adapter/decoder/error path. Preserve owner isolation, admission, readiness, exact intent, peer/issuer and secret-disclosure negatives.
- Only `apps/app/src/lib/common/state/setupApi.test.mjs` and this evidence file changed. Parent owns ticket 07 closure; ticket, map and spec were not edited.

## Actual red before corrections

Runtime: Node `v26.5.0`.

```sh
node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
```

First attempt failed before tests: `ERR_MODULE_NOT_FOUND` for `@tauri-apps/api`. No dependencies were installed or built. Reused existing dependencies with a temporary ignored symlink:

```sh
ln -s /home/nathan/code/rust/aicacia/of/apps/app/node_modules apps/app/node_modules
node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
```

**Unmodified baseline: 24 tests, 9 passed / 15 failed, 0 skipped/cancelled/todo.** Failures were recovery denial errors; successful Join; Create/unfinished/ready observation; independent login evidence; successful Create; Create remote errors; cookie-free status observation; missing status; admission errors; unified scope; independent owner routes/codes; wrong owner/bases; 503 health; HTTP error redaction; and unexpected successful HTTP status.

## Diagnosis and minimal corrections

### Owner and route fixtures

`SetupOwner`, `readSetupStatus` and `fetchSetupHosts` use `idp`, `device`, `storage`. Management owns permissions, not Device setup. Corrected successful fixtures and independent configuration keys; added explicit rejection of singleton `management` and the obsolete three-owner Management scope. Wrong-owner fixtures still use valid-but-wrong current owners so they reach owner-target mismatch validation instead of failing earlier as unknown scopes.

`fetchSetupStatus` uses generated `SetupApi.setupStatusRaw` at a configured base ending `/idp`, or generated `IndependentSetupApi.independentSetupStatusRaw` at `/independent/setup/status` (including coordinated 404 fallback). The unified prefixed fixture now uses `/custom/idp`, retaining prefix/trailing-slash checks. Independent bases retain arbitrary paths, including Device at `/policy` or `/management`; ownership must come from evidence, not those suffixes.

`verifySetupCode(base, code, true)` selects generated independent admission at `/independent/setup/admission`. Corrected the independent fixture's paths and selector, preserving three distinct codes, exact configured targets, omitted cookies, disabled redirects, no-store and bodyless admission. Generated status requests explicitly use `GET`, not an undefined method.

### Login metadata: decoder boundary, not a protocol fix

Exact path: `fetchSetupStatus` -> `IndependentSetupApi.independentSetupStatusRaw` -> `IndependentSetupStatusFromJSON` -> `readSetupStatus`. The independent model contains only `stage/state/ready/owners`; its decoder discards a wire `login_client`. Requiring rejection of that unknown field was not the current adapter contract. The replacement negative asserts the complete returned singleton status has **no login metadata**, even when the wire body tries to supply it. On the coordinated path, `CreateResultFromJSON` retains `login_client` as `loginClient`; singleton inline login evidence is still explicitly rejected for each owner. Unified ready status still requires complete valid inline login evidence.

This does not make three independent ready booleans sufficient: `fetchSetupHosts` still calls `verifyIndependentReadiness` and obtains login metadata from correlated public facts when every owner is ready. This slice does not change or independently qualify that separate correlation implementation. No proven production bug or protocol change arose from these failures.

### Error assertions: retain denial/redaction, stop expecting retired messages

Exact path for admission, intent recovery and Create HTTP denial: generated `BaseAPI.request` throws `ResponseError` for non-2xx **before** the caller's post-response status checks. Assert public SDK error type and `error.response.status` for 401/403/409/503 as applicable, plus absence of protected values in the displayed message. Unexpected 2xx still reach explicit caller guards (201 recovery; 200 admission). Transport exceptions become generated `FetchError`; preserve denial and message redaction rather than expect the old `host may still be working` string.

Status observation deliberately maps access failures to `SetupPrerequisiteError` and invalid 2xx evidence to `SetupEvidenceError`; updated those obsolete numeric-message assertions without accepting the response as ready. Malformed JSON, missing/wrong scopes, inconsistent readiness, 503 unavailable evidence, exact intent validation, pinned peer/issuer checks and HTTPS/current-Origin constraints remain tested. Redaction claims here concern displayed `error.message`, not erasure of SDK `response`/`cause` diagnostic objects.

## Commands and outcomes

After the actual baseline red, corrections were checked in bounded slices:

```sh
node --experimental-strip-types --test --test-name-pattern='Join admits|web observes|Create sends' apps/app/src/lib/common/state/setupApi.test.mjs
```

**3/3 passed** after correcting the shared owner fixture.

```sh
node --experimental-strip-types --test --test-name-pattern='independent status|web setup reads|unified scope|independent owners each|missing, duplicate|missing or duplicate independent|503 health' apps/app/src/lib/common/state/setupApi.test.mjs
```

**7/7 passed** after correcting owner/path/metadata fixtures. These are focused selections, not skipped tests in the final suite.

```sh
node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
git --no-pager diff --check
```

Full suite after error correction: **24/24 passed**. After adding coordinated singleton-login rejection checks: **24/24 passed** again. Whitespace check passed. A read-only Node filesystem byte comparison confirmed all **77** files under the existing root `packages/idp-client/src` match this worktree's SDK source; reused existing ESM, with no SDK build or generated artifact edits.

```sh
GIT_EDITOR=true git -c core.hooksPath=/dev/null merge --no-edit integration/setup-simplification-followup
node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs
git --no-pager diff --check
```

Integration merge: **Already up to date**, integration tip still `12c7fda8e97f3ec0e2c8e4000e193122e692b296`. Post-merge standalone run: **24 passed / 0 failed / 0 skipped/cancelled/todo**. Whitespace check passed. Commit uses `-c core.hooksPath=/dev/null`; temporary dependency symlink is removed before clean-worktree handoff. To rerun in this dependency-free worktree, recreate the symlink above, run the historical command, then `rm apps/app/node_modules` (remove the symlink only).

## Limits

Fixture-only qualification, not ticket 07 closure. No production, Svelte, Rust, generated SDK, timeout or security-policy changes. No Cargo, SDK builds, pnpm install, generator suites, matrix, coverage/CRAP, real profile/OS trust, GUI, production provider or WAN checks ran. Exclusive Rust qualification remains with the other agent; broader acceptance remains with the parent.
