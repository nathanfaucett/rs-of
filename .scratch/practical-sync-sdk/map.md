# Practical Sync SDK — Map

## Notes

- Connects existing engines: ofdb-sql, ofdb-kv, offs, ofnet/iroh-chain
- Services already exist: IdP, Device, Management, Storage
- Missing: app-facing Storage Client API with built-in transport, Sync Server endpoint resolution, WASM npm packages
- Phase 1: SQL over WebSocket only. KV/offs over WebSocket later.

## Decisions So Far

1. **Single app-facing seam**: Storage Client API (`create_sql_db()`, `sync_to_device()`)
2. **Transport**: WebSocket (browser/server), Iroh (native) — hidden behind handle
3. **Sync Server**: Resolves endpoint via Management selection, not hardcoded `local`
4. **WASM packages**: `@ofsdk/sql` (ofdb-sql + sql-sync), `@ofsdk/fs` (offs)
5. **High-level SDK**: `@ofsdk/sync-sdk` wraps raw transport + WASM engines

## Fog

- Exact Management API shape for WebSocket sync selection
- Whether Sync Server calls Management via gRPC or internal trait
- Vector clock format for catch-up (reuse ofdb-sql-sync or new)
- Conflict event shape in `@ofsdk/sync-sdk`
- Publishing process for WASM npm packages from workspace

## Tickets

| # | Title | Type | Status | Depends On |
|---|-------|------|--------|------------|
| 01 | Storage Client: add `create_sql_db`/`open_sql_db`/`sync_to_device` | task | open | — |
| 02 | Sync Server: resolve endpoint via Management selection | task | open | 01 |
| 03 | Management Service: WebSocket sync selection API | task | open | 01 |
| 04 | Publish `@ofsdk/sql` WASM npm package | task | open | — |
| 05 | Publish `@ofsdk/fs` WASM npm package | task | open | — |
| 06 | Create `@ofsdk/sync-sdk` high-level SDK | task | open | 01, 04, 05 |
| 07 | Integration test: browser ↔ native sync via Management | prototype | open | 01-06 |
