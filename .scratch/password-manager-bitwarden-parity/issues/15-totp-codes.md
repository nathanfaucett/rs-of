# 15: Generate TOTP codes locally

**What to build:** Local code generation and countdown for stored authenticator keys, with safe copy and explicit unsupported-input errors.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Parse supported raw secrets and otpauth URIs. Validate algorithm, digit count, and period; explicitly reject malformed/unsupported inputs without disclosing the key in messages.
- [ ] Show the current code and time remaining in the item detail flow, with accessible copy and failure feedback. Retain protected handling of the underlying secret.
- [ ] Update at time boundaries and immediately reconcile on tab resume. Dispose timers and remove codes/keys from application references on navigation and lock.
- [ ] Verify supported algorithm/digit/period combinations against known-answer vectors and cover malformed URIs, boundary times, suspended/resumed tabs, and clipboard failure.
- [ ] Test save/reopen and backup/restore preservation of stored keys without rewriting key material merely because it was displayed.
- [ ] Operate entirely locally/offline; do not send keys to a service or present this as a second cryptographic factor for local vault unlock. Reuse an existing appropriate dependency if available.

## Comments
