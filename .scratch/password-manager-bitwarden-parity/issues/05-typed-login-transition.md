# 05: Transition existing records to typed Login items

**What to build:** A versioned typed-item path, exercised end to end by Login items with optional credentials, without losing existing login data.

**Blocked by:** 01 — Verify the reference feature baseline; 04 — Export and restore complete encrypted backups.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Replace the generic required-password model with a discriminated item model. Deliver Login create/edit/cancel/save/reopen behavior through the real collection and vault.
- [ ] Allow username-only or passwordless Login items. Apply verified field validation; retain multiple URIs and stored match settings without claiming autofill.
- [ ] Transition existing records without losing identity, usernames, passwords, TOTP keys, URIs, match settings, notes, favorites, custom fields, timestamps, or password history.
- [ ] Use backup and a versioned atomic or recoverable transition. On malformed data, interrupted writes, or failed transition, leave the source recoverable and do not silently load a partial vault.
- [ ] Retain complete-snapshot validation, single-write acknowledgement, cancel isolation, and authoritative reload for uncertain writes. Delete replaced schema/workflow paths; do not retain parallel legacy writers.
- [ ] Test visible existing/new Login workflows, nested edit cancellation, transition failures, real-persistence reopen, and encrypted backup/restore of the new format.
- [ ] Check lock cleanup and offline asset requirements for this change. Use existing UI, validation, and storage patterns rather than a new abstraction layer.

## Comments
