# 09: Store Identity items

**What to build:** Complete Identity item management with verified contact/identity fields and preservation of optional data.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Implement the verified Identity field mapping across validation, encrypted persistence, list, detail, and editor. Avoid locale-specific assumptions that reject supported international names, addresses, or telephone values.
- [ ] Support create, view, edit, cancel, save, and reopen without requiring a login password. Preserve optional values and applicable common metadata/custom fields.
- [ ] Protect sensitive identity values in detail and copy actions. Provide accessible labels, focus, validation, and failure feedback; do not log personal data.
- [ ] Test synthetic international and sparse records, Unicode, multiline values where supported, cancellation, isolated updates, failed writes, and real-persistence reopen.
- [ ] Extend encrypted backup/clean-storage restore coverage for Identity items and mixed-type vaults, including optional-field preservation.
- [ ] Preserve lock cleanup, complete-snapshot validation, acknowledged writes, and offline operation/assets.

## Comments
