# 18: Export the supported Bitwarden JSON subset

**What to build:** Explicit plaintext export of the verified supported Bitwarden JSON subset with clear warnings and exclusions.

**Blocked by:** 07 — Store Secure Note items; 08 — Store Card items; 09 — Store Identity items; 10 — Store SSH Key items; 12 — Organize items in encrypted folders; 13 — Recover deleted items through Trash.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Define and document supported export mappings from the verified baseline, using synthetic reference fixtures. Include supported fields across all five item types, folders, custom fields, URI settings, dates/history, and an explicit Trash disposition.
- [ ] Provide an accessible user-triggered export flow with a clear plaintext credential warning, scope/exclusion summary, and cancellation before generating a download.
- [ ] Preserve supported logical data and valid reference-format semantics. Report fields that cannot be represented; never claim lossless export outside the tested subset or claim organization permissions are recreated.
- [ ] Reject export while locked and prevent late asynchronous callbacks from downloading decrypted data after lock. Do not persist a plaintext export cache or log contents.
- [ ] Test synthetic Unicode/multiline/optional data, each supported type/field mapping, cancelled export, serialization/download errors, locked state, and explicit unsupported-field handling. Validate output against reference fixtures independently of the importer.
- [ ] Once the importer is available, release qualification must test round trips between both flows. This ticket does not need to wait for the importer to land.
- [ ] Keep export offline and distinguish plaintext portability from the encrypted backup/recovery feature. Preserve encrypted backup/restore coverage.

## Comments
