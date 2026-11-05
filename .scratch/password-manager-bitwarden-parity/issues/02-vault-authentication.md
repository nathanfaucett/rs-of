# 02: Authenticate empty and populated vaults

**What to build:** Consistent master-password authentication before a local vault becomes unlocked, even when it contains no items.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] New empty vaults accept the correct password and reject wrong passwords before showing an unlocked view. The same rules hold for populated vaults and vaults whose last item was removed.
- [ ] Authenticate independently of item presence using a reviewed authenticated vault-level mechanism. Do not treat algorithm choice as proof of security.
- [ ] Define safe handling of existing vaults without authentication metadata. A legacy empty vault has no stored password proof; do not claim to verify its original password or silently establish a new password from a failed attempt.
- [ ] Preserve existing encrypted data and ensure failed authentication or metadata writes do not damage the vault. Provide actionable errors without logging passwords or keys.
- [ ] Test the visible unlock flow and the public vault API with real persistence across create, close, and reopen. Cover correct/wrong passwords, empty/populated data, removal of the last item, malformed authentication data, and write failure.
- [ ] Keep authentication entirely local. Remove obsolete authentication paths after the safe format transition; add no legacy writer or compatibility wrapper.

## Comments
