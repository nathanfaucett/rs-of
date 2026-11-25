# 02: Make vault forms and editing safe

**What to build:** A person can create and unlock a vault with normal keyboard submission, inspect and edit items with secrets masked by default, receive safe clipboard feedback, and navigate away without accidentally losing a dirty draft.

**Blocked by:** 01 — Restore item browsing and folder assignment

**Status:** ready-for-agent

**Work status:** resolved

**Scope:** Vault creation and unlock forms, protected values in details and editors, field accessibility, clipboard feedback, dirty-draft navigation guards, and document titles.

**Non-goals:** Master-password policy changes, strength meters, breach checks, biometric unlock, per-item re-prompt, or changes to encrypted persistence.

- [x] Creation and unlock submit with Enter, preserve validation and busy behavior, and use appropriate password autocomplete semantics.
- [x] The existing master-password minimum appears as persistent help before submission; its value does not change.
- [x] Every protected field starts masked in details and editors; reveal affects only the chosen field and has a state-specific accessible name.
- [x] Exact field labels identify the intended controls without adjacent action text, and field description IDs remain unique.
- [x] Clipboard success and failure produce short non-secret feedback without copied values or exception text.
- [x] Dirty Cancel, Back, sidebar navigation, browser Back, and reload require confirmation; clean navigation and successful save do not prompt.
- [x] Canceling a dirty-draft prompt preserves the draft, and late editor completion cannot reopen a locked or newer vault session.
- [x] Tested routes have meaningful document titles and the framework-default favicon is replaced without introducing a new brand system.
- [x] Public browser tests cover the complete details-to-editor path; focused component tests cover labels, masking, clipboard denial, and secret non-disclosure.

**Validation:** Run focused browser and component tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion.

## Comments

- Implemented in `e96b8175`, with protected-field and pending-save regressions covered in `33a3c3b0`. Focused component and editing tests and the full Password Manager Chromium project passed.
