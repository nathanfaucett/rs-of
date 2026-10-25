# Integration of tickets 02, 03, 05 and 06

Merged into `setup-simplification` in the main `of` worktree, in order 02 → 03 → 05 → 06, with merge commits. Ticket 07 remains unchanged and is not qualified as done.

## Conflict resolution

- Keep credentials-only Create, default `My Node`, exact retained retry and direct verified ordinary sign-in from 02; mount dedicated guided Join from 03 with its own protected admission and default name. Remove the now-unused parent Node-name state.
- Combine mounted Create recovery and startup/trust consent regressions; preserve both default-name expectations and sign-in without the deleted Device dismissal gate.
- Keep 05's asynchronous lifecycle/trust consent while deleting the obsolete completion command, readiness helper and synchronous startup cleanup helper. Do not restore completion persistence or registration.
- Preserve 05's explained trust controls and 06's maintenance navigation; no inline destructive onboarding controls. Include Join and maintenance in the mounted test configuration.
- Combine all ticket map entries and current workflow/native/operations documentation, including verification-first trust, separate maintenance and deferred authenticated Device preferences.

## Validation

Commands run in `apps/app` unless noted.

- `pnpm check`: passed, zero errors and warnings, including rerun after final source cleanup.
- `RESIDENCY_JSDOM_MODULE=/home/nathan/code/rust/aicacia/of-setup-04/node_modules/.cache/source-approval-test-tools/node_modules/jsdom pnpm exec vitest run --config vitest.residency.config.ts --maxWorkers 1 --testTimeout 20000`: passed twice, **32 tests / five files**, including final source cleanup. Uses existing disposable jsdom tooling, no dependency changes.
- `pnpm exec vitest run --config vitest.auth.config.ts --maxWorkers 1 --no-file-parallelism --testTimeout 20000`: **180 tests / seven files passed**, including all 28 Installation page cases.
- `pnpm dlx @sveltejs/mcp svelte-autofixer ./src/routes/setup/installation/+page.svelte`: no issues; existing lifecycle-managed polling effect suggestions retained.
- Root native focused test attempted with `timeout 90s cargo test -p lidp --lib -- --skip native_shared_create_reopen_and_provider_safe_reset`: bounded dependency compilation; no native test pass claimed.
- Root final matrix `timeout 45s cargo hack test --feature-powerset --all-targets` and complexity `timeout 45s just crap`: bounded compilation, no matrix pass or CRAP score claimed.
- Rust formatting and `git diff --check` checked; final formatting removes the extra blank line left by conflict resolution.

## Preservation and limits

The sole initial uncommitted file was `Cargo.lock`. A file-specific stash preserved its exact original bytes (SHA-256 `bb7e608f57a6167612acac7d247e51eb2c6efbe449b95208c7a1862b78dd3a3f`). Cargo refreshed the working copy during checks; restore the saved bytes after all Cargo commands and verify this checksum. No lockfile changes belong in integration commits.

These frontend results do not qualify real native GUI, trust stores, providers, cross-platform behavior or ticket 07. Ticket evidence retains its own additional validation limits.
