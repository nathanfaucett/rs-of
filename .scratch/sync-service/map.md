# Sync Service Map

## Notes

- Uses existing device mesh (iroh P2P) — no new transport layer
- WebSocket sync is an additional entry point, not a replacement
- Auth reuses IdP token introspection + Device Server selection API
- Sync engine is existing `ofdb-sql-sync` (same as P2P)

## Decisions So Far

1. **No new service** — add `sync` feature to `storage-server`
2. **Protocol** — reuse `sync-test-harness` types (copy, not depend)
3. **Scope** — SQL sync only; KV/filesystem later
4. **Deployment** — enable on at least one device; no dedicated relay required
5. **Passthrough** — works automatically via existing `offs` residency model

## Fog

- [ ] Exact `sync-test-harness` types to copy vs. shared crate
- [ ] WebSocket frame format compatibility with `ofdb-sql-sync` events
- [ ] Error handling: IdP unavailable, Device Server unavailable
- [ ] Rate limiting / DoS protection on WebSocket endpoint
- [ ] TLS termination (reuse existing `LocalTls` config?)
- [ ] Browser WASM client SDK (separate crate or embedded in app?)
