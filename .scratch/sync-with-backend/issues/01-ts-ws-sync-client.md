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

**Status:** ready-for-agent

- [ ] Client connects to the sync endpoint derived from a storage API URL and completes the Hello/Attach handshake with a valid IdP token
- [ ] Attach rejection (unauthorized/invalid token) surfaces as a clear auth error, not a hang or generic failure
- [ ] Push, Pull, and Bidirectional directions are selectable and reported by the session result
- [ ] Protocol behavior (framing, handshake, direction, accept/reject) is tested against a mock/in-memory WebSocket at the protocol seam, asserting external behavior only
- [ ] Package follows the existing package layout and build/test conventions of the other client packages
