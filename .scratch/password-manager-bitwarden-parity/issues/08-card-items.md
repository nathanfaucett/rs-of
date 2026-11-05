# 08: Store Card items

**What to build:** Complete Card item management with relevant payment fields, protected display, and encrypted local persistence.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Use the verified Card field mapping and document validation decisions. Do not invent mandatory fields or reject valid stored/imported data solely from an unverified brand or checksum assumption.
- [ ] Deliver type selection, create, list, detail, edit, cancel, and save through the typed item model and real storage. Card items must not require a login password.
- [ ] Mask sensitive payment values by default and support explicit reveal/copy with safe feedback and keyboard access. Preserve applicable common metadata and custom fields.
- [ ] Test optional fields, representative valid/invalid input, Unicode, nested cancellation, isolated updates, write errors, and close/reopen using synthetic payment data only.
- [ ] Extend encrypted backup/clean-storage restore tests for Cards and mixed-type vaults. Verify sensitive values are not stored in plaintext or logged.
- [ ] Preserve lock cleanup, authoritative-write verification, and offline operation/assets.

## Comments
