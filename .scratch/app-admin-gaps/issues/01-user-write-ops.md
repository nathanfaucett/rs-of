# Issue 01: User create/edit/delete + password reset

## Description

Implement full user lifecycle management in the admin UI. Currently users are read-only (list + detail). Admin needs to create, edit, delete users, reset passwords, manage MFA, and block/unblock.

## Type

task

## Priority

Critical

## Status

open

## Blocked by

- **00-backend-user-create**: IdP server must add `createUser` endpoint (MISSING from @ofsdk/idp-client)
- **00-backend-user-mfa-block**: IdP server must add MFA enable/disable and block/unblock endpoints (MISSING)

## Acceptance Criteria

- **Create user**: Form with email, username, initial password (or send invite), role assignment
- **Edit user**: Modify profile fields (name, email, username, locale)
- **Delete user**: Confirmation dialog, hard delete
- **Password reset**: Button to trigger reset email / generate temporary password
- **MFA**: Enable/disable MFA for user (TOTP, WebAuthn)
- **Block/unblock**: Toggle to prevent sign-in

## Seams

- `idpApi` — available: `updateUser`, `deleteUser`, `resetPassword`
- `idpApi` — **MISSING**: `createUser`, MFA endpoints, block/unblock endpoints
- Route: `(auth)/users/+page.svelte` (list) → add "Create user" button (blocked by 00-backend-user-create)
- Route: `(auth)/users/[userId]/+page.svelte` (detail) → add edit/delete/reset/MFA/block actions
- UI patterns: Page, Card, Field, Alert, Table, notifications, confirm dialogs
- Route: `(auth)/users/+page.svelte` (list) → add "Create user" button
- Route: `(auth)/users/[userId]/+page.svelte` (detail) → add edit/delete/reset/MFA/block actions
- UI patterns: Page, Card, Field, Alert, Table, notifications, confirm dialogs

## Notes

- **Verified**: `updateUser`, `deleteUser`, `resetPassword` exist in @ofsdk/idp-client
- **MISSING**: `createUser`, MFA (TOTP/WebAuthn), block/unblock — require Rust IdP server changes
- Split into two backend tickets (00-backend-user-create, 00-backend-user-mfa-block) before UI work can start
- UI can implement edit/delete/reset immediately once backend tickets unblock create

## Comments

## Answer
