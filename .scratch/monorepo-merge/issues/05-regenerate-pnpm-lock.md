# Ticket 05: Regenerate pnpm-lock.yaml for the merged workspace

Status: ready-for-agent

Blocked by: 04-rewrite-pnpm-path-refs

## What it delivers

A single `of/pnpm-lock.yaml` consistent with all rewritten package.json files so that CI's `--frozen-lockfile` installs succeed.

## Scope

- Run `pnpm install` (no `--frozen-lockfile`) from `of/` after Ticket 04 changes land
- Commit the updated `pnpm-lock.yaml`

## Acceptance

- `pnpm install --frozen-lockfile` succeeds in CI and locally
- No `link:` or external references remain in `pnpm-lock.yaml` for `@ofsdk/sql`, `@ofsdk/kv`, `@ofsdk/net`, `@ofsdk/file-system`
