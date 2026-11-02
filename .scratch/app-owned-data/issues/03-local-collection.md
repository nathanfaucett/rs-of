# 03 — Replace the collection with a local-source implementation

Status: resolved
Work status: resolved
Type: task
Blocked by: 02

## Goal

Use the proven local vault source with TanStack DB. Keep persistence, schema,
encryption, and sync setup out of the generic collection.

## Scope

Rewrite `packages/storage-db-collection` from the source interface proved in 02.
Keep Standard Schema/key validation and the official TanStack binding. Remove
required User/Application labels; collection identity must distinguish actual
local vaults and unlock lifetimes.

Delete superseded byte-backend/RPC adapters, unused acknowledgement types, and
Storage-client dependencies/exports. Replace obsolete tests and README examples.
Do not make SQL implement filename CRUD. Do not add universal SQL/KV/filesystem
adapters. Move reusable settlement logic here and keep Secret behavior in the app.

If the proven interface has no useful generic behavior beyond the app module,
delete the unused package instead and record that result. Do not keep a wrapper
only to preserve the package name.

## Acceptance

- App caller consumes one tested implementation; no duplicate collection engines.
- Initial load, ready-empty, malformed snapshot, optimistic CRUD, rejected and
  uncertain commit, refresh, queued edits, and terminal disposal have tests.
- Local persistence settlement is independent of sync availability.
- Preserve atomic local mutation/snapshot and avoid TanStack commit-receipt
  deadlocks. A view publication receipt is not durable owner replication.
- Lock, vault switch, and disposal cannot publish late results into a new view.
- Owner-backed reopen tests accompany real TanStack tests; Map fixtures alone
  cannot prove durability.
- Call sites, exports, dependencies, fixtures, and docs match the new interface.

## Comments

Source inspection found no production callers of the old generic collection.
Confirm that again before deletion. No compatibility shim is required by this plan.
