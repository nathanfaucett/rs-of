# Ticket 09: Extend .gitignore for moved crate build output

Status: ready-for-agent

Blocked by: 01-copy-remaining-ofdb-crates

## What it delivers

Git status stays clean; copied build output from `db/*`, `fs/*`, `net/*`, and `packages/*` is ignored.

## Scope

Add to `of/.gitignore`:

```
/target
**/target/
**/pkg/
**/test-results/
**/artifacts/
**/browser/
**/esm/
**/types/
**/*.tsbuildinfo
**/node_modules/
**/.svelte-kit/
**/dist/
```

(Preserve existing entries; the `**/target/` and `**/pkg/` patterns cover nested crate output; `packages/sql-wasm/pkg` and `packages/fs-wasm/pkg` are intentionally checked in — if they should stay checked in, add `!packages/sql-wasm/pkg` and `!packages/fs-wasm/pkg` exceptions)

## Acceptance

- `git status --porcelain` shows only the intentional untracked directories (`db/`, `fs/`, `net/`, `packages/`, `services/`) and no build output
- `git check-ignore db/kv/target` succeeds
- `git check-ignore packages/sql/pkg` fails (allowed)
