# Ticket 01: onboarding observation evidence

## Scope and ancestry

- Worktree: `/home/nathan/code/rust/aicacia/of-setup-01`; branch `setup-simplification-01`.
- Started clean at `dfeaf7dbe`, exactly the integration branch `setup-simplification`; `git merge-base --is-ancestor setup-simplification HEAD` succeeded.
- Claimed the ticket before implementation. Used the spec's existing rendered-page, mounted-form, generated HTTP and narrow native/provider seams; no source approval UI changes.
- `git merge setup-simplification` reported `Already up to date` before the implementation commit. No main-worktree source changes.
- Complexity/risk/priority: medium observation/workflow complexity, high identity/security risk, high priority. No new lifecycle persistence, compatibility wrappers, provisioning implementation or frontend dependency.

## Red → green evidence

Observed failing checks before their corresponding implementation:

1. Rendered loader test required one target-bound, secret-free bundled snapshot; the former loader returned no observation.
2. Shared admission test required existing-only protected capability retrieval and retirement; `SetupAdmission::protected_code` did not exist.
3. Actual native adapter test required typed `NativeHost::observation`; the method did not exist.
4. Remote HTTP 500 incorrectly projected identity repair; it now offers transient retry.
5. Mounted bundled failure retry invoked the former recovery navigation but did not refresh the page; the primary runtime retry now observes again without reset.
6. Invalid selected URLs leaked embedded credentials/query input into the target projection; public targets now omit unsafe addresses before returning status.
7. Malformed evidence from a changed target incorrectly requested repair of the new target; old evidence/failures are now discarded and the new target gets retry.
8. Missing native Device-preference evidence incorrectly implied completion; a ready snapshot now requires explicit boolean evidence rather than assuming completion.
9. Fresh bundled launch had no stored IdP address and refused exact protected capability retrieval. An unset address now retains the existing default bundled selection; the native origin and any explicit address must still match exactly, and selection is rechecked before and after retrieval. Observation itself does not initialize or persist owner addresses.

Preserved exact Create/Join/recovery/cancellation tests, separate owner admission/correlated readiness, transient password/code handling, listener/Origin/trust tests, and isolated provider Create/reopen/reset coverage. The protected read validates current private files but cannot know a prior worker's lock inode; the live admission handle retains that historical lock fence. A new read-only management handle is not worker health or mutation proof.

## Final focused validation

| Command (worktree-relative) | Outcome |
| --- | --- |
| `pnpm install --offline --frozen-lockfile` | Passed; worktree-local dependencies |
| `pnpm packages:build` | Passed; generated tracked Storage/browser/bin artifacts restored afterwards, not committed |
| `cd apps/app; pnpm exec vitest run --config vitest.auth.config.ts` | **181 passed**, seven files |
| `cd apps/app; pnpm exec vitest run --config vitest.residency.config.ts` | **8 passed**, including mounted independent/bundled Create/Join and non-destructive retry |
| `cd apps/app; pnpm check` | **0 errors, 0 warnings**, after SDK/localization build prerequisites |
| `cd apps/app; pnpm build` | Passed; static application built |
| Svelte MCP autofixer on Installation page and NativeSetupRecovery | No issues; Installation suggestions concern the existing lifecycle-owned polling timer, which needs an effect/cleanup rather than a pure derived value |
| `cargo test -p api` | **40 passed**, two explicitly isolated child-test ignores; launched delayed-readiness child passed; one compile-fail doc test passed |
| `cargo test -p lidp --lib` | **17 passed**, one isolated child ignore; provider parent explicitly launched that child and it passed |
| `cargo test -p unified-server --test setup_host` | **2 passed** |
| `cargo clippy -p api -p lidp --all-targets --no-deps -- -D warnings` | Passed |
| `rustfmt --edition 2024 --check` on changed Rust modules; `git diff --check` | Passed |

DOM tests use the existing `RESIDENCY_JSDOM_MODULE` test-environment injection. `jsdom` was installed only in an ephemeral `.scratch/setup-simplification/test-tools` directory, with no package/lockfile dependency changes; that directory was deleted after validation. To reproduce, provide an installed `jsdom` module through that environment variable, or install it in a disposable prefix. SDK and localization output are build prerequisites, not source changes in this ticket.

## Failures and qualification limits

- Initial frontend baseline: **10 failures / 191 passes**, all failures in the existing auth-layout suite. Migrating its obsolete native fixtures and stale `management` owner scope to the current shared `device` scope resolved those failures in the final green suite.
- `node --experimental-strip-types --test apps/app/src/lib/common/state/setupApi.test.mjs`: **9 passed / 15 failed**. Re-ran the original source and original tests from `dfeaf7dbe` in disposable sibling files: **the same 15 failing named cases**. Existing standalone fixtures/legacy error expectations remain outside this ticket. Its removed stage-function import was migrated to typed status observation; these failures are not hidden or claimed green.
- `cargo clippy -p api -p lidp --all-targets -- -D warnings` is blocked by **93 existing generated IdP SDK lint errors**, including `empty_docs` and `derivable_impls`. The changed-crate `--no-deps` strict check passes; generated SDKs were not rewritten.
- Initial native dependency builds exceeded 200-second limits; subsequent bounded builds completed and actual native tests passed. Initial simultaneous broad Rust attempts also timed out during compilation/build-lock contention; final checks were serialized.
- Final `cargo hack test --feature-powerset --all-targets` **failed** on `idp-unified` (1/10): `utoipa-swagger-ui 9.0.2` could not open its downloaded/cached Swagger UI `v5.17.14.zip`, `InvalidArchive("Could not find EOCD")`. The complete feature matrix is **not qualified**.
- Final `just crap` **timed out after 200 seconds** compiling coverage dependencies (571/981); no CRAP score or complete coverage report is claimed.
- No real OS CA installation, elevation, real secure-provider profile, full desktop GUI acceptance, WAN or other-platform qualification. Native tests use isolated roots/providers and injected trust. Shared Create/Join provisioning, Device preference semantics and source approval are preserved, not reimplemented; removal of the Device-dismissal gate remains a later slice.
