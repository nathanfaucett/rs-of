# Ticket 03: guided Join evidence

## Scope and ancestry

- Claimed before implementation in `/home/nathan/code/rust/aicacia/of-setup-03`, branch `setup-simplification-03`, initially clean at integration `23e9006ca` (tickets 01 and 04 merged). Read spec, AGENTS, tracker, shared glossary and 01/04 evidence. Used the authorized existing rendered and shared HTTPS seams without an interview.
- `git merge setup-simplification` reported `Already up to date` after implementation/final tests; integration tip was `23e9006ca`. All file edits, dependency installation, generated output and build targets were confined to this dedicated worktree, not the main worktree.
- Medium presentation complexity, high identity/security risk, high priority. No worker, discovery, pairing protocol, persistent frontend journal or production dependency added.
- Dedicated `JoinSetup.svelte` owns Join presentation and lifecycle-bound observation only. It reuses `WebSetupCreate` and the real generated HTTP adapters. The parent only mounts Join, removes superseded Join form/submission/polling coordination, and locks its choice once a Join intent exists. Create/default choice/advanced connection and parent Device-completion removal remain ticket 02; native startup and maintenance remain 05/06. Independent-owner UI is unchanged.

## TDD and behavior

Used the existing DOM environment injection and real setup client/HTTP adapter; only platform/configuration/HTTP boundaries are injected. Initial rendered test failed because the component did not exist. Subsequent red/green slices caught missing retained Node identity display, sending authorization after IdP/Device selection changes, and recovery finding ready work without triggering the current public-login observation. A real submit click also exposed the Svelte literal-pattern interpolation issue: patterns now use expression strings and the test checks native form validity before clicking.

Nine dedicated rendered cases cover:

1. One pinned peer, default `My Node`, exact selected remote HTTP route, transient code clearing, protected authenticated issuer display before source approval instructions, no native calls.
2. Reload recovery, original retained name/nonce/peer, explicit exact retry, authenticated issuer retained for cancellation, signal acknowledgement not cleanup or readiness.
3. Approval/provisioning acceptance cannot expose sign-in; current local target/public-login observation can, without contacting the source.
4–5. IdP or Device target changes refuse before sending the code or provisioning a bundled owner.
6. Timeout after acceptance retains the original request; protected recovery does not automatically retry, and unmount sends no cancellation.
7. Rejected/stopped/interrupted aggregate status requests attention/recovery, not a new identity.
8. Unpinned input disables submission and sends no authorization.
9. Recovery finding completed work triggers current selected-target/public-login observation even after the intent is cleared, rather than leaving a stalled screen.

The existing mounted Installation test now exercises bundled Join with only the pinned peer and automatic exact protected capability retrieval, while preserving Create coverage. The existing server-rendered working-Join assertion now follows the source Add a device instructions. Source approval UI and public HTTPS lifecycle fixtures were reused, not rewritten.

## Reproduction prerequisites

From this worktree: `pnpm install --offline --frozen-lockfile`; `pnpm packages:build`; in `apps/app`, `pnpm exec paraglide-js compile --project ./project.inlang --outdir ./src/lib/paraglide`.

The existing DOM harness requires external jsdom. Installed `jsdom@26` using npm `--no-save --no-package-lock` in ignored `node_modules/.cache/join-test-tools`, not as a workspace dependency. Set `RESIDENCY_JSDOM_MODULE` to its absolute `node_modules/jsdom` directory. No package/lockfile changes are retained. Generated tracked Storage/browser/bin and Cargo feature-matrix lockfile churn were restored after validation.

## Final passing evidence

Commands below are worktree-relative; frontend commands run in `apps/app`.

| Command | Result |
| --- | --- |
| `RESIDENCY_JSDOM_MODULE=<absolute jsdom directory> pnpm exec vitest run --config vitest.residency.config.ts` | **23 passed**, four files: nine Join, five independent/bundled Installation, six source approval, three residency |
| `pnpm exec vitest run --config vitest.auth.config.ts --testTimeout 20000` | **181 passed**, seven files |
| Focused auth run with the same timeout, `setupClient.test.ts` and Installation `page.test.ts` | **86 passed**, two files |
| `pnpm check` | **0 errors, 0 warnings** |
| Svelte MCP `svelte-autofixer` on JoinSetup and Installation page | No issues; timer/effect suggestions retained intentionally: observation polling requires lifecycle cleanup, not a derived value |
| Biome format on changed interaction tests; `git diff --check` | Passed |

## Failures and qualification limits

- Initial default 5-second focused auth run: 83 passed, three failed. The obsolete working-Join text expectation was updated for the new UI. The other failures were an unchanged setup-client test timing out while transforming imports and its subsequent request-count assertion; with explicit `--testTimeout 20000`, the focused 86 and full 181 tests passed without changing shared client tests or their assertions. No baseline-failure claim is needed.
- Initial `pnpm check` lacked generated Paraglide output; after compiling localization and fixing a new test mock tuple type, the final check is clean.
- Some combined frontend commands timed out at their tool bounds under concurrent compilation load; only the independently observed complete passing runs above are claimed.
- Reused public shared HTTPS Join seam with `RUST_MIN_STACK=67108864 CARGO_TARGET_DIR=<this worktree>/target cargo test -p unified-server --lib production_api_join_ -- --nocapture` (explicit worktree manifest). **Timed out at 200 seconds during dependency compilation (251/560), before tests executed.** The selected existing fixtures include interruption/exact intent, completion/source-offline reopen, and complete-unretired finalization. Runtime/security qualification is not claimed.
- Final `RUST_MIN_STACK=67108864 CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<this worktree>/target cargo hack test --feature-powerset --all-targets` (explicit worktree manifest): **timed out at 120 seconds**, compiling idp-unified, first of ten runs (49/600). No matrix pass.
- Final `RUST_MIN_STACK=67108864 CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=<this worktree>/target just crap` (explicit worktree working directory/justfile): **timed out at 120 seconds**, compiling coverage dependencies (47/982). No CRAP score.
- No Rust implementation changed, so no new Rust clippy result is claimed. No production build, real desktop GUI, OS trust/elevation, secure-provider profile, WAN or other-platform certification. Rendered offline-source coverage verifies no source HTTP dependency in presentation; the real HTTPS offline-reopen scenario was selected but did not execute in this bounded run.
- The worker exposes aggregate unfinished status, not a trustworthy rejection cause; presentation does not invent one. Authenticated issuer availability can lag admission: the UI explicitly requires protected recovery again rather than keeping a capability for polling or trusting unsigned metadata. Source approval remains explicit and source-owned.
