# 14: Generate passwords and passphrases

**What to build:** A local configurable generator with copy and explicit insertion into a Login draft without saving the item automatically.

**Blocked by:** 06 — Add item details and safe credential actions.

**Status:** ready-for-agent

**Type:** task

**Work status:** open

- [ ] Generate random passwords and passphrases using cryptographically secure randomness. Prefer existing suitable libraries/platform functions; document supported settings and any word-list source/license.
- [ ] Provide accessible configuration, generation, copy, and explicit use-in-draft actions. Handle impossible settings and clipboard failures without exposing generated secrets in errors.
- [ ] Insertion changes only the current draft and works with password confirmation/history rules. Cancel does not persist the generated value or modify another item.
- [ ] Do not add persistent plaintext generated history or claim measured entropy without a justified model. Clear generated values on navigation/disposal/lock.
- [ ] Test configuration constraints, invalid inputs, draft insertion, copy failure, cancel/save/reopen, and lock cleanup. Use focused deterministic logic checks, not flaky statistical assertions as security proof.
- [ ] Ensure passphrase assets and all generator behavior work offline. Preserve existing encrypted backup/restore behavior.

## Comments
