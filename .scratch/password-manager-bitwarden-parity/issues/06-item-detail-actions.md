# 06: Add item details and safe credential actions

**What to build:** A read-only item detail flow with explicit editing, safe reveal/copy actions, and usable dated password history.

**Blocked by:** 05 — Transition existing records to typed Login items.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Opening a Login item shows details without entering edit mode or changing persistence. Sensitive values start masked; reveal is explicit and resets on navigation and lock.
- [ ] Provide explicit copy actions with success/failure feedback. Handle denied or unavailable clipboard access. Any clipboard clearing is best effort and must not overwrite newer content.
- [ ] Support explicit edit, save, cancel, and unsaved-edit handling. Cancel must discard nested edits and leave stored data unchanged.
- [ ] Show dated password history with protected reveal/copy behavior. Record actual password changes, not notes-only edits or abandoned drafts.
- [ ] Use keyboard-operable controls, clear labels, predictable focus, and accessible error feedback. Do not expose credentials in URLs, notifications, logs, or telemetry.
- [ ] Test the real workflow/collection through visible actions, including clipboard failure, history preservation, cancellation, navigation, lock cleanup, and late asynchronous results.
- [ ] Preserve backup/restore and offline operation. Keep the detail/edit pattern usable by subsequent item types without speculative plugin infrastructure.

## Comments
