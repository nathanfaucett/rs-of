# 02: Make Create lead directly to ordinary sign-in

**What to build:** A new user creates an Installation using required Initial Administrator credentials and a default Node name, sees one progress screen, and reaches ordinary sign-in after current readiness is verified. Optional local data preferences do not become a mandatory completion ceremony.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** ready-for-agent

- [ ] Default bundled onboarding offers Create or Join without requiring internal service composition. Preserve advanced remote configuration as a separate supported path.
- [ ] Create requests only required administrator credentials and derives a sensible Node name without adding a dependency. Retain or provide an ordinary later editing path for that name.
- [ ] Trace Device Setup callers and actual local data-selection behavior before removing its gate. Confirm safe defaults preserve restrictions, selections, and resource authorization.
- [ ] Remove a completion-only click, marker, command, and persistence module where verified redundant. Preserve genuine required owner operations and keep local data-selection preferences editable after Installation Setup.
- [ ] Use the existing shared Create worker and show progress until current verified readiness, then offer ordinary sign-in. Do not establish a bootstrap-only session or persist the administrator password.
- [ ] Accepted work survives page closure. Reload and authorized retry recover the same intent and identities, asking for transient credentials again when required rather than silently issuing a new operation.
- [ ] Rendered-flow tests cover minimal Create input, default naming, progress, verified completion, ordinary sign-in, deferred preferences, and failure/retry without replacement identity.
- [ ] Existing public lifecycle coverage verifies completed reopen and protects against missing or malformed state being treated as fresh setup.
- [ ] Delete superseded Create/completion UI and logic as replacements land; update current Device Setup and onboarding documentation without deleting the domain concept.
- [ ] Run focused frontend and shared/native checks appropriate to the change and record actual outcomes.
