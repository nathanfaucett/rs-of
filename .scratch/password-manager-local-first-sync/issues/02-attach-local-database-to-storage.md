# 02: Attach an existing local database to a new Storage Resource

**What to build:** Let a User create and edit a vault offline, sign in, confirm the Installation and User/Application destination, and synchronize the same encrypted local database to one newly registered Storage Resource. Use one Create vault action for signed-out and signed-in creation. Do not open another browser database keyed by the remote Resource.

**Blocked by:** 01 — Share one encrypted vault implementation across local and synced workflows

**Status:** ready-for-agent

Milestone: M2 — Single-Node encrypted synchronization

## Scope and non-goals

- Require explicit confirmation before the first upload. Authentication alone must not attach or upload data.
- Reuse the verified IdP and Storage endpoints and the idempotent Storage registration contract. IdP issues tokens; Storage hosts the WebSocket and authorizes the exact Resource and applicable Selection.
- The SDK opens a Resource-scoped transport and supplies it to `ofdb` on the existing local database. The transport carries the existing SQL sync payload bytes. Do not translate local commits into Storage changes or require a new database wrapper at the app boundary.
- Use one SQL Resource per WebSocket in this ticket. The Resource transport must not expose this connection detail to the app; multiplexing and browser KV are follow-up work.
- Keep the legacy path for opening old synced data, but remove the separate action for creating new synced vaults.
- Do not merge two populated databases or add commit-driven push in this ticket.

## Acceptance checks

- [ ] A vault created and edited offline retains its ID and master password after sign-in and attachment.
- [ ] The confirmation identifies the exact Installation and User/Application destination before any local change is sent.
- [ ] Initial reconciliation runs `ofdb` SQL history exchange on the existing local database through the authorized WebSocket transport for the selected Resource.
- [ ] Storage contains ciphertext and no known plaintext secret, master password, or unwrapped key.
- [ ] Repeated registration and attachment attempts do not create a duplicate Resource or browser database.
- [ ] The vault remains readable and editable locally after network loss or sign-out.
- [ ] An existing remote Resource with data is not overwritten or silently merged with populated local data.
- [ ] The flow passes through the Password Manager UI and public Storage contract on a real, non-seeded unified backend.

## Current evidence and qualification limits

The legacy synced path still opens a separate browser database. The SDK has an established-replica helper that reuses an existing database and fixes the destination at construction. This is groundwork, not the target app interface or a first-attachment mechanism. Reuse its transport and lifecycle where applicable; do not base first attachment on the helper's unguarded bidirectional exchange.

## Comments

First upload still needs a Storage-side guard. The current WebSocket exchange is bidirectional and can apply batches before completion. A browser preflight does not prevent a concurrent SQL or replica write. Reject independent history before either side changes, and keep retries on the same Resource without treating an incomplete upload as an established binding. The non-seeded browser flow must also obtain the serving Node's exact Resource Selection through the authorized Device contract. Do not infer Selection or browser Device approval from sign-in.
