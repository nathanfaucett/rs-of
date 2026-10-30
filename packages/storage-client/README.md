# Storage Client

The package contains the existing WebSocket client and a generated REST owner client under `src/generated`. `ResourcesResidencyApi`, `StorageOwnerConfiguration`, and `ResidencyMode` expose authorized whole-filesystem Full/Passthrough controls. They do not grant access or establish content/cleanup readiness.

Regenerate from the actual composed Storage OpenAPI listener:

```sh
OPENAPI_SPEC_URL=https://localhost:PORT/storage/openapi.json pnpm run openapi
```

Use the verified listener's literal URL and its normally approved CA trust. Do not bypass certificate validation. A saved, unmodified document fetched from that listener can also be supplied as the input path for reproducible offline generation.

The existing shared generator checks both Residency operations before atomically replacing only `src/generated`; failure preserves prior generated and handwritten sources. `openapitools.json` pins the generator. Never edit generated TypeScript by hand. Build with `pnpm build`; run focused client tests with `pnpm exec vitest run src/residency.test.ts src/index.test.ts`.

## Authenticated application SQL

The handwritten `StorageClient` exposes explicit `ensureDatabase(request, signal?)`,
`createDatabase(name?, signal?)`, `listDatabases(signal?)`, `getDatabase(id, signal?)`,
`deleteDatabase(id, signal?)` and `executeSql(id, request, actions, signal?)`.
They use generated owner requests and exchange the current User token for
`storage:databases` with exact requested actions. Filesystem calls retain their
`storage:filesystems` exchange. SQL uses Authorization headers, never socket/query
tokens. Service-base prefixes are retained and AbortSignals reach both requests.
`DatabaseOperationError.status` reports HTTP failure without owner response bodies.
There are no resource defaults, implicit creation, schemas, mutation retries or
Device selection in this package. The SDK adds captured-session lifecycle guards.

Public wire types (`EnsureDatabaseRequest`, `DatabaseResource`, `SqlRequest`,
`SqlStatement`, `SqlValue`, `SqlResult` via generated exports, and `SqlResponse`)
are generated from `openapi/storage.json`, alongside the Rust Storage owner client.
Tagged integers are decimal strings, preserving i64 values beyond JS safe integers;
serialized records can use text. A batch succeeds completely or fails without a
partial snapshot. Timeouts/cancellation after sending do not prove rollback.
See the [exact server contract](../../crates/storage-service/docs/api.md#explicit-application-sql-contract)
and [SDK SQL usage](../sdk/README.md#explicit-sql-capability).
