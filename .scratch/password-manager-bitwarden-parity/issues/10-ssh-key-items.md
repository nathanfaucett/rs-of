# 10: Store SSH Key items

**What to build:** Complete local storage and protected viewing of SSH public/private key material as its own item type.

**Blocked by:** 05 — Transition existing records to typed Login items; 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Implement the verified SSH Key field mapping and validation across the typed model, persistence, list, detail, and editor. Record supported key representations and unsupported inputs explicitly.
- [ ] Support create, view, edit, cancel, save, and reopen without a required Login password. Preserve multiline key material and applicable metadata/custom fields without unintended normalization.
- [ ] Mask private key material by default. Reveal/copy must be explicit, keyboard accessible, and reset on navigation and lock. Do not log keys or include them in error messages.
- [ ] Test only synthetic keys: supported representations, malformed input, multiline preservation, cancellation, isolated updates, write failures, and real-persistence reopen.
- [ ] Extend encrypted backup/clean-storage restore coverage for SSH Key and mixed-type vaults. Preserve acknowledged writes and complete-snapshot validation.
- [ ] Preserve offline operation/assets. Do not add SSH key generation, an SSH agent, passkey support, or native integration.

## Comments
