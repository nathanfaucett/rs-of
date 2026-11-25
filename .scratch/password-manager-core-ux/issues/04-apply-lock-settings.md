# 04: Apply lock settings to the active session

**What to build:** A person who changes the inactivity timeout gets the confirmed behavior immediately. The same value remains visible after route changes and is loaded from the existing encrypted representation after unlock or reload.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Work status:** open

**Scope:** The unlocked vault-state owner, active inactivity controller, Settings feedback, and focused state/browser tests.

**Non-goals:** New timeout options, persisted-format changes, master-password changes, synchronization, or backend changes.

- [ ] Saving persists through the existing vault interface, updates the owning unlocked state with the confirmed value, and reapplies it to the active inactivity controller.
- [ ] A confirmed save starts a new inactivity deadline and remains selected after leaving and returning to Settings.
- [ ] Lock/unlock and reload load the confirmed value without migration or format changes.
- [ ] A failed save retains the previous active and displayed settings and does not report success.
- [ ] A stale completion after lock or a newer vault session cannot update that session.
- [ ] Browser coverage uses clock control after vault setup to prove the one-minute active-lock boundary without a real-minute delay.
- [ ] Focused state tests cover confirmation, failure, and stale-session behavior without replacing the public browser acceptance path.

**Validation:** Run focused state and browser tests, then the Password Manager package test, type-check, lint, and build scripts. Run the Password Manager Playwright project before completion. Record any unrun cross-restart or non-Chromium qualification explicitly.
