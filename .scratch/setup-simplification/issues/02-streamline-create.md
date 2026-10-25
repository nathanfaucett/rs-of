# 02: Make Create lead directly to ordinary sign-in

**What to build:** A new user creates an Installation using required Initial Administrator credentials and a default Node name, sees one progress screen, and reaches ordinary sign-in after current readiness is verified. Optional local data preferences do not become a mandatory completion ceremony.

**Blocked by:** 01 — Unify onboarding observation and recovery routing.

**Status:** resolved

- [x] Default bundled onboarding offers Create or Join without requiring internal service composition. Preserve advanced remote configuration as a separate supported path.
- [x] Create requests only required administrator credentials and derives a sensible Node name without adding a dependency. Retain or provide an ordinary later editing path for that name.
- [x] Trace Device Setup callers and actual local data-selection behavior before removing its gate. Confirm safe defaults preserve restrictions, selections, and resource authorization.
- [x] Remove a completion-only click, marker, command, and persistence module where verified redundant. Preserve genuine required owner operations and keep local data-selection preferences editable after Installation Setup.
- [x] Use the existing shared Create worker and show progress until current verified readiness, then offer ordinary sign-in. Do not establish a bootstrap-only session or persist the administrator password.
- [x] Accepted work survives page closure. Reload and authorized retry recover the same intent and identities, asking for transient credentials again when required rather than silently issuing a new operation.
- [x] Rendered-flow tests cover minimal Create input, default naming, progress, verified completion, ordinary sign-in, deferred preferences, and failure/retry without replacement identity.
- [x] Existing public lifecycle coverage verifies completed reopen and protects against missing or malformed state being treated as fresh setup.
- [x] Delete superseded Create/completion UI and logic as replacements land; update current Device Setup and onboarding documentation without deleting the domain concept.
- [x] Run focused frontend and shared/native checks appropriate to the change and record actual outcomes.


## Answer

Implemented minimal Create on `setup-simplification-02` in `of-setup-02`. Bundled Create takes only administrator credentials, defaults the Node name to `My Node`, retrieves scoped admission internally and uses the shared worker. Advanced connections and independent owner scopes remain supported. Exact recovery retains the operation/name/identity and requires a newly entered password. Verified readiness now leads directly to ordinary sign-in; the Device-dismissal route, native completion command, observation flag and persistence module are deleted. Device Setup remains ordinary authenticated selection/restriction/Residency configuration, with existing rename controls and unchanged resource authorization.

Updated [setup worker docs](../../../docs/setup-workers.md), [native handoff](../../../apps/app/src-tauri/NATIVE-SETUP.md) and glossary navigation. [Evidence](../evidence/02-streamline-create.md) records the caller/default audit, TDD failures and passing checks: 180 frontend, 15 mounted, 15 native plus isolated child, 40 API plus doc test, clean Svelte/build/Clippy/rustfmt. The separate unified-host test, full feature matrix and CRAP timed out compiling and remain unqualified. No real OS/provider/GUI acceptance is claimed. Integration merge was already up to date; adjacent ticket behaviors were not implemented here.
