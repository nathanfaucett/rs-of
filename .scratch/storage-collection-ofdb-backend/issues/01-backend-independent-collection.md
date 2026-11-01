# 01: Make the collection backend-independent

**What to build:** Developers can connect a TanStack DB collection to a small async storage backend. Existing WebSocket-backed collections retain their behavior through an adapter in the same package. Tests, callers, and examples change with the API.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Replace the collection's client.openSocket option with a connect factory that accepts an optional AbortSignal and returns a backend.
- [ ] Define a minimal async backend contract for read, write, delete, and prefix listing, with optional cleanup and disconnect notification. Missing reads return undefined; listings return full keys. Embedded backends need no socket or lifecycle events.
- [ ] Move Storage request/response mapping and filesystem-directory traversal into the WebSocket adapter. Preserve empty-directory behavior without treating failed listings as empty.
- [ ] Define successful write/delete completion as acknowledgement. Preserve the distinction between explicit rejection and an uncertain outcome, including malformed acknowledgements and timeouts; do not infer success from a later snapshot alone.
- [ ] Keep schema validation, record identity checks, safe paths, duplicate detection, serialization hooks, single-record mutations, optimistic settlement, authoritative reconciliation, request deadlines, explicit Refresh, and cleanup behavior.
- [ ] Define handling for a listed record that disappears before read; do not deserialize undefined or publish a partial snapshot as complete.
- [ ] Keep collection scope identity separate from transport selection. Embedded use must not require a fictitious Filesystem ID; WebSocket resource selection remains in its adapter configuration. Preserve isolation between collection scopes.
- [ ] Preserve reconnect-on-Refresh after failure, abort connection setup on cleanup, detach lifecycle listeners, and prevent late results from publishing after failure or disposal. Release late-opened owned backends.
- [ ] Update all affected callers, package exports, examples, and documentation together. Do not retain the old option through a compatibility layer.
- [ ] Verify public collection behavior with existing regression tests and focused backend tests. Keep socket-specific protocol, disconnect, and timeout assertions with the WebSocket adapter.
- [ ] Run the affected package's tests, type checks, build, and lint checks. Record any validation limits.

## Notes

The parent spec is planning context. This approved breakdown replaces its conflicting promises of unchanged caller syntax and unchanged WebSocket test setup. No backend registry, new package, or separate test framework is required.
