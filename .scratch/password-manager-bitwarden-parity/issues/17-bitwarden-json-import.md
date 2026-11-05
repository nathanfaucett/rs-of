# 17: Preview and atomically import Bitwarden JSON

**What to build:** Import the verified Bitwarden JSON subset into a chosen local vault after an informative preview, without silent loss or partial writes.

**Blocked by:** 07 — Store Secure Note items; 08 — Store Card items; 09 — Store Identity items; 10 — Store SSH Key items; 12 — Organize items in encrypted folders; 13 — Recover deleted items through Trash.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Document and implement the evidence-backed supported JSON subset across all five item types, common metadata, folders, history, URI settings, and custom fields. Use synthetic fixtures, never live credential exports.
- [ ] Preview counts, validation problems, duplicates, and unsupported data before any writes. Unknown field variants, attachments, passkeys, organization ownership, shared permissions, and other unmapped data must be reported, not silently dropped or converted.
- [ ] Provide explicit duplicate behavior for repeated imports. Do not overwrite by display name. Unsupported data requires a clear rejection or explicitly acknowledged exclusion, with the original input unchanged.
- [ ] Validate the entire import before commit. Use an atomic persistence boundary so failed/quota-limited/interrupted imports leave the destination unchanged. Replace the single-item limitation only as needed; do not claim an ordinary write loop is atomic.
- [ ] On success, show imported items in the real list/detail/folder/Trash flows and verify them after close/reopen. Keep source organization metadata distinct from actual local access rights.
- [ ] Test Unicode, multiline text, missing optional values, every supported mapping, unknown variants, duplicates, cancelled preview, corrupt files, transaction failure, lock during import, and uncertain outcomes.
- [ ] Extend encrypted backup/clean-storage restore coverage for imported data. Preserve offline use and do not put input contents in URLs, logs, telemetry, or notifications. CSV, other vendors, and encrypted Bitwarden formats are not implied support.

## Comments
