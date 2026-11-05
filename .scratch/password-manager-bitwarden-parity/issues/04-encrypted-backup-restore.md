# 04: Export and restore complete encrypted backups

**What to build:** Explicit encrypted backup download and restore into clean local storage without depending on hidden source-browser metadata.

**Blocked by:** 02 — Authenticate empty and populated vaults; 03 — Separate vault identity from its name.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Define a versioned self-contained backup with derivation parameters, salt, authentication material, encrypted records, and authenticated metadata. Never export an unprotected key.
- [ ] Provide visible export and restore actions, validation feedback, destination identification, and collision handling. Do not overwrite an existing vault without an explicit safe contract.
- [ ] Validate/authenticate the complete backup before committing restore. Wrong passwords, missing metadata, tampering, truncation, unsupported versions, quota failures, and write failures must not leave a visible partial destination or false success.
- [ ] Demonstrate restore in clean storage with no source vault available, for both empty and populated vaults. Compare all supported logical data after close/reopen.
- [ ] Use synthetic sensitive markers to verify that the backup does not expose plaintext item contents. Never log decrypted data or passwords.
- [ ] Test through visible application actions and the public vault API with real persistence. Preserve authoritative-write verification and actionable uncertain-outcome handling.
- [ ] Replace the incomplete encrypted-record export rather than wrapping it. Document the format and recovery limits in a stable topic document.

## Comments
