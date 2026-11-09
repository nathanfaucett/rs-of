# Issue 00: Backend - IdP MFA and block/unblock endpoints

## Description

Add MFA management (enable/disable TOTP, WebAuthn) and user block/unblock endpoints to the IdP server. Currently missing from `@ofsdk/idp-client` generated client.

## Type

task

## Priority

Critical

## Status

open

## Blocked by

- None

## Acceptance Criteria

- **MFA enable/disable**: POST `/users/{userId}/mfa` with `{method: "totp"|"webauthn", enable: true|false}`
- **Block/unblock**: POST `/users/{userId}/block` with `{blocked: true|false}`
- Returns updated user info
- Requires admin scopes: `idp.users.mfa`, `idp.users.block`
- Generated client `@ofsdk/idp-client` includes `setUserMfa`, `setUserBlocked` methods

## Seams

- IdP server: user management routes
- OpenAPI spec update → regenerates `@ofsdk/idp-client`
- Admin scope/permission model
- MFA storage (TOTP secrets, WebAuthn credentials)

## Notes

- Blocks: 01-user-write-ops (MFA and block/unblock functionality)
- Other user endpoints exist: `updateUser`, `deleteUser`, `resetPassword`
- MFA implementation may be complex (TOTP secret generation, QR code, WebAuthn ceremony)

## Comments

## Answer
