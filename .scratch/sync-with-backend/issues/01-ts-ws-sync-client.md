# 01: TS WebSocket sync client package

**What to build:** A browser-side TypeScript sync client package that lets
the app open an authenticated WebSocket sync session against a storage
service's sync endpoint. It implements the wire framing and the
Hello/Attach handshake, supports Push, Pull, and Bidirectional
directions, and takes an IdP access token through a token provider fed
from the existing signed-in OIDC state. From the User's perspective: the
app can connect to a configured sync service and complete (or be
rejected by) the attach handshake.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Client connects to the sync endpoint derived from a storage API URL and completes the Hello/Attach handshake with a valid IdP token
- [x] Attach rejection (unauthorized/invalid token) surfaces as a clear auth error, not a hang or generic failure
- [x] Push, Pull, and Bidirectional directions are selectable and reported by the session result
- [x] Protocol behavior (framing, handshake, direction, accept/reject) is tested against a mock/in-memory WebSocket at the protocol seam, asserting external behavior only
- [x] Package follows the existing package layout and build/test conventions of the other client packages

## Comments

- Claimed for completion on `implement/sync-ws-client` in the dedicated worktree.
  Base confirmed: integration `implement/sync-with-backend` at
  `a03e37f331509641f2670de9b6cd5e30a75403dc`. Existing uncommitted client work is retained.
  Protocol seam is defined by the spec: public client/session behavior over an
  in-memory WebSocket, checked against actual server-generated wire fixtures.

## Answer

Implemented `@ofsdk/sync-client` with no runtime dependencies. It derives the
WS endpoint from the Storage API URL, obtains a fresh token for each attach,
encodes real Postcard messages and IEEE CRC32 frames, handles the echoed Hello,
and reports attach acceptance, auth rejection, and requested/granted directions.
Cancellation and transport failures close the session and reject pending reads.
The open session exposes `send`, `receive`, and `close` for actual SQL owner
messages, with frame/session budgets. It reports `attached`, not backup success.

The actual Rust server definitions and FrameCodec generate the checked-in wire
fixtures. The older Rust client/harness and collection sync placeholder do not
match that contract. They were not used as a wire reference or modified.
The root SDK build includes this package; workspace dependency resolution is
recorded in the lockfile. All existing uncommitted task implementation was
retained and checked. A new red/green URL test fixes unclassified TypeError
failures; a large-token regression also verifies exact binary token bytes.

Validation:

- Public client/session protocol tests: 57 passed.
- Package TypeScript build, Biome lint, and format: passed.
- Rust server fixture drift check: passed (actual definitions compiled offline).
- Root `pnpm run sdk:build`: passed.
- `cargo hack test --feature-powerset --all-targets`: timed out after 45 seconds
  during Rust dependency compilation; not qualified.
- `just crap`: timed out after 45 seconds during Rust coverage compilation;
  no complexity score obtained. Timed-out build processes were stopped.
- No Rust implementation changed. No live-server data-transfer test is claimed.

### Ticket 02 handoff

See `packages/sync-client/README.md` for concrete owner APIs and source paths.
A JS/WASM SQL owner binding can implement `ofdb_sql::SyncTransport` with this
session and run the persistent database as Initiator. Native callers can use
the public staged database facade. Use the real owner identity and completion
result; individual earlier applies can remain after a later failure.

Current blockers: the WS endpoint only runs SQL (not KV/filesystem content),
Push/Pull requires complementary scoped owner sessions but the server always
starts an unscoped Bidirectional responder, auth requires audience `storage:sync`
and Device selections for hard-coded endpoint `local`, and the WS pre-apply
callback does not recheck admission. Attach precedes database open and cannot
prove backup. Reuse the existing iroh SQL+KV/filesystem drivers for a native
path, or extend authenticated WS dispatch with owner support in a later ticket.
