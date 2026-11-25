# 04: Apply lock settings to the active session

**What to build:** A person who changes the inactivity timeout gets the confirmed behavior immediately. The same value remains visible after route changes and is loaded from the existing encrypted representation after unlock or reload.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** resolved

**Scope:** The unlocked vault-state owner, active inactivity controller, Settings feedback, and focused state/browser tests.

**Non-goals:** New timeout options, persisted-format changes, master-password changes, synchronization, or backend changes.

- [x] Saving persists through the existing vault interface, updates the owning unlocked state with the confirmed value, and reapplies it to the active inactivity controller.
- [x] A confirmed save starts a new inactivity deadline and remains selected after leaving and returning to Settings.
- [x] Lock/unlock and reload load the confirmed value without migration or format changes.
- [x] A failed save retains the previous active and displayed settings and does not report success.
- [x] A stale completion after lock or a newer vault session cannot update that session.
- [x] Browser coverage uses clock control after vault setup to prove the one-minute active-lock boundary without a real-minute delay.
- [x] Focused state tests cover confirmation, failure, and stale-session behavior without replacing the public browser acceptance path.

**Validation:** Run focused state and browser tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion. Record any unrun cross-restart or non-Chromium qualification explicitly.

## Comments

- Implemented in `9f9a7d83`, with retired-session timer coverage added in `33a3c3b0`. State tests and Chromium confirmed immediate timeout changes, reload, and unlock using the retained value. Browser-process restart and other browsers were not tested.
