# 02: Attach an existing local database to a new Storage Resource

**What to build:** Let a User create and edit a vault offline, sign in, confirm the Installation and User/Application destination, and synchronize the same encrypted local database to one newly registered Storage Resource. Use one Create vault action for signed-out and signed-in creation. Do not open another browser database keyed by the remote Resource.

**Blocked by:** 01 — Share one encrypted vault implementation across local and synced workflows

**Status:** ready-for-agent

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Require explicit confirmation before the first upload. Authentication alone must not attach or upload data.
- Reuse the verified IdP and Storage endpoints and the idempotent Storage registration contract.
- Keep the legacy path for opening old synced data, but remove the separate action for creating new synced vaults.
- Do not merge two populated databases or add commit-driven push in this ticket.

## Acceptance checks

- [ ] A vault created and edited offline retains its ID and master password after sign-in and attachment.
- [ ] The confirmation identifies the exact Installation and User/Application destination before any local change is sent.
- [ ] Initial reconciliation publishes the existing local history to one Storage Resource through the authorized SQL transport.
- [ ] Storage contains ciphertext and no known plaintext secret, master password, or unwrapped key.
- [ ] Repeated registration and attachment attempts do not create a duplicate Resource or browser database.
- [ ] The vault remains readable and editable locally after network loss or sign-out.
- [ ] An existing remote Resource with data is not overwritten or silently merged with populated local data.
- [ ] The flow passes through the Password Manager UI and public Storage contract on a real, non-seeded unified backend.

## Current evidence and qualification limits

The legacy synced path still opens a separate browser database. The SDK can now wrap an existing browser SQL database for explicit exchange with an established replica. Construction does not open a second database, request a token, or start sync. SDK tests cover the fixed destination and ownership of the local database. The app does not use this handle for first attachment.

## Comments

First upload still needs a Storage-side guard. The current WebSocket exchange is bidirectional and can apply batches before completion. A browser preflight does not prevent a concurrent SQL or replica write. Reject independent history before either side changes, and keep retries on the same Resource without treating an incomplete upload as an established binding. The non-seeded browser flow must also obtain the serving Node's exact Resource Selection through the authorized Device contract. Do not infer Selection or browser Device approval from sign-in.
