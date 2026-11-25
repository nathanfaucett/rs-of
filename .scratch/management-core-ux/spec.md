# Management app core UX and authorization repair

**Status:** ready-for-agent

**Milestone:** M3 — Two-Device offline use and administration

## Problem Statement

The management app does not provide reliable core administration. An expired session leaves a broken authenticated shell. The current administrator cannot use several Role, Permission, OAuth Client, User, and Device workflows. Errors hide their causes. Forms bypass validation, some dialogs have wrong accessible names or broken Cancel buttons, and the default mobile layout clips primary actions.

The Devices page also contains a Sync panel that cannot synchronize data. It checks capability, saves local preferences, and always reports a blocked result. It is not the native Iroh synchronization path or the browser Storage WebSocket path. Its sign-in message is misleading.

## Solution

Make each existing management workflow complete, clear, and safe for authorized users. Recover expired sessions, preserve valid input, explain authorization failures, and make forms, dialogs, lists, and navigation usable with keyboard and small screens.

Remove the Devices Sync panel and code that becomes unused. Keep Device approval, Device policy, Selection, native synchronization commands, and the browser Storage SDK outside this dead-end panel. Do not add a replacement sync UI to Devices.

## User Stories

1. As a signed-in administrator, I want an expired session to refresh or return me to Sign in, so that I can continue without generic API errors.
2. As an authorized administrator, I want to manage Roles and Permissions, so that I can assign application access.
3. As an authorized administrator, I want to see Roles assigned to a User, so that I can inspect effective policy.
4. As a user without management authorization, I want a clear read-only explanation, so that I do not mistake a permission denial for a broken page.
5. As an authorized administrator, I want to create and reopen an OAuth Client, so that I can use its registration details.
6. As an authorized administrator, I want newly created clients to remain visible while I navigate within the same session, so that I do not lose their IDs.
7. As an authorized administrator, I want to create a User with valid details, so that the account can sign in.
8. As a user creator, I want native validation and Enter submission, so that I cannot accidentally send malformed email addresses or short passwords.
9. As an administrator, I want password reset to require a real destination and report its result, so that I know what happened.
10. As an administrator, I want destructive account actions to require confirmation, so that I do not block or delete a User accidentally.
11. As an administrator, I want clear MFA and Block results, so that I know the current security state.
12. As an authorized Device administrator, I want rename and replication-policy operations to work, so that I can manage Devices.
13. As a Device administrator, I want one actionable enrollment error and preserved input, so that I can correct the request.
14. As a Device administrator, I want meaningful timestamps, so that I am not shown the Unix epoch as a real update time.
15. As an application administrator, I want valid URI validation and safe links, so that malformed records do not enter the list.
16. As an application administrator, I want pagination to stop at the end and after failures, so that I cannot move through unlimited empty pages.
17. As a mobile administrator, I want navigation and content to fit my screen, so that I can complete the same workflows on a small device.
18. As a keyboard or screen-reader user, I want correctly named dialogs and working Cancel controls, so that I can understand and leave each dialog.
19. As an administrator, I want useful errors and recovery actions, so that I can distinguish validation, authorization, missing records, expired sessions, and service failures.
20. As an administrator, I want clickable names and truthful search/filter controls, so that I can find records without copying UUIDs.
21. As an administrator, I want route titles, persistent navigation preferences, and a way home from errors, so that I can orient myself.
22. As a user signing in, I want concise and correct validation text, so that I can correct my credentials.
23. As a Device administrator, I want only meaningful Device controls on the Devices page, so that I am not invited to run a synchronization that always reports Blocked.

## Implementation Decisions

### Session and authorization

- Refresh the OIDC session before access-token exchange when the stored access token expires. Share concurrent refresh attempts and reject stale completions after a session change.
- If renewal fails, clear the invalid session and return to Sign in with a safe return location. Do not keep an authenticated shell that issues repeated failing requests.
- Use existing OIDC, token-exchange, Management, Device, Storage, and generated-client contracts. Do not add a parallel session store.
- Reproduce each `403` and `500` at its owning boundary. Fix incorrect client authorization or service behavior rather than hiding it.
- Do not grant broad privileges to make tests pass. Authorized workflows must succeed; unauthorized workflows must present an intentional read-only or recovery state.
- Do not automatically migrate existing Installation policy or persisted records without separate evidence and an explicit migration decision.

### Devices and synchronization

- Remove the Sync card, its controls, and code that becomes unused from the management app.
- Do not show `Sign in before checking sync.`, Push/Pull/Bidirectional preferences, or always-blocked target checks on Devices.
- Preserve native synchronization commands, Iroh endpoint handling, Storage WebSocket synchronization, Device approval, Device policy, Selection, and Residency behavior.
- Do not add a manual synchronization control to Devices. Synchronization UI belongs to the application and Resource workflow that owns the data.
- Leave old browser sync-preference entries untouched. The revised UI must not read or write them. No persisted-data migration is required.
- Keep the native Iroh path and browser WebSocket path distinct in documentation and tests. Their presence is not part of this UI repair.
- Repair Device rename, policy reads and updates, enrollment feedback, and timestamp presentation at their owning boundaries.

### Applications and OAuth Clients

- Validate Application URI input against the existing Application contract. Reject malformed or unsafe values before submission and do not render unsafe values as links.
- Stop pagination when fewer results than the page limit return, when API metadata identifies the end, or when loading fails. Preserve the last valid offset after a failed page change.
- Make application and User names open the same detail destination as their IDs.
- Keep created OAuth Client metadata in memory scoped to the current application and authenticated session. It must survive SPA navigation but not sign-out or replacement by a newer session.
- Do not persist client secrets or registration responses in browser storage. Reload loss must be explained, and authorized client-ID lookup must work.
- Distinguish missing, unauthorized, and failed client lookup. Do not offer dead detail links.

