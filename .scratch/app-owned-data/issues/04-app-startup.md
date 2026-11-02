# 04 — Make app startup local and accounts optional

Status: ready-for-agent
Work status: claimed
Type: task
Blocked by: 03

## Goal

A user can open and use the standalone password manager without configuring an
Installation, signing in, or running a desktop/native host.

## Scope

Replace mandatory auth-route and workflow prerequisites with local create/unlock.
Update SDK state, startup callers, callback/sign-in roles, UI states, tests,
package dependencies, aliases, environment docs, and app README together.
Preserve useful editor behavior, metadata/history, ordering, and draft safety.

Add only useful local setup to an independent `@of/sdk` module/export, based on
the proven app operations. Direct owner handles remain accessible. Local setup
must not load or restore OIDC, require service URLs, register an Application, or
create remote resources. Do not change other SDK consumers unnecessarily.

Account configuration belongs to optional sync settings. Disconnect/sign-out must
not delete local data or prevent local unlock. Local lock clears views/drafts and
releases access to keys; explicit data deletion remains a separate action.

## Acceptance

- Fresh browser startup reaches local create/unlock with all services stopped.
- Read/write paths send no HTTP SQL, token exchange, or native bridge requests.
- Preparing, locked, ready-empty, ready, local failure, and sync failure differ.
- UI reports saved locally separately from synced; network failure does not roll
  back a successful local save or hide a valid local vault.
- Lock/switch/teardown clears data promptly and rejects obsolete completions.
- No mandatory `PUBLIC_IDP_BASE_URL`/`PUBLIC_STORAGE_BASE_URL` for local use.
- App remains a static browser deployment. Define offline asset caching for the
  next gate; do not cache plaintext secrets or authenticated responses.
- Keep only shared dependencies still used. Remove obsolete app-only remote
  preparation code instead of wrapping it.

## Comments

When editing Svelte files, load the required Svelte skills and validate components.
This planning task did not inspect or alter their implementation.
