# Ticket 02: streamlined Create and deferred Device preferences

## Scope and ownership

- Worktree `of-setup-02`, branch `setup-simplification-02`, initially clean at integration `23e9006ca`.
- Read AGENTS, tracker, spec, map and ticket01 evidence; claimed ticket02 before source edits. Used the spec-authorized rendered-page/mounted-form and shared/native lifecycle seams, without another interview.
- Complexity/risk/priority: medium presentation complexity, high identity/authorization risk, high priority. No dependency, new worker, persistence, migration, bootstrap session or password storage.
- Default bundled Create/Join remains separate from advanced connection settings. Browser/remote and independent owners keep their existing selected-target and separate admission contracts. Join presentation is unchanged apart from removing the shared Device-dismissal gate. Startup and maintenance modules receive only necessary completion-gate deletion; their owned behaviors are unchanged.

## Caller and safe-default audit

The removed `/setup/device` page invoked only `complete_device_setup` then navigated to sign-in. The native command verified current readiness/public login and wrote `device-setup-ready`; it performed no owner operation or local resource selection. Native observation read that marker; setup entry, sign-in, authenticated layout and Installation setup used its projected `device_setup_required` action. Those callers, command registration, command, marker module, obsolete readiness helper and reset marker cleanup are removed, not wrapped. No historical marker is read or migrated.

Actual Device Setup remains in authenticated Devices (`DevicePolicy.svelte`, `ResourceResidency.svelte`), not in that dismissal page. `crates/device-server/src/router/routes/device_selection.rs` still requires `devices.select`, Device ownership, independently scoped Storage access and existing administrative permission before mutation. `DbSelectionPolicyRepo::resource_selected` denies absent selections; its replication admission requires both approved owner-bound Devices, policy, administrative allowance and exact resource selection. Fresh Storage/selection are empty. Skipping dismissal makes no owner mutation and preserves existing selections/restrictions/authorization. Ordinary Devices already supports `renameDevice` through the owner API; default naming is not permanent.

Fresh unified and independent Create derive `My Node` without a dependency. Exact recovered intents always supply their retained name, never that default. Bundled Create retrieves its target-bound protected code internally per submit/recover/cancel; remote/browser codes remain explicit. Credentials are cleared on submission, observation and component destruction. Recovery uses the existing shared worker and newly entered password, never a replacement nonce or persisted credential.

| Situation | Behavior |
| --- | --- |
| Fresh bundled Create | Administrator name/password only; scoped protected code retrieval; `My Node` default |
| Request accepted or response lost | Progress/observation, not sign-in; accepted work belongs to shared worker |
| Page closed/reopened | Recover exact durable intent with existing admission; re-enter original password for retry |
| Current verified owner/public-login readiness | Ordinary sign-in; no session established and no dismissal marker |
| Missing/malformed retained state | Existing-only lifecycle refusal/repair, not fresh setup or new identity |
| Deferred data preferences | Ordinary authenticated Devices selection/restriction/Residency and rename controls |

## Red → green evidence

1. Changed rendered ready/deferred-preference expectations first: **2 failed / 26 passed** against the former Device gate. Removing marker-based routing restored **28 passing** page cases; malformed owner/login evidence still blocks.
2. Changed the mounted bundled Create scenario to require only credentials, automatic admission, advanced connection navigation, a default name and ordinary sign-in: **1 failed / 4 passed** before implementation, then all passed. Join still uses its existing presentation and native/HTTP seams.
3. Changed independent Create to omit Node input while retaining owner-scoped admission: **1 failed / 5 passed**, then green after defaulting Create only.
4. Added a mounted regression for a response lost after accepted Create, closure/reload, exact recovery and password re-entry. It compares both actual mutation bodies (including nonce) and checks no early sign-in, no persisted password, no mutation to another owner. All **6 Create/Join/retry interaction cases pass**.
5. Native lifecycle tests now verify a foreign historical dismissal marker does not block actual current readiness, while a marker alone cannot establish readiness. Existing provider Create/completed reopen/reset and lost-host/TLS-evidence refusal tests remain exercised.

## Validation

Commands are worktree-relative; frontend commands run in `apps/app` unless stated otherwise.

| Command | Result |
| --- | --- |
| Root `pnpm install --offline --frozen-lockfile`; `pnpm packages:build` | Passed; worktree-local build prerequisites |
| `pnpm exec vitest run --config vitest.auth.config.ts --maxWorkers 1 --testTimeout 20000` | **180 passed**, seven files; final rerun passed |
| `pnpm exec vitest run --config vitest.residency.config.ts --maxWorkers 1` | **15 passed**, three files, including six mounted Create/Join/retry cases and three Residency/authorization regressions; final rerun passed |
| `pnpm check` | **0 errors, 0 warnings**, final rerun |
| `pnpm build` | Passed, final static application build |
| Svelte MCP autofixer, Installation page and IndependentSetup | No issues; IndependentSetup has no suggestions; Installation retains timer/effect suggestions because polling requires asynchronous effect cleanup rather than a pure derived value |
| Root `cargo test -p lidp --lib` | **15 passed**, one deliberately isolated child ignore; parent launched child and **1 passed**; final rerun passed |
| Root `cargo test -p api` | **40 passed**, two explicit child ignores, parent-launched children passed; compile-fail doc test passed |
| Root `cargo clippy -p api -p lidp --all-targets --no-deps -- -D warnings` | Passed |
| `rustfmt --edition 2024 --check --config skip_children=true` on changed Rust modules; `git diff --check` | Passed |
| Root `cargo test -p unified-server --test setup_host` | **Timed out at 200 seconds** compiling its distinct feature graph; no pass claimed |
| Root `cargo hack test --feature-powerset --all-targets` | **Timed out at 200 seconds** compiling dependencies (414/600); matrix unqualified |
| Root `just crap` | **Timed out at 200 seconds** compiling coverage dependencies (418/982); no score claimed |

DOM tests used the existing `RESIDENCY_JSDOM_MODULE` injection with `jsdom@26` installed into a disposable feature-local test-tools directory. That directory was removed. No dependency/lockfile change is committed. Generated Storage/browser/bin output and unrelated Cargo lock refresh caused by sibling path dependencies were restored after checks.

Initial broad frontend execution had 18 failures under compilation contention. Serial execution reduced this to two failures following an existing 5-second import timeout; the complete suite passes with one worker and a 20-second test bound, without source/fixture changes to those unrelated tests. Initial Svelte check needed generated localization output; production build generated it. The new interaction fixture had one discriminated-union typing error, corrected before final clean check. Several native builds timed out while warming dependencies; subsequent complete native/API tests and strict Clippy passed. One MCP invocation reported a post-analysis network timeout; later complete analyses reported no issues.

## Integration and limits

`git merge --no-edit setup-simplification` reported **Already up to date** at integration `23e9006ca`; no main-worktree source was modified. Ticket02 does not claim ticket03 Join presentation, ticket05 visible startup/trust consent or ticket06 maintenance delivery.

No real OS trust/elevation, secure-provider profile, full desktop GUI, WAN, other-platform or production qualification. Native tests use isolated roots/providers and injected trust. Feature matrix, CRAP and the separately requested unified-host test remain explicitly unqualified by the bounded runs, not silently marked green. The passing native provider scenario exercises actual shared HTTPS Create and completed existing-only reopen; no competing lifecycle implementation or suite was introduced.