### Users and account security

- Put Create User fields and actions in one real form. Use native email and password constraints, Enter submission, one pending request, and `new-password` autocomplete.
- Prevent invalid submissions before any API request. Preserve valid draft values after a failed request.
- Show a password-reset destination only when it exists. Disable the action with an explanation when no email or supported reset destination exists.
- Show reset success or failure in the dialog and preserve a working Cancel action. Do not display placeholder text as an email address.
- Require confirmation before Block. Preserve the current state after denial or failure and show the result.
- Keep the existing MFA contract. Make success and denial visible; do not invent a new enrollment protocol in this repair.

### Shared UI, errors, and navigation

- Give every modal instance a unique title ID. Multiple mounted modals must retain their own accessible names.
- Make all Cancel buttons close their dialogs through valid dialog-form submission or explicit close behavior. Preserve Close and Escape behavior and verify focus return.
- Replace raw `ResponseError` text with short, status-aware guidance and an appropriate retry, reauthentication, back, or permission action. Keep technical details out of primary UI text.
- Do not clear valid drafts or show unrelated error notifications after a successful save.
- Use a small-screen navigation pattern that leaves the main workflow usable. Persist the sidebar preference across reloads and keep the Configuration control from obscuring actions or records.
- Make wide tables usable without clipping primary actions. Test reflow rather than relying only on horizontal page overflow.
- Add a unique document title to each normal route and a Dashboard or Back action to the error page.
- Add search, filter, or sort only with truthful scope. Clearly label current-page controls; do not imply server-wide search when only loaded rows are searched.
- Correct sign-in validation copy and use the SvelteKit-provided fetch where route loads currently trigger framework warnings.

### Compatibility and ownership

- Make the smallest complete change at the owning frontend, client, or service boundary. Reuse existing public interfaces and generated contracts.
- No schema, persistence-format, public API, or wire-protocol change is expected. Any evidence that requires one must be recorded before implementation continues.
- Preserve existing Installation, Application, OAuth Client, User, Device, and Resource data. Use synthetic records and restore administrator fixtures in tests.
- Delete only code and documentation made obsolete by Sync panel removal and the UX repairs. Do not remove working native or browser synchronization implementations.

## Testing Decisions

### Primary seam: existing management app Playwright project

Use the existing real-backend app Playwright project as the primary seam. Test public browser behavior rather than component internals.

Cover:

- Controlled access-token expiry, concurrent renewal, recovery after failed renewal, and safe return to the original route.
- Authorized Role, Permission, OAuth Client, User, and Device success paths, plus explicit unauthorized fixtures.
- Application create/edit/delete, URI rejection, exact pagination boundaries, failed page changes, and name-based navigation.
- Client create, detail, Back, reload expectations, sign-out cleanup, and lookup failure states.
- User creation, native validation, Enter submission, and preservation after failure.
- Password reset with and without a destination, reset failure, MFA, Block confirmation, and denial paths.
- Device rename, policy operations, enrollment validation, and unknown timestamps.
- Multiple mounted modals, accessible names, unique title IDs, Cancel, Close, Escape, and focus return.
- Desktop and narrow layouts at 320, 390, 768, and 1440 CSS pixels, plus sidebar persistence and table/action reflow.
- Route titles, error-page navigation, sign-in copy, and absence of new browser console errors.
- Devices has no Sync card, Sync controls, or blocked target checks and does not write old sync-preference keys.

Use synthetic records and clean them up. Do not delete the administrator or change its password. Do not wait for real token expiry; use a controlled short-lived fixture.

### Supporting seams

- Use existing Vitest state tests for refresh single-flight, stale-session protection, pagination boundaries, URI validation, session-scoped client metadata, and truthful list filtering.
- Use real-owner service tests when a failure originates in IdP, Management, Device, or Storage authorization or reset behavior. Browser tests alone cannot locate that root cause.
- Keep native synchronization and browser Storage SDK tests unchanged unless a shared edit requires their focused qualification.

### Acceptance evidence

For each ticket, run its focused real-backend browser tests and relevant unit or owner tests. Final validation must include the app package test, type check, lint, build, E2E TypeScript check, full app Playwright project, and affected owner tests.

Record any unrun browser engines, native synchronization paths, actual reset-email delivery, or device enrollment scenarios as qualification limits. Never claim an unrun check passed.

## Out of Scope

- Implementing or redesigning native Iroh synchronization or browser WebSocket synchronization.
- Adding a replacement Sync panel to Devices or another application.
- New Resource types, multi-Device convergence work, durable replay, or automatic real-time delivery.
- Production password-manager parity or unrelated application features.
- Broad visual redesign, new design-system dependencies, or rewrites of working workflows.
- Actual email delivery infrastructure. The reset request and UI result are tested with local fixtures.
- Firefox, WebKit, real mobile devices, screen-reader qualification, and native desktop packaging beyond the focused tests selected for a change.
- Automatic authorization migration for existing Installations without a separate proven migration decision.

## Further Notes

The source evidence is a Playwriter walkthrough on 2026-11-25 using the local `admin` account at desktop and mobile sizes. It produced 23 confirmed findings. Vite/HMR and browser-extension-only errors were excluded. The role-draft loss and Device-label concern were retested and excluded.

The accepted sync decision is explicit: remove the dead Devices panel. Do not represent target synchronization architecture as current behavior, and do not weaken the existing authorization boundaries to obtain successful tests.
