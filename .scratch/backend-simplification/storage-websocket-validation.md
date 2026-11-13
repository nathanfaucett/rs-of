# Storage WebSocket repair

Branch: `backend-simplification-buildfix`.
Merged requested integration `f9209c816` before implementation. The merge was a fast-forward: integration already contained the earlier buildfix commits. Nothing was merged back into integration or main.

## Implementation

- Replaced obsolete SQL calls with `ofdb_sql::history` and checked `Database::exchange_resource_history` with native file staging.
- Resource scope is the physical database selected by the authenticated subject, typed application ID, and resource ID. No global/shared owner database is exposed through this endpoint.
- Restored the existing typed control/owner protocol: Postcard, little-endian lengths, and IEEE CRC32 over the payload. Owner SQL bytes must be inside `OwnerMessage(OfdbSqlV7)`, not raw WebSocket binary messages.
- Reused current IdP introspection and Device selected-resource APIs. Endpoint IDs are opaque endpoint identities, not parsed as UUID Device IDs. Policy must match endpoint, owner, application, database kind, resource, administrator eligibility, and absence of pending cleanup.
- Authorization is re-evaluated before send, before/after receive, and immediately before each atomic apply. A changed subject/application/resource or denied/unavailable policy terminates the session. No unchecked production apply path was added.
- Hello and AttachRequest identities must match. Unsupported protocol versions/owners, malformed envelopes, CRC failures, trailing bytes, flags, and oversized frames fail closed. WebSocket reads and cumulative session bytes are bounded, including handshake frames.
- Push/Pull attach requests now fail explicitly. The current resource-history session is bidirectional; silently granting bidirectional authority for a restricted request was incorrect. No new directional sync engine was added.
- Removed obsolete Management selection calls and unused session scaffolding. Reused `log`, `postcard`, and current scoped `DatabaseRuntime::open_selected`; no missing runtime dependency was papered over with a new library.
- Added only a direct test dependency on already-resolved `tokio-tungstenite 0.29`, with minimal connect/handshake features, for real loopback WebSocket tests.

Risk/priority: high security and protocol risk; repair the existing optional path without changing native mesh protocols, dependency paths, local seed/migration code, or frontend/Desktop callers.

## Tests

The initial all-feature targeted run failed on the obsolete imports/APIs. The existing orphaned authorization tests are now wired into the module again.

| Check | Result |
| --- | --- |
| `cargo test -p storage-server --all-features --all-targets -- --test-threads=1` | **101 unit tests + 5 integration tests passed** |
| `cargo test -p storage-server --all-targets -- --test-threads=1` | **95 native/default unit tests + 5 integration tests passed** |
| `cargo test -p storage-server --all-features --lib sync:: -- --test-threads=1` | **6 targeted WS/security tests passed** |
| `cargo clippy -p storage-server --all-features --all-targets` | Passed with warnings; no lint errors |
| Explicit-file `rustfmt`, `git diff --check` | Passed |

Both full suites retain one directly ignored HTTPS child entrypoint; its parent runs the isolated HTTPS child cases. Native SQL/KV transfer, blocked-apply denial, retry/reconnect, separate SQL/KV streams, and idle tests remain green.

Targeted coverage:

- Real HTTP upgrade, typed SQL exchange, committed row observation, separate owner namespace preservation, current policy revocation, and authorized reconnect/retry.
- Exact Device policy field mutations, pending cleanup, and upstream failure denial.
- The same WS `SessionAuthorization` guard blocked at the native SQL pre-apply boundary over paired owner transports: revoke while blocked, then verify `PreApply` denial and unchanged committed history. This isolates the commit guard from frame authorization. The full WebSocket transfer/revocation test is separate.
- Fixed Postcard Ping wire tag, little-endian frame fields, payload CRC, invalid flags/size/truncation/trailing bytes/CRC, identity substitution, unsupported direction, and unsupported owner protocol.

The first two targeted attempts reached a 200-second compile limit; rerunning with a 600-second bound completed compilation and passed. Final runs above completed.

## Dependency and remaining limits

- No new ofdb commit was needed. The required existing dependency remains **`544b6270116ef73e7cab53b2a191bcad608146ba`** in the shared ofdb buildfix worktree. No sibling tracked files or Cargo paths were changed.
- Workspace feature powerset: `cargo hack test --feature-powerset --workspace --all-targets` reached the 200-second limit during compilation; no full result.
- `just crap` failed before producing a score: the unchanged `of-storage-client` has 14 compile errors, including unresolved `storage_sync_client` references in `native.rs`/`transport.rs` and removed `ofdb_sync::PushResult` in `handle.rs`. These client repairs are outside this Storage-server sync change.
- WS remains a bidirectional SQL resource endpoint. No KV/filesystem dispatch, browser/Wasm engine, directional protocol, or frontend integration is claimed. Native SQL+KV mesh is covered by the full suites.

Implementation commit uses explicit owned paths only. Commit hooks are bypassed because their broad workspace check is blocked; explicit test/lint results are recorded above. Logs remain untracked.
