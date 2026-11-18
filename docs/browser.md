# Browser storage and packages

The browser packages wrap the Rust storage engines with WebAssembly. IndexedDB provides SQL/KV persistence; the filesystem package uses browser persistence for metadata and content. No network is required for ordinary local operations.

Current browser support and synchronization qualification are separate. A package or transport export does not prove that an application has a complete authorized sync path. See [current status](../README.md#current-status) and [E2E evidence](../e2e/runtime-validation.md).

## Packages

| Package | Local source | Package state |
| --- | --- | --- |
| `@ofsdk/sql` | `packages/sql` | Packable; registry publication is not assumed |
| `@ofsdk/kv` | `packages/kv` | Private workspace package |
| `@ofsdk/file-system` | `packages/file-system` | Private workspace package |
| `@ofsdk/net` | `packages/net` | Private workspace package |
| `@ofsdk/sync-sdk` | `packages/sync-sdk` | Private workspace package under active integration |

Build local artifacts from this checkout:

```bash
pnpm install --frozen-lockfile --ignore-scripts
pnpm --filter @ofsdk/sql run build:all
pnpm --filter @ofsdk/kv run build:all
pnpm --filter @ofsdk/file-system run build
pnpm --filter @ofsdk/net run build
```

Build only the packages needed by the application. `packages/README.md` is the package index.

## Rust browser facades

The canonical Rust facades are `db/sql` and `db/kv`. Their browser backends use IndexedDB and require explicit feature selection. See their crate READMEs for current features and public APIs.

Do not use native Redb, native file staging, Tokio, TCP/Unix servers, or Iroh native transports in a browser build. Browser transport and authorization remain application responsibilities.

## Storage limits

Browser storage is scoped to origin, browser profile, and browser partitioning policy. Private browsing, quota pressure, eviction, and clearing site data can remove local data. IndexedDB completion is not native filesystem `fsync` equivalence. Keep a backup or replica for data that must survive browser storage loss.

The initial IndexedDB transaction adapter materializes a whole physical database snapshot. Snapshot, history, WASM, and JavaScript allocations must be included in application memory budgets. There is no established safe maximum database size.

A blocked or incompatible open must fail explicitly. Do not silently delete or recreate a database. Format migration and recovery require an explicit product decision.

## Synchronization limits

Local subscriptions observe local commits. They do not observe remote-only writes, other independently opened runtimes, or every other tab/worker. Applications must provide authorized transport, remote wakeups or explicit reconciliation, and broad recovery after missed state.

A continuous runner is not a durable replay log or automatic bidirectional real-time guarantee. Browser sync is complete only when a real application performs the full OIDC, Device, Selection, transport, persistence, and reconnect workflow through public contracts.
