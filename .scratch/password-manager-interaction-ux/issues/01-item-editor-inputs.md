# 01: Repair item editor inputs and draft ownership

**What to build:** Identity and Secure Note creation and editing show ordinary fields immediately. Actual secrets remain masked and independently revealable. Card and Identity drafts use explicit ownership, so every field saves without Svelte prop-ownership warnings.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

## Scope

- Make ordinary Identity fields and Secure Note or Identity notes visible during data entry.
- Preserve masked inputs, Show, Hide, Copy, saved-detail protection, and hidden custom-field behavior for actual secrets.
- Correct Card and Identity draft binding at the existing editor boundary without adding parallel state.
- Preserve encrypted saves, native validation, generator insertion, and dirty-draft protection.

## Non-goals

- No new item types, secret-storage changes, persistence migration, or broad form redesign.
- Do not expose saved protected values by default or suppress Svelte diagnostics.

## Acceptance checks

- [ ] Ordinary Identity fields are editable without a prior Show action in new and edited records.
- [ ] Secure Note and Identity note editors are visible immediately and their content saves and reopens correctly.
- [ ] Passwords, authenticator keys, private keys, payment secrets, government identifiers, and hidden custom fields retain their protected behavior.
- [ ] Every Card and Identity field accepts and persists edits without `ownership_invalid_mutation` or new browser console errors.
- [ ] Existing masked-editor tests distinguish ordinary editor inputs from protected secrets; they do not retain the outdated hidden-note expectation.
- [ ] Focused public Playwright workflows and relevant protected-field Vitest tests pass.

## Dependencies and evidence

No ticket dependencies, backend changes, schema changes, or data migration. The browser audit confirmed the hidden ordinary fields and runtime prop-ownership warnings. Chromium qualification is required; other engines remain a qualification limit unless run.
