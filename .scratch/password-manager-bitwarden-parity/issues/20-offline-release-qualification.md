# 20: Qualify the complete offline release

**What to build:** Repeatable release evidence that the complete core personal-vault feature set works offline and preserves data through migration, recovery, and failures.

**Blocked by:** 04 — Export and restore complete encrypted backups; 11 — Search, filter, and sort unlocked items; 12 — Organize items in encrypted folders; 13 — Recover deleted items through Trash; 14 — Generate passwords and passphrases; 15 — Generate TOTP codes locally; 16 — Enforce manual and automatic lock; 17 — Preview and atomically import Bitwarden JSON; 18 — Export the supported Bitwarden JSON subset; 19 — Start and reload the app offline.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Run the full application test/build checks and a repeatable real-browser suite with synthetic vaults, real persistence, production assets, and disabled network access. Do not replace evidence with mocked UI results.
- [ ] Exercise all five item types through create/view/edit/cancel/save/reopen, combined search/filter/sort, folders, Trash restore/permanent deletion, generator, TOTP, and protected copy/history actions.
- [ ] Verify existing-vault format transitions and complete encrypted backup restore in a clean profile without the source environment. Compare all supported logical data, including folders, Trash, history, and optional fields.
- [ ] Round-trip synthetic Bitwarden JSON through export/import and compare the documented supported subset. Confirm exclusions, duplicate handling, plaintext warnings, and no silent field loss.
- [ ] Test wrong passwords for empty/populated vaults, vault-name collisions, damaged metadata/backups/imports, storage denial/quota errors, failed and uncertain writes, cancelled operations, and atomic import rollback.
- [ ] Test offline startup/reload with cached database assets, cache-update failure, inactivity deadlines after tab suspension, stale handles, pending writes, and late callbacks after lock. No core operation may require a successful network request.
- [ ] Verify keyboard/focus/error behavior, masked sensitive values, denied clipboard access, no persistent plaintext search/index/generated history, and no sensitive logs or URLs. Document browser memory/clipboard/storage limits honestly.
- [ ] Fix integration failures caused by this effort, using the existing owner boundaries rather than wrappers. If storage-library code changed, run the affected Rust repository's required validation as well.
- [ ] Record exact commands, environment, pass/fail outcomes, supported reference subset, and remaining limits in stable named qualification/operations documentation. Do not claim full Bitwarden parity, online-feature offline support, or checks that were not run.

## Comments
