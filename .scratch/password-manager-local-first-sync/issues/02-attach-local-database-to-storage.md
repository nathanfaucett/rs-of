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

The current synced path registers or opens a Resource, then opens a separate browser database. It does not attach the existing local database. First attachment of an established local history is not yet qualified.
